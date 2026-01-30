use crate::types::{BuildArtifacts, InstallationPaths};
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub struct InstallationManager {
    user_manager: super::user::UserManager,
}

impl InstallationManager {
    pub fn new() -> Self {
        Self {
            user_manager: super::user::UserManager::new(),
        }
    }

    /// Create all required directories
    pub fn create_directories(&self, paths: &InstallationPaths) -> Result<()> {
        println!("📁 Creating directory structure...");

        for (name, path) in [
            ("bin", &paths.bin),
            ("sdk/python", &paths.sdk_python),
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

        // Set ownership if running as root
        if self.user_manager.is_root() {
            for path in [
                &paths.work,
                &paths.logs,
                &paths.data,
                &paths.conf,
                &paths.migrations,
            ] {
                self.user_manager.set_ownership(path)?;
            }

            // Set ownership for SDK directory parent (sdk/python directory will be set later)
            let sdk_parent = paths.sdk_python.parent().unwrap().parent().unwrap(); // ${PREFIX}/sdk
            if sdk_parent.exists() {
                self.user_manager.set_ownership(sdk_parent)?;
            }
        }

        Ok(())
    }

    /// Install binaries to the target directory
    pub fn install_binaries(
        &self,
        artifacts: &BuildArtifacts,
        paths: &InstallationPaths,
    ) -> Result<()> {
        println!("📦 Installing binaries...");

        for (name, src, dst) in [
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
        ] {
            fs::copy(src, &dst).context(format!("Failed to copy {} binary", name))?;

            // Set executable permissions
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&dst, perms)
                .context(format!("Failed to set permissions on {}", name))?;

            println!("  ✓ Installed {}", name);
        }

        // Set ownership if running as root
        if self.user_manager.is_root() {
            self.user_manager.set_ownership(&paths.bin)?;
        }

        Ok(())
    }

    /// Install Python SDK
    pub fn install_python_sdk(&self, src_dir: &Path, paths: &InstallationPaths) -> Result<()> {
        println!("🐍 Installing Python SDK...");

        let sdk_src = src_dir.join("sdk/python");
        if !sdk_src.exists() {
            anyhow::bail!("Python SDK source not found at: {:?}", sdk_src);
        }

        // Copy SDK source to the installation directory, excluding development artifacts
        // uv will use this directly with --with "flamepy @ file://..."
        self.copy_sdk_excluding_artifacts(&sdk_src, &paths.sdk_python)
            .context("Failed to copy SDK to installation directory")?;

        // Set ownership if running as root
        if self.user_manager.is_root() {
            self.user_manager.set_ownership(&paths.sdk_python)?;
        }

        println!("✓ Copied Python SDK to: {}", paths.sdk_python.display());

        // Create a note in the sdk_python directory for reference
        let readme_path = paths.sdk_python.join("README.txt");
        std::fs::write(
            &readme_path,
            "Python SDK source copied to this directory.\n\
             Applications use 'uv run --with \"flamepy @ file://...\"' to access it.\n\
             No separate installation required.\n",
        )
        .ok(); // Ignore errors for this informational file

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
    pub fn install_migrations(&self, src_dir: &Path, paths: &InstallationPaths) -> Result<()> {
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
