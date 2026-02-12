use crate::types::{BuildArtifacts, InstallProfile, InstallationPaths};
use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub struct InstallationManager;

impl InstallationManager {
    pub fn new() -> Self {
        Self
    }

    /// Create all required directories
    pub fn create_directories(&self, paths: &InstallationPaths) -> Result<()> {
        println!("📁 Creating directory structure...");

        for (name, path) in [
            ("bin", &paths.bin),
            // Note: sdk/python is created by install_python_sdk() to allow existence check
            ("work", &paths.work),
            ("work/sessions", &paths.work.join("sessions")),
            ("work/executors", &paths.work.join("executors")),
            ("logs", &paths.logs),
            ("conf", &paths.conf),
            ("data", &paths.data),
            ("data/cache", &paths.cache),
            ("data/packages", &paths.data.join("packages")),
            ("migrations", &paths.migrations),
            ("migrations/sqlite", &paths.migrations.join("sqlite")),
        ] {
            if !path.exists() {
                fs::create_dir_all(path)
                    .context(format!("Failed to create directory: {}", name))?;
            }
        }

        // Set permissions
        self.set_directory_permissions(paths)?;

        println!(
            "✓ Created directory structure at: {}",
            paths.prefix.display()
        );
        Ok(())
    }

    fn set_directory_permissions(&self, paths: &InstallationPaths) -> Result<()> {
        // Set restrictive permissions on data directory
        let data_perms = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&paths.data, data_perms)
            .context("Failed to set data directory permissions")?;

