// flmadm is a Unix-only administration tool for installing and managing Flame
// on Linux servers. It manages systemd services, Unix permissions, and other
// Unix-specific functionality.

#[cfg(unix)]
mod commands;
#[cfg(unix)]
mod managers;
mod package;
#[cfg(unix)]
mod types;

#[cfg(unix)]
use clap::{CommandFactory, Parser, Subcommand};
#[cfg(unix)]
use clap_complete::{generate, Shell};
#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(unix)]
#[derive(Parser)]
#[command(name = "flmadm")]
#[command(version = VERSION)]
#[command(about = "Flame Administration Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[cfg(unix)]
#[derive(Subcommand)]
enum Commands {
    /// Build a release package from a source tree
    Package {
        /// Source code directory of Flame
        #[arg(long, default_value = ".", value_name = "PATH")]
        src_dir: PathBuf,

        /// Directory to write the release assets to: package/, the .tar.gz archive and the standalone
        /// flmadm, each with a .sha256 file
        #[arg(long, value_name = "PATH")]
        output: PathBuf,

        /// Version recorded in the package (default: the flmadm version)
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,

        /// Replace the output directory if it is not empty
        #[arg(long)]
        force: bool,

        /// Show detailed build output
        #[arg(long)]
        verbose: bool,
    },

    /// Install Flame from a release package on this machine
    Install {
        /// Release package to install: a package directory (or the directory a package was
        /// extracted into), a .tar.gz archive, or the http(s) URL of an archive (default: the
        /// package holding this flmadm)
        #[arg(long, value_name = "PATH|URL")]
        package: Option<String>,

        /// Target installation directory
        #[arg(long, default_value = "/usr/local/flame", value_name = "PATH")]
        prefix: PathBuf,

        /// Install control plane components (flame-session-manager, flmctl, flmadm)
        #[arg(long)]
        control_plane: bool,

        /// Install worker components (flame-executor-manager, flmping-service, flmexec-service, flmbox, flamepy)
        #[arg(long)]
        worker: bool,

        /// Install cache component (flame-object-cache)
        #[arg(long)]
        cache: bool,

        /// Install client components (flmctl, flmping, flmexec, flmbench, flamepy)
        #[arg(long)]
        client: bool,

        /// Install all components (control plane + worker + cache + client)
        #[arg(long)]
        all: bool,

        /// Install all supported examples under FLAME_HOME/examples.
        #[arg(long)]
        with_examples: bool,

        /// Skip systemd service generation
        #[arg(long)]
        no_systemd: bool,

        /// Enable and start systemd services after installation
        #[arg(long)]
        enable: bool,

        /// Remove existing installation before installing
        #[arg(long)]
        clean: bool,

        /// Force overwrite existing components without prompting
        #[arg(long)]
        force: bool,

        /// Python versions for SDK installation (can be specified multiple times)
        #[arg(long, action = clap::ArgAction::Append)]
        python_version: Vec<String>,
    },

    /// Uninstall Flame from this machine
    Uninstall {
        /// Installation directory to uninstall
        #[arg(long, default_value = "/usr/local/flame", value_name = "PATH")]
        prefix: PathBuf,

        /// Preserve data directory
        #[arg(long)]
        preserve_data: bool,

        /// Preserve configuration files
        #[arg(long)]
        preserve_config: bool,

        /// Preserve log files
        #[arg(long)]
        preserve_logs: bool,

        /// Custom backup directory
        #[arg(long, value_name = "PATH")]
        backup_dir: Option<PathBuf>,

        /// Do not create backup (permanently delete)
        #[arg(long)]
        no_backup: bool,

        /// Skip confirmation prompts
        #[arg(long)]
        force: bool,
    },
    /// Generate shell completion scripts
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[cfg(unix)]
fn main() {
    let _log_guard = match common::init_logger(None) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize logger: {}", e);
            std::process::exit(types::exit_codes::INSTALL_FAILURE);
        }
    };

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Package {
            src_dir,
            output,
            tag,
            force,
            verbose,
        } => commands::package::run(commands::package::PackageConfig {
            src_dir,
            output,
            tag: tag.unwrap_or_else(|| VERSION.to_string()),
            verbose,
            force,
        }),
        Commands::Install {
            package,
            prefix,
            control_plane,
            worker,
            cache,
            client,
            all,
            with_examples,
            no_systemd,
            enable,
            clean,
            force,
            python_version,
        } => {
            // Validate profile flags
            if all && (control_plane || worker || cache || client) {
                eprintln!(
                    "Error: --all cannot be used with --control-plane, --worker, --cache, or --client"
                );
                std::process::exit(types::exit_codes::INSTALL_FAILURE);
            }

            // Require explicit profile selection
            if !all && !control_plane && !worker && !cache && !client {
                eprintln!("Error: You must specify which components to install:");
                eprintln!(
                    "  --all              Install all components (control plane + worker + cache + client)"
                );
                eprintln!("  --control-plane    Install control plane components only");
                eprintln!("  --worker           Install worker components only");
                eprintln!("  --cache            Install cache component only");
                eprintln!("  --client           Install client components only");
                eprintln!("\nYou can also combine profiles, for example:");
                eprintln!("  --control-plane --worker    Install control plane and worker");
                eprintln!("  --worker --cache            Install worker with object cache");
                std::process::exit(types::exit_codes::INSTALL_FAILURE);
            }

            // Determine which profiles to install
            let profiles = if all {
                // Explicit --all flag: install all profiles
                vec![
                    types::InstallProfile::ControlPlane,
                    types::InstallProfile::Worker,
                    types::InstallProfile::Cache,
                    types::InstallProfile::Client,
                ]
            } else {
                // Specific profile flags specified
                let mut profiles = Vec::new();
                if control_plane {
                    profiles.push(types::InstallProfile::ControlPlane);
                }
                if worker {
                    profiles.push(types::InstallProfile::Worker);
                }
                if cache {
                    profiles.push(types::InstallProfile::Cache);
                }
                if client {
                    profiles.push(types::InstallProfile::Client);
                }
                profiles
            };

            let config = types::InstallConfig {
                package,
                prefix,
                systemd: !no_systemd,
                enable,
                clean,
                profiles,
                force_overwrite: force,
                python_versions: if python_version.is_empty() {
                    types::DEFAULT_PYTHON_VERSIONS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    python_version
                },
                with_examples,
            };
            commands::install::run(config)
        }
        Commands::Uninstall {
            prefix,
            preserve_data,
            preserve_config,
            preserve_logs,
            backup_dir,
            no_backup,
            force,
        } => {
            let config = types::UninstallConfig {
                prefix,
                preserve_data,
                preserve_config,
                preserve_logs,
                backup_dir,
                no_backup,
                force,
            };
            commands::uninstall::run(config)
        }
        Commands::Completion { shell } => {
            generate(shell, &mut Cli::command(), "flmadm", &mut io::stdout());
            Ok(())
        }
    };

    match result {
        Ok(_) => std::process::exit(types::exit_codes::SUCCESS),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(types::exit_codes::INSTALL_FAILURE);
        }
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("flmadm is only supported on Unix-like systems (Linux, macOS).");
    eprintln!(
        "It manages systemd services, Unix permissions, and other Unix-specific functionality."
    );
    std::process::exit(1);
}
