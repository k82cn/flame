use crate::managers::packager::Packager;
use anyhow::Result;
use std::path::PathBuf;

/// Configuration for the package command
#[derive(Debug, Clone)]
pub struct PackageConfig {
    pub src_dir: PathBuf,
    pub output: PathBuf,
    pub tag: String,
    pub verbose: bool,
    pub force: bool,
}

pub fn run(config: PackageConfig) -> Result<()> {
    println!("📦 Flame Package");
    println!("   Source: {}", config.src_dir.display());
    println!("   Output: {}", config.output.display());
    println!("   Version: {}", config.tag);
    println!();

    Packager::new(config.verbose).package(
        &config.src_dir,
        &config.output,
        &config.tag,
        config.force,
    )?;

    println!(
        "\nInstall it with: {}/package/sbin/flmadm install --all",
        config.output.display()
    );
    Ok(())
}
