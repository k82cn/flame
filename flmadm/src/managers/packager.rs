//! Assemble a release package from a source tree (see `crate::package` for the layout).
//!
//! `flmadm package` builds the workspace, builds the Python SDK wheel, and lays out everything
//! `flmadm install` needs, so that an installation never builds anything: a package is the only
//! thing `flmadm install` installs from.

use crate::managers::build::BuildManager;
use crate::managers::installation::built_wheel;
use crate::types::{available_examples, BuildArtifacts};
use anyhow::{Context, Result};
use flate2::{write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Packager {
    verbose: bool,
}

impl Packager {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Build everything in `src_dir` and write the release assets to `output`:
    ///
    /// - `package/`, the package directory (see `crate::package`);
    /// - `flame-<tag>-<os>-<arch>.tar.gz`, that package as one archive, and
    /// - `flmadm-<tag>-<os>-<arch>`, the installer by itself,
    ///
    /// each with a `.sha256` file.
    pub fn package(&self, src_dir: &Path, output: &Path, tag: &str, force: bool) -> Result<()> {
        validate_source_dir(src_dir)?;
        prepare_output(output, force)?;

        // The package is always built from the source: cargo reuses what is already up to date,
        // so building again costs little when the workspace was just built.
        let build_manager = BuildManager::new(self.verbose);
        build_manager.check_prerequisites()?;
        let artifacts = build_manager.build_all(src_dir)?;

        let package = output.join("package");
        println!("📦 Assembling the package at {}...", package.display());
        copy_binaries(&artifacts, &package)?;
        let uv = install_uv(&package)?;
        build_wheel(&uv, src_dir, &package)?;
        copy_tree(
            &src_dir.join("common/migrations/sqlite"),
            &package.join("common/migrations/sqlite"),
        )?;
        copy_tree(
            &src_dir.join("config/applications"),
            &package.join("config/applications"),
        )?;
        copy_examples(src_dir, &package)?;
        copy_file(&src_dir.join("LICENSE"), &package.join("LICENSE"), 0o644)?;
        fs::write(package.join("VERSION"), format!("{tag}\n"))
            .context("Failed to write VERSION")?;

        if !crate::package::is_package(&package) {
            anyhow::bail!("Assembled an incomplete package at {}", package.display());
        }
        println!("✓ Package ready: {}", package.display());

        let archive = archive_package(&package, tag, output)?;
        println!("✓ Archive ready: {}", archive.display());
        let flmadm = release_flmadm(&package, tag, output)?;
        println!("✓ Installer ready: {}", flmadm.display());
        Ok(())
    }
}

/// The platform part of a release asset name, `<tag>-<os>-<arch>`.
fn asset_suffix(tag: &str) -> Result<String> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => anyhow::bail!("Unsupported architecture: {other}"),
    };
    Ok(format!("{tag}-{}-{arch}", std::env::consts::OS))
}

/// The name of a package for this machine, `flame-<tag>-<os>-<arch>`.
fn package_name(tag: &str) -> Result<String> {
    Ok(format!("flame-{}", asset_suffix(tag)?))
}

/// Archive `package` as `<archive_dir>/<name>.tar.gz`, one top-level directory called `<name>`,
/// and write `<name>.tar.gz.sha256` next to it. Ownership and times are normalized, so the same
/// package gives the same archive.
fn archive_package(package: &Path, tag: &str, archive_dir: &Path) -> Result<PathBuf> {
    let name = package_name(tag)?;
    fs::create_dir_all(archive_dir)
        .with_context(|| format!("Failed to create {}", archive_dir.display()))?;
    let archive = archive_dir.join(format!("{name}.tar.gz"));

    let file = fs::File::create(&archive)
        .with_context(|| format!("Failed to create {}", archive.display()))?;
    let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::default()));
    builder.mode(tar::HeaderMode::Deterministic);
    builder
        .append_dir_all(&name, package)
        .context("Failed to add the package to the archive")?;
    builder
        .into_inner()
        .and_then(|encoder| encoder.finish())
        .context("Failed to finish the archive")?;

    write_checksum(&archive)?;
    Ok(archive)
}

/// The installer by itself, `flmadm-<tag>-<os>-<arch>`: it installs a package from a URL or a file,
/// so it is the one thing to download first.
fn release_flmadm(package: &Path, tag: &str, dir: &Path) -> Result<PathBuf> {
    let flmadm = dir.join(format!("flmadm-{}", asset_suffix(tag)?));
    copy_file(&package.join("sbin/flmadm"), &flmadm, 0o755)?;
    write_checksum(&flmadm)?;
    Ok(flmadm)
}

/// Write `<file>.sha256` in the format `sha256sum -c` reads.
fn write_checksum(file: &Path) -> Result<()> {
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    let digest = Sha256::digest(fs::read(file).with_context(|| format!("Failed to read {name}"))?);
    fs::write(
        file.with_file_name(format!("{name}.sha256")),
        format!("{digest:x}  {name}\n"),
    )
    .context("Failed to write the checksum")
}

