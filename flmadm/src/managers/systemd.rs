use crate::types::InstallProfile;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct SystemdManager;

impl SystemdManager {
    pub fn new() -> Self {
        Self
    }

    /// Generate and install systemd service files
    pub fn install_services(&self, prefix: &Path, profiles: &[InstallProfile]) -> Result<()> {
        println!("⚙️  Installing systemd service files...");

        let prefix_str = prefix.to_str().unwrap();

        let has_control_plane = profiles.contains(&InstallProfile::ControlPlane);
        let has_worker = profiles.contains(&InstallProfile::Worker);

        // Write to /etc/systemd/system/
        if has_control_plane {
            let fsm_service = self.generate_session_manager_service(prefix_str);
            let fsm_path = PathBuf::from("/etc/systemd/system/flame-session-manager.service");
            fs::write(&fsm_path, fsm_service)
                .context("Failed to write flame-session-manager.service")?;
            println!("  ✓ Installed flame-session-manager.service");
        } else {
            println!("  ⊘ Skipped flame-session-manager.service (control plane not selected)");
        }

        if has_worker {
            let fem_service = self.generate_executor_manager_service(prefix_str);
            let fem_path = PathBuf::from("/etc/systemd/system/flame-executor-manager.service");
            fs::write(&fem_path, fem_service)
                .context("Failed to write flame-executor-manager.service")?;
            println!("  ✓ Installed flame-executor-manager.service");
        } else {
            println!("  ⊘ Skipped flame-executor-manager.service (worker not selected)");
        }

        // Only reload if we installed at least one service
        if has_control_plane || has_worker {
            self.daemon_reload()?;
        }

        println!("✓ Installed systemd service files");
        Ok(())
    }

    /// Remove systemd service files
    pub fn remove_services(&self) -> Result<()> {
        println!("🗑️  Removing systemd service files...");

        let fsm_path = PathBuf::from("/etc/systemd/system/flame-session-manager.service");
        let fem_path = PathBuf::from("/etc/systemd/system/flame-executor-manager.service");

        // Stop services first
        let _ = self.stop_service("flame-executor-manager");
        let _ = self.stop_service("flame-session-manager");

        // Disable services
        let _ = self.disable_service("flame-executor-manager");
        let _ = self.disable_service("flame-session-manager");

        // Remove service files
        if fsm_path.exists() {
            fs::remove_file(&fsm_path).context("Failed to remove flame-session-manager.service")?;
        }
        if fem_path.exists() {
            fs::remove_file(&fem_path)
                .context("Failed to remove flame-executor-manager.service")?;
        }

        // Reload systemd daemon
        self.daemon_reload()?;

        println!("✓ Removed systemd service files");
        Ok(())
    }

    /// Enable and start systemd services
    pub fn enable_and_start_services(&self, profiles: &[InstallProfile]) -> Result<()> {
        println!("🚀 Enabling and starting Flame services...");

        let has_control_plane = profiles.contains(&InstallProfile::ControlPlane);
        let has_worker = profiles.contains(&InstallProfile::Worker);

        if has_control_plane {
            // Enable and start session manager
            self.enable_service("flame-session-manager")?;
            self.start_service("flame-session-manager")?;
            self.wait_for_service_active("flame-session-manager", 15)?;
        }

        if has_worker {
            // Enable and start executor manager
            self.enable_service("flame-executor-manager")?;
            self.start_service("flame-executor-manager")?;
            self.wait_for_service_active("flame-executor-manager", 15)?;
        }

        println!("✓ Services are running");
        Ok(())
    }

    fn daemon_reload(&self) -> Result<()> {
        let output = Command::new("systemctl")
            .arg("daemon-reload")
            .output()
            .context("Failed to reload systemd daemon")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to reload systemd daemon: {}", stderr);
        }