        Ok(())
    }

    /// Install binaries to the target directory
    pub fn install_binaries(
        &self,
        artifacts: &BuildArtifacts,
        paths: &InstallationPaths,
        profiles: &[InstallProfile],
        force_overwrite: bool,
    ) -> Result<()> {
        println!("📦 Installing binaries...");

        // Check which components should be installed based on profiles
        let components_to_install = self.get_components_to_install(profiles);

        let all_binaries = [
            (
                "flame-session-manager",
                &artifacts.session_manager,
                paths.bin.join("flame-session-manager"),
            ),
            (
                "flame-executor-manager",
                &artifacts.executor_manager,
                paths.bin.join("flame-executor-manager"),
            ),
            ("flmctl", &artifacts.flmctl, paths.bin.join("flmctl")),
            ("flmadm", &artifacts.flmadm, paths.bin.join("flmadm")),
            ("flmping", &artifacts.flmping, paths.bin.join("flmping")),
            (
                "flmping-service",
                &artifacts.flmping_service,
                paths.bin.join("flmping-service"),
            ),
            ("flmexec", &artifacts.flmexec, paths.bin.join("flmexec")),
            (
                "flmexec-service",
                &artifacts.flmexec_service,
                paths.bin.join("flmexec-service"),
            ),
        ];

        for (name, src, dst) in all_binaries {
            // Skip components that are not in any of the selected profiles
            if !components_to_install.iter().any(|c| c == name) {
                println!("  ⊘ Skipped {} (not in selected profiles)", name);
                continue;
            }

            // Check if the file already exists
            if dst.exists() && !force_overwrite && !self.prompt_overwrite(name)? {
                println!("  ⊘ Skipped {} (already exists)", name);
                continue;
            }

            fs::copy(src, &dst).context(format!("Failed to copy {} binary", name))?;

            // Set executable permissions
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&dst, perms)
                .context(format!("Failed to set permissions on {}", name))?;

            println!("  ✓ Installed {}", name);
        }

        Ok(())
    }

    /// Get all components that should be installed based on the profiles
    fn get_components_to_install(&self, profiles: &[InstallProfile]) -> Vec<String> {
        let mut components = Vec::new();
        for profile in profiles {
            for component in profile.components() {
                let component_str = component.to_string();
                if !components.contains(&component_str) {
                    components.push(component_str);
                }
            }
        }
        components
    }

    /// Prompt the user whether to overwrite an existing file
    fn prompt_overwrite(&self, component: &str) -> Result<bool> {
        print!("  ⚠️  {} already exists. Overwrite? [y/N]: ", component);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let response = input.trim().to_lowercase();
        Ok(response == "y" || response == "yes")
    }

    /// Install Python SDK
    pub fn install_python_sdk(
        &self,
        src_dir: &Path,
        paths: &InstallationPaths,
        profiles: &[InstallProfile],
        force_overwrite: bool,
    ) -> Result<()> {
        // Check if any profile requires flamepy
        let components_to_install = self.get_components_to_install(profiles);
        if !components_to_install.iter().any(|c| c == "flamepy") {
            println!("⊘ Skipped Python SDK (not in selected profiles)");
            return Ok(());
        }

        println!("🐍 Installing Python SDK...");

        let sdk_src = src_dir.join("sdk/python");
        if !sdk_src.exists() {
            anyhow::bail!("Python SDK source not found at: {:?}", sdk_src);
        }

        // Check if SDK already exists
        if paths.sdk_python.exists() && !force_overwrite {
            print!(
                "  ⚠️  Python SDK already exists at {}. Overwrite? [y/N]: ",
                paths.sdk_python.display()
            );
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let response = input.trim().to_lowercase();
            if response != "y" && response != "yes" {
                println!("  ⊘ Skipped Python SDK (already exists)");
                return Ok(());
            }

            // Remove existing SDK before copying
            if paths.sdk_python.exists() {
                fs::remove_dir_all(&paths.sdk_python).context("Failed to remove existing SDK")?;
            }
        }

        // Copy SDK source to the installation directory, excluding development artifacts
        self.copy_sdk_excluding_artifacts(&sdk_src, &paths.sdk_python)
            .context("Failed to copy SDK to installation directory")?;

        println!("  ✓ Copied Python SDK to: {}", paths.sdk_python.display());

        // Build wheel for faster runtime loading
        // uv always rebuilds local directory dependencies, but wheel files are cached
        self.build_python_wheel(paths)?;

        Ok(())
    }

    /// Build Python wheel from SDK source
    fn build_python_wheel(&self, paths: &InstallationPaths) -> Result<()> {
        println!("  📦 Building Python wheel...");

        // Create wheels directory
        fs::create_dir_all(&paths.wheels).context("Failed to create wheels directory")?;

        // Find uv binary
        let uv_path = paths.bin.join("uv");
        if !uv_path.exists() {
            println!("  ⚠️  uv not found, skipping wheel build (will build at runtime)");
            return Ok(());
        }

        // Build wheel using uv
        let output = std::process::Command::new(&uv_path)
            .args([
                "build",
                "--wheel",
                "--out-dir",
                paths.wheels.to_str().unwrap(),
            ])
            .arg(&paths.sdk_python)
            .output()
            .context("Failed to execute uv build")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to build wheel: {}", stderr);
        }

        println!("  ✓ Built wheel to: {}", paths.wheels.display());
        Ok(())
    }

    /// Copy SDK directory while excluding development artifacts
    fn copy_sdk_excluding_artifacts(&self, src: &Path, dst: &Path) -> Result<()> {
        use walkdir::WalkDir;

        let exclude_patterns = [
            ".venv",
            "__pycache__",
            ".pytest_cache",
            ".pyc",
            ".pyo",
            ".eggs",
            ".egg-info",
            ".tox",
            ".coverage",
            ".mypy_cache",
            ".ruff_cache",
            "build",
            "dist",
        ];

        fs::create_dir_all(dst).context("Failed to create destination directory")?;

        for entry in WalkDir::new(src).into_iter().filter_entry(|e| {
            // Filter out directories and files matching exclude patterns
            let file_name = e.file_name().to_string_lossy();
            !exclude_patterns
                .iter()
                .any(|pattern| file_name.contains(pattern) || file_name == *pattern)
        }) {
            let entry = entry.context("Failed to read directory entry")?;
            let entry_path = entry.path();

            // Skip the source root itself
            if entry_path == src {
                continue;
            }

            // Calculate relative path and destination
            let relative_path = entry_path
                .strip_prefix(src)
                .context("Failed to strip prefix")?;
            let dst_path = dst.join(relative_path);

            if entry.file_type().is_dir() {
                fs::create_dir_all(&dst_path)
                    .context(format!("Failed to create directory {:?}", dst_path))?;
            } else {
                // Ensure parent directory exists
                if let Some(parent) = dst_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(entry_path, &dst_path)
                    .context(format!("Failed to copy {:?}", entry_path))?;
            }
        }

        Ok(())
    }

    /// Install database migrations
    pub fn install_migrations(
        &self,
        src_dir: &Path,
        paths: &InstallationPaths,
        profiles: &[InstallProfile],
    ) -> Result<()> {
        // Migrations are only needed for control plane
        if !profiles.contains(&InstallProfile::ControlPlane) {
            println!("⊘ Skipped database migrations (not in selected profiles)");
            return Ok(());
        }

        println!("🗄️  Installing database migrations...");

        let migrations_src = src_dir.join("session_manager/migrations/sqlite");
        if !migrations_src.exists() {
            anyhow::bail!("Migrations source not found at: {:?}", migrations_src);
        }

        // Copy all migration files
        for entry in fs::read_dir(&migrations_src).context("Failed to read migrations directory")? {
            let entry = entry.context("Failed to read migration file entry")?;
            let file_name = entry.file_name();
            let src_path = entry.path();
            let dst_path = paths.migrations.join("sqlite").join(&file_name);

            if src_path.is_file() {
                fs::copy(&src_path, &dst_path)
                    .context(format!("Failed to copy migration: {:?}", file_name))?;
            }
        }

        println!("✓ Installed migrations to: {}", paths.migrations.display());
        Ok(())
    }

    /// Install uv tool
    pub fn install_uv(&self, paths: &InstallationPaths, profiles: &[InstallProfile]) -> Result<()> {
        // UV is only needed for worker and client profiles
        let needs_uv = profiles.contains(&InstallProfile::Worker)
            || profiles.contains(&InstallProfile::Client);

        if !needs_uv {
            println!("⊘ Skipped uv installation (not in selected profiles)");
            return Ok(());
        }

        println!("🔧 Installing uv...");

        // Find uv in the system
        let uv_src = self.find_uv_executable().context(
            "uv not found in system. Please install uv first:\n\
             1. curl -LsSf https://astral.sh/uv/install.sh | sh\n\
             2. Or install via your package manager",
        )?;

        let uv_dst = paths.bin.join("uv");

        // Copy uv to installation directory
        fs::copy(&uv_src, &uv_dst).context("Failed to copy uv binary")?;

        // Set executable permissions
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&uv_dst, perms).context("Failed to set permissions on uv")?;

        println!("  ✓ Installed uv from {}", uv_src.display());
        Ok(())
    }

    /// Find uv executable in the system
    fn find_uv_executable(&self) -> Result<std::path::PathBuf> {
        use std::process::Command;

        // Try to find uv using 'which' command
        if let Ok(output) = Command::new("which").arg("uv").output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                let path = path_str.trim();
                if !path.is_empty() {
                    return Ok(std::path::PathBuf::from(path));
                }
            }
        }

        // Fallback: check common locations
        for common_path in [
            "/usr/bin/uv",
            "/usr/local/bin/uv",
            "/opt/homebrew/bin/uv", // macOS Homebrew
        ] {
            let path = std::path::Path::new(common_path);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        // Try to find in $HOME/.local/bin (common user install location)
        if let Ok(home) = std::env::var("HOME") {
            let user_uv = std::path::PathBuf::from(home).join(".local/bin/uv");
            if user_uv.exists() {
                return Ok(user_uv);
            }
        }

        anyhow::bail!("uv executable not found in system")
    }

    /// Remove the installation directory
    pub fn remove_installation(
        &self,
        paths: &InstallationPaths,
        preserve_data: bool,
        preserve_config: bool,
        preserve_logs: bool,
    ) -> Result<()> {
        println!("🗑️  Removing installation files...");

        // Remove binaries
        if paths.bin.exists() {
            fs::remove_dir_all(&paths.bin).context("Failed to remove bin directory")?;
            println!("  ✓ Removed binaries");
        }

        // Remove SDK
        if paths.sdk_python.parent().unwrap().exists() {
            fs::remove_dir_all(paths.sdk_python.parent().unwrap())
                .context("Failed to remove sdk directory")?;
            println!("  ✓ Removed Python SDK");
        }

        // Remove wheels
        if paths.wheels.exists() {
            fs::remove_dir_all(&paths.wheels).context("Failed to remove wheels directory")?;
            println!("  ✓ Removed wheels");
        }

        // Remove migrations
        if paths.migrations.exists() {
            fs::remove_dir_all(&paths.migrations)
                .context("Failed to remove migrations directory")?;
            println!("  ✓ Removed migrations");
        }

        // Remove work directory
        if paths.work.exists() {
            fs::remove_dir_all(&paths.work).context("Failed to remove work directory")?;
            println!("  ✓ Removed working directory");
        }

        // Remove events directory (session-manager creates this in prefix)
        let events_dir = paths.prefix.join("events");
        if events_dir.exists() {
            fs::remove_dir_all(&events_dir).context("Failed to remove events directory")?;
            println!("  ✓ Removed events directory");
        }

        // Remove data directory (unless preserved)
        if !preserve_data && paths.data.exists() {
            fs::remove_dir_all(&paths.data).context("Failed to remove data directory")?;
            println!("  ✓ Removed data directory");
        } else if preserve_data {
            println!("  ⚠️  Preserved data directory");
        }

        // Remove config directory (unless preserved)
        if !preserve_config && paths.conf.exists() {
            fs::remove_dir_all(&paths.conf).context("Failed to remove conf directory")?;
            println!("  ✓ Removed configuration directory");
        } else if preserve_config {
            println!("  ⚠️  Preserved configuration directory");
        }

        // Remove logs directory (unless preserved)
        if !preserve_logs && paths.logs.exists() {
            fs::remove_dir_all(&paths.logs).context("Failed to remove logs directory")?;
            println!("  ✓ Removed logs directory");
        } else if preserve_logs {
            println!("  ⚠️  Preserved logs directory");
        }

        // Try to remove prefix if empty
        if paths.prefix.exists() {
            match fs::remove_dir(&paths.prefix) {
                Ok(_) => println!(
                    "✓ Removed installation directory: {}",
                    paths.prefix.display()
                ),
                Err(_) => println!(
                    "  ⚠️  Installation directory not empty: {}",
                    paths.prefix.display()
                ),
            }
        }

        Ok(())
    }
}
