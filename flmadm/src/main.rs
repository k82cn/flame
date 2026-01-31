mod commands;
mod managers;
mod types;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "flmadm")]
#[command(version = VERSION)]
#[command(about = "Flame Administration Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install Flame on this machine
    Install {
        /// Source code directory for building Flame
        #[arg(long, value_name = "PATH")]
        src_dir: Option<PathBuf>,

        /// Target installation directory
        #[arg(long, default_value = "/usr/local/flame", value_name = "PATH")]
        prefix: PathBuf,

        /// Install control plane components (flame-session-manager, flmctl, flmadm)
        #[arg(long)]
        control_plane: bool,

        /// Install worker components (flame-executor-manager, flmping-service, flmexec-service, flamepy)
        #[arg(long)]
        worker: bool,

        /// Install client components (flmping, flmexec, flamepy)
        #[arg(long)]
        client: bool,

        /// Skip systemd service generation
        #[arg(long)]
        no_systemd: bool,

        /// Enable and start systemd services after installation
        #[arg(long)]
        enable: bool,

        /// Skip building from source (use pre-built binaries)
        #[arg(long)]
        skip_build: bool,

        /// Remove existing installation before installing
        #[arg(long)]
        clean: bool,

        /// Force overwrite existing components without prompting
        #[arg(long)]
        force: bool,

        /// Show detailed build output
        #[arg(long)]
        verbose: bool,
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
}

fn main() {
    // Initialize logging
    if let Err(e) = common::init_logger() {
        eprintln!("Failed to initialize logger: {}", e);
        std::process::exit(types::exit_codes::INSTALL_FAILURE);
    }

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Install {
            src_dir,
            prefix,
            control_plane,
            worker,
            client,
            no_systemd,
            enable,
            skip_build,
            clean,
            force,
            verbose,
        } => {
            // Determine which profiles to install
            let profiles = if control_plane || worker || client {
                // If any profile flag is specified, only install those profiles
                let mut profiles = Vec::new();
                if control_plane {
                    profiles.push(types::InstallProfile::ControlPlane);
                }
                if worker {
                    profiles.push(types::InstallProfile::Worker);
                }
                if client {
                    profiles.push(types::InstallProfile::Client);
                }
                profiles
            } else {
                // If no profile flags are specified, install all profiles (default behavior)
                vec![
                    types::InstallProfile::ControlPlane,
                    types::InstallProfile::Worker,
                    types::InstallProfile::Client,
                ]
            };

            let config = types::InstallConfig {
                src_dir,
                prefix,
                systemd: !no_systemd,
                enable,
                skip_build,
                clean,
                verbose,
                profiles,
                force_overwrite: force,
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
    };

    match result {
        Ok(_) => std::process::exit(types::exit_codes::SUCCESS),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(types::exit_codes::INSTALL_FAILURE);
        }
    }
}