        Ok(())
    }

    fn enable_service(&self, service: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["enable", service])
            .output()
            .context(format!("Failed to enable {}", service))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to enable {}: {}", service, stderr);
        }

        Ok(())
    }

    fn disable_service(&self, service: &str) -> Result<()> {
        let _output = Command::new("systemctl")
            .args(["disable", service])
            .output()
            .context(format!("Failed to disable {}", service))?;

        // Ignore errors for disable (service might not be enabled)
        Ok(())
    }

    fn start_service(&self, service: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["start", service])
            .output()
            .context(format!("Failed to start {}", service))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to start {}: {}", service, stderr);
        }

        println!("✓ Started {}", service);
        Ok(())
    }

    fn stop_service(&self, service: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["stop", service])
            .output()
            .context(format!("Failed to stop {}", service))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Don't fail if service wasn't running
            if !stderr.contains("not loaded") {
                println!("⚠️  Warning: Failed to stop {}: {}", service, stderr);
            }
        } else {
            println!("✓ Stopped {}", service);
        }

        Ok(())
    }

    fn check_service_status(&self, service: &str) -> Result<String> {
        let output = Command::new("systemctl")
            .args(["is-active", service])
            .output()
            .context(format!("Failed to check {} status", service))?;

        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(status)
    }

    /// Wait for a service to become active with retry logic
    fn wait_for_service_active(&self, service: &str, max_wait_secs: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let mut last_status = String::new();

        loop {
            match self.check_service_status(service) {
                Ok(status) if status == "active" => {
                    println!("✓ {} is active", service);
                    return Ok(());
                }
                Ok(status) => {
                    last_status = status;
                    // Service is not active yet, keep waiting
                }
                Err(e) => {
                    // Error checking status, log it but continue
                    println!("⚠️  Warning: Failed to check {} status: {}", service, e);
                }
            }

            // Check if we've exceeded max wait time
            if start.elapsed().as_secs() >= max_wait_secs {
                // Get detailed status and logs for debugging
                let _ = self.show_service_status(service);
                anyhow::bail!(
                    "{} is not active after {}s (status: {})",
                    service,
                    max_wait_secs,
                    last_status
                );
            }

            // Wait before retrying
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    /// Show detailed service status for debugging
    fn show_service_status(&self, service: &str) -> Result<()> {
        println!("\n=== Debugging {} ===", service);

        // Show service status
        let output = Command::new("systemctl")
            .args(["status", service])
            .output()
            .context(format!("Failed to get {} status", service))?;

        println!("{}", String::from_utf8_lossy(&output.stdout));

        // Show recent journal logs
        let output = Command::new("journalctl")
            .args(["-u", service, "-n", "20", "--no-pager"])
            .output()
            .context(format!("Failed to get {} logs", service))?;

        println!("\n=== Recent logs ===");
        println!("{}", String::from_utf8_lossy(&output.stdout));

        Ok(())
    }

    fn generate_session_manager_service(&self, prefix: &str) -> String {
        format!(
            r#"[Unit]
Description=Flame Session Manager
Documentation=https://github.com/xflops-io/flame
After=network.target
Wants=network-online.target

[Service]
Type=simple
Environment="RUST_LOG=info"
Environment="FLAME_HOME={prefix}"
WorkingDirectory={prefix}
ExecStart={prefix}/bin/flame-session-manager --config {prefix}/conf/flame-cluster.yaml
StandardOutput=append:{prefix}/logs/fsm.log
StandardError=append:{prefix}/logs/fsm.log
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
"#,
            prefix = prefix
        )
    }

    fn generate_executor_manager_service(&self, prefix: &str) -> String {
        format!(
            r#"[Unit]
Description=Flame Executor Manager
Documentation=https://github.com/xflops-io/flame
After=network.target flame-session-manager.service
Wants=network-online.target
Requires=flame-session-manager.service

[Service]
Type=simple
Environment="RUST_LOG=info"
Environment="FLAME_HOME={prefix}"
WorkingDirectory={prefix}/work
ExecStart={prefix}/bin/flame-executor-manager --config {prefix}/conf/flame-cluster.yaml
StandardOutput=append:{prefix}/logs/fem.log
StandardError=append:{prefix}/logs/fem.log
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
"#,
            prefix = prefix
        )
    }
}