fn validate_source_dir(dir: &Path) -> Result<()> {
    if !dir.join("Cargo.toml").exists() {
        anyhow::bail!(
            "Not a valid Flame source directory (missing Cargo.toml): {:?}",
            dir
        );
    }
    for component in [
        "session_manager",
        "executor_manager",
        "flmctl",
        "sdk/python",
        "common/migrations/sqlite",
        "config/applications",
    ] {
        if !dir.join(component).exists() {
            anyhow::bail!("Missing required component: {}", component);
        }
    }
    println!("✓ Using source directory: {}", dir.display());
    Ok(())
}

fn prepare_output(output: &Path, force: bool) -> Result<()> {
    if output.exists() && output.read_dir()?.next().is_some() {
        if !force {
            anyhow::bail!(
                "Output directory {} is not empty; use --force to replace it",
                output.display()
            );
        }
        fs::remove_dir_all(output).context("Failed to clear the output directory")?;
    }
    fs::create_dir_all(output).context("Failed to create the output directory")
}

fn copy_binaries(artifacts: &BuildArtifacts, output: &Path) -> Result<()> {
    for (name, src) in artifacts.entries() {
        let dst = if name == "flmadm" {
            output.join("sbin").join(name)
        } else {
            output.join("bin").join(name)
        };
        copy_file(src, &dst, 0o755)?;
    }
    Ok(())
}

/// Bundle the uv that builds the wheel here and installs it on the target; returns its path.
fn install_uv(output: &Path) -> Result<PathBuf> {
    let uv_src = find_uv_executable().context(
        "uv not found in system. Please install uv first:\n\
         1. curl -LsSf https://astral.sh/uv/install.sh | sh\n\
         2. Or install via your package manager",
    )?;
    let uv = output.join("bin/uv");
    copy_file(&uv_src, &uv, 0o755)?;
    println!("  ✓ Bundled uv from {}", uv_src.display());
    Ok(uv)
}

fn build_wheel(uv: &Path, src_dir: &Path, output: &Path) -> Result<()> {
    let wheels = output.join("wheels");
    fs::create_dir_all(&wheels).context("Failed to create wheels directory")?;
    println!("  📦 Building Python wheel...");
    let build = Command::new(uv)
        .args(["build", "--wheel", "--out-dir"])
        .arg(&wheels)
        .arg(src_dir.join("sdk/python"))
        .output()
        .context("Failed to execute uv build")?;
    if !build.status.success() {
        anyhow::bail!(
            "Failed to build wheel: {}",
            String::from_utf8_lossy(&build.stderr)
        );
    }
    // uv leaves a .gitignore next to the wheel; the wheel must be the only file.
    let _ = fs::remove_file(wheels.join(".gitignore"));
    let wheel = built_wheel(&wheels)?;
    println!("  ✓ Built wheel: {}", wheel.display());
    Ok(())
}

/// The example binaries and source assets, laid out as they are installed under FLAME_HOME/examples.
pub(crate) fn copy_examples(src_dir: &Path, output: &Path) -> Result<()> {
    let target_dir = src_dir.join("target/release");
    for example in available_examples() {
        let dst_dir = output.join("examples").join(example.relative_dir);
        for binary in example.binaries {
            let src = target_dir.join(binary);
            if !src.exists() {
                anyhow::bail!(
                    "Example binary not found: {}. Build package '{}' first.",
                    src.display(),
                    example.package
                );
            }
            copy_file(&src, &dst_dir.join(binary), 0o755)?;
        }
        let src_example_dir = src_dir.join("examples").join(example.relative_dir);
        for asset in example.assets {
            let src = src_example_dir.join(asset);
            if !src.exists() {
                anyhow::bail!(
                    "Example asset not found: {} for example '{}'.",
                    src.display(),
                    example.name
                );
            }
            let mode = if asset.ends_with(".sh") { 0o755 } else { 0o644 };
            copy_file(&src, &dst_dir.join(asset), mode)?;
        }
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path, mode: u32) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::copy(src, dst).with_context(|| format!("Failed to copy {}", src.display()))?;
    fs::set_permissions(dst, fs::Permissions::from_mode(mode))
        .with_context(|| format!("Failed to set permissions on {}", dst.display()))
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("Failed to create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("Failed to read {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else {
            copy_file(&path, &target, 0o644)?;
        }
    }
    Ok(())
}

/// Find the uv executable on this machine.
pub fn find_uv_executable() -> Result<PathBuf> {
    if let Ok(output) = Command::new("which").arg("uv").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            let path = path.trim();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    for common_path in [
        "/usr/bin/uv",
        "/usr/local/bin/uv",
        "/opt/homebrew/bin/uv", // macOS Homebrew
    ] {
        let path = Path::new(common_path);
        if path.exists() {
            return Ok(path.to_path_buf());
        }
    }

    // $HOME/.local/bin is a common user install location.
    if let Ok(home) = std::env::var("HOME") {
        let user_uv = PathBuf::from(home).join(".local/bin/uv");
        if user_uv.exists() {
            return Ok(user_uv);
        }
    }

    anyhow::bail!("uv executable not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_a_directory_that_is_not_a_source_tree() {
        let temp = tempdir().unwrap();

        let error = validate_source_dir(temp.path()).unwrap_err();

        assert!(error.to_string().contains("missing Cargo.toml"));
    }

    #[test]
    fn requires_the_files_a_package_carries() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "").unwrap();
        for dir in [
            "session_manager",
            "executor_manager",
            "flmctl",
            "sdk/python",
        ] {
            fs::create_dir_all(temp.path().join(dir)).unwrap();
        }

        let error = validate_source_dir(temp.path()).unwrap_err();
        assert!(error.to_string().contains("common/migrations/sqlite"));

        fs::create_dir_all(temp.path().join("common/migrations/sqlite")).unwrap();
        fs::create_dir_all(temp.path().join("config/applications")).unwrap();
        validate_source_dir(temp.path()).unwrap();
    }

    #[test]
    fn output_directory_must_be_empty_unless_forced() {
        let temp = tempdir().unwrap();
        let output = temp.path().join("package");
        prepare_output(&output, false).unwrap();
        assert!(output.is_dir());

        fs::write(output.join("stale"), "").unwrap();
        assert!(prepare_output(&output, false).is_err());
        assert!(output.join("stale").exists());

        prepare_output(&output, true).unwrap();
        assert!(output.is_dir());
        assert!(!output.join("stale").exists());
    }

    #[test]
    fn archives_the_package_under_one_named_directory_with_a_checksum() {
        use std::io::Read;

        let temp = tempdir().unwrap();
        let package = temp.path().join("package");
        copy_file_with(&package.join("sbin/flmadm"), "flmadm", 0o755);
        copy_file_with(&package.join("VERSION"), "v1.2.3\n", 0o644);

        let archive_dir = temp.path().join("dist");
        let archive = archive_package(&package, "v1.2.3", &archive_dir).unwrap();

        let name = package_name("v1.2.3").unwrap();
        assert_eq!(archive, archive_dir.join(format!("{name}.tar.gz")));
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(
            fs::File::open(&archive).unwrap(),
        ));
        let mut files = Vec::new();
        for entry in tar.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().display().to_string();
            let mode = entry.header().mode().unwrap();
            let uid = entry.header().uid().unwrap();
            let mut content = String::new();
            entry.read_to_string(&mut content).unwrap();
            files.push((path, mode, uid, content));
        }
        assert!(files
            .iter()
            .all(|(path, _, uid, _)| path.starts_with(&name) && *uid == 0));
        let flmadm = files
            .iter()
            .find(|(path, ..)| path == &format!("{name}/sbin/flmadm"))
            .unwrap();
        assert_eq!(flmadm.1 & 0o111, 0o111);
        assert_eq!(flmadm.3, "flmadm");

        let checksum =
            fs::read_to_string(archive_dir.join(format!("{name}.tar.gz.sha256"))).unwrap();
        let digest = Sha256::digest(fs::read(&archive).unwrap());
        assert_eq!(checksum, format!("{digest:x}  {name}.tar.gz\n"));
    }

    #[test]
    fn publishes_the_installer_by_itself_with_a_checksum() {
        let temp = tempdir().unwrap();
        let package = temp.path().join("package");
        copy_file_with(&package.join("sbin/flmadm"), "installer", 0o755);

        let flmadm = release_flmadm(&package, "v1.2.3", temp.path()).unwrap();

        let name = format!("flmadm-{}", asset_suffix("v1.2.3").unwrap());
        assert_eq!(flmadm, temp.path().join(&name));
        assert_eq!(fs::read_to_string(&flmadm).unwrap(), "installer");
        assert_eq!(
            fs::metadata(&flmadm).unwrap().permissions().mode() & 0o111,
            0o111
        );
        let digest = Sha256::digest(b"installer");
        assert_eq!(
            fs::read_to_string(temp.path().join(format!("{name}.sha256"))).unwrap(),
            format!("{digest:x}  {name}\n")
        );
    }

    fn copy_file_with(path: &Path, content: &str, mode: u32) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn copies_a_tree_with_nested_directories() {
        let temp = tempdir().unwrap();
        let src = temp.path().join("src");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("a.yaml"), "a").unwrap();
        fs::write(src.join("nested/b.sql"), "b").unwrap();

        let dst = temp.path().join("dst");
        copy_tree(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("a.yaml")).unwrap(), "a");
        assert_eq!(fs::read_to_string(dst.join("nested/b.sql")).unwrap(), "b");
    }
}
