//! The release package: the extracted `flame-<tag>-linux-<arch>.tar.gz`.
//!
//! ```text
//! <package>/
//!   sbin/flmadm                  this installer
//!   bin/                         the Flame binaries and uv
//!   wheels/flamepy-*.whl         the Python SDK, already built
//!   common/migrations/sqlite/    database migrations
//!   config/applications/         application manifests
//! ```
//!
//! `./sbin/flmadm install` installs from the package that holds it, so nothing is built.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Whether `dir` is an extracted release package.
pub fn is_package(dir: &Path) -> bool {
    dir.join("sbin/flmadm").is_file()
        && dir.join("bin/flame-session-manager").is_file()
        && dir.join("wheels").is_dir()
}

/// The package the running `flmadm` belongs to, when it runs as `<package>/sbin/flmadm`.
pub fn current_package() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let package = exe.parent()?.parent()?;
    is_package(package).then(|| package.to_path_buf())
}

/// A package to install from. When it came from an archive, the directory it was extracted into
/// lives as long as this value does.
pub struct ResolvedPackage {
    dir: PathBuf,
    _extracted: Option<tempfile::TempDir>,
}

impl ResolvedPackage {
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// The package to install. `explicit` is a package directory, a `.tar.gz` archive of one, or the
/// `http(s)` URL of an archive (its `<url>.sha256` must verify it); without it, the package
/// holding the running `flmadm`.
pub fn resolve(explicit: Option<&str>) -> Result<ResolvedPackage> {
    let resolved = match explicit {
        None => {
            let dir = current_package().context(
                "flmadm installs from a release package. Run it from an extracted package \
                 (./sbin/flmadm install), pass --package with a package directory, archive or \
                 URL, or build a package from a source tree with `flmadm package`.",
            )?;
            ResolvedPackage {
                dir,
                _extracted: None,
            }
        }
        Some(spec) if is_url(spec) => {
            let temp = tempfile::tempdir().context("Failed to create a temporary directory")?;
            let archive = temp.path().join("package.tar.gz");
            println!("⬇️  Downloading {spec}...");
            download(spec, &archive)?;
            let checksum = temp.path().join("package.tar.gz.sha256");
            download(&format!("{spec}.sha256"), &checksum).context(
                "The package must be published with its .sha256 file, next to the archive",
            )?;
            verify_checksum(&archive, &fs::read_to_string(&checksum)?)?;
            extract(&archive, temp)?
        }
        Some(spec) => {
            let path = Path::new(spec);
            if path.is_dir() {
                let dir = package_in(path).with_context(|| {
                    format!(
                        "{spec} is not a release package (expected sbin/flmadm, bin/ and wheels/, \
                         here or in its one subdirectory). Build one from a source tree with \
                         `flmadm package`."
                    )
                })?;
                ResolvedPackage {
                    dir,
                    _extracted: None,
                }
            } else if path.is_file() {
                // A checksum file next to the archive is verified; a bare archive is trusted.
                let checksum = path.with_file_name(format!(
                    "{}.sha256",
                    path.file_name().unwrap().to_string_lossy()
                ));
                if checksum.is_file() {
                    verify_checksum(path, &fs::read_to_string(&checksum)?)?;
                }
                let temp = tempfile::tempdir().context("Failed to create a temporary directory")?;
                extract(path, temp)?
            } else {
                anyhow::bail!("--package {spec}: not a directory, an archive, or an http(s) URL");
            }
        }
    };
    println!("✓ Using release package: {}", resolved.dir().display());
    Ok(resolved)
}

/// `dir` when it is a package, else its one subdirectory that is: an archive extracts into one
/// directory (`flame-<tag>-<os>-<arch>/`), and `flmadm package` writes `package/`, so the directory
/// they were extracted or written into names the package too.
fn package_in(dir: &Path) -> Option<PathBuf> {
    if is_package(dir) {
        return Some(dir.to_path_buf());
    }
    let mut subdirs = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir());
    let only = subdirs.next()?;
    (subdirs.next().is_none() && is_package(&only)).then_some(only)
}

fn is_url(spec: &str) -> bool {
    spec.starts_with("http://") || spec.starts_with("https://")
}

fn download(url: &str, destination: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--retry",
            "3",
            "--output",
        ])
        .arg(destination)
        .arg(url)
        .status()
        .context("Failed to run curl; it is needed to download a package")?;
    if !status.success() {
        anyhow::bail!("Failed to download {url}");
    }
    Ok(())
}

/// Check `archive` against the first field of a `sha256sum` line.
fn verify_checksum(archive: &Path, checksum_file: &str) -> Result<()> {
    let expected = checksum_file
        .split_whitespace()
        .next()
        .context("The checksum file is empty")?
        .to_lowercase();
    let actual = format!(
        "{:x}",
        Sha256::digest(fs::read(archive).context("Failed to read the archive")?)
    );
    if expected != actual {
        anyhow::bail!("Checksum mismatch for the package: expected {expected}, got {actual}");
    }
    Ok(())
}

/// Extract `archive` into `temp` and find the package directory it holds.
fn extract(archive: &Path, temp: tempfile::TempDir) -> Result<ResolvedPackage> {
    let file =
        fs::File::open(archive).with_context(|| format!("Failed to open {}", archive.display()))?;
    tar::Archive::new(GzDecoder::new(file))
        .unpack(temp.path())
        .with_context(|| format!("Failed to extract {}", archive.display()))?;

    match package_in(temp.path()) {
        Some(dir) => Ok(ResolvedPackage {
            dir,
            _extracted: Some(temp),
        }),
        None => anyhow::bail!(
            "{} does not hold a release package (one directory with sbin/flmadm, bin/ and wheels/)",
            archive.display()
        ),
    }
}

/// The Python SDK wheel of a package.
pub fn wheel(package: &Path) -> Result<PathBuf> {
    let dir = package.join("wheels");
    let mut wheels = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "whl") {
            wheels.push(path);
        }
    }
    match wheels.len() {
        1 => Ok(wheels.remove(0)),
        count => anyhow::bail!(
            "Expected one Python SDK wheel in {}, found {}",
            dir.display(),
            count
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_package(dir: &Path) {
        for file in ["sbin/flmadm", "bin/flame-session-manager"] {
            fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
            fs::write(dir.join(file), "").unwrap();
        }
        fs::create_dir_all(dir.join("wheels")).unwrap();
    }

    #[test]
    fn detects_a_package_layout() {
        let temp = tempdir().unwrap();
        assert!(!is_package(temp.path()));

        make_package(temp.path());
        assert!(is_package(temp.path()));
    }

    #[test]
    fn a_source_tree_is_not_a_package() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("target/release")).unwrap();
        fs::write(temp.path().join("Cargo.toml"), "").unwrap();

        assert!(!is_package(temp.path()));
    }

    fn spec(path: &Path) -> String {
        path.to_str().unwrap().to_string()
    }

    /// An archive of `dir` whose top-level directory is `name`.
    fn archive(dir: &Path, name: &str, to: &Path) {
        let file = fs::File::create(to).unwrap();
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        builder.append_dir_all(name, dir).unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn resolves_an_explicit_directory_only_when_it_is_a_package() {
        let temp = tempdir().unwrap();
        let error = resolve(Some(&spec(temp.path()))).err().unwrap();
        assert!(error.to_string().contains("is not a release package"));

        make_package(temp.path());
        assert_eq!(
            resolve(Some(&spec(temp.path()))).unwrap().dir(),
            temp.path()
        );
    }

    #[test]
    fn resolves_the_directory_an_archive_was_extracted_into() {
        let temp = tempdir().unwrap();
        let extracted = temp.path().join("extracted");
        let package = extracted.join("flame-v1-linux-amd64");
        make_package(&package);

        // The extraction directory names the package, as does the package directory itself.
        assert_eq!(resolve(Some(&spec(&extracted))).unwrap().dir(), package);
        assert_eq!(resolve(Some(&spec(&package))).unwrap().dir(), package);

        // Two candidates, or none, are ambiguous.
        make_package(&extracted.join("other"));
        assert!(resolve(Some(&spec(&extracted))).is_err());
        assert!(resolve(Some(&spec(&temp.path().join("extracted/other/bin")))).is_err());
    }

    #[test]
    fn resolves_an_archive_and_verifies_its_checksum_when_there_is_one() {
        let temp = tempdir().unwrap();
        let package = temp.path().join("package");
        make_package(&package);
        let archive_path = temp.path().join("flame-v1-linux-amd64.tar.gz");
        archive(&package, "flame-v1-linux-amd64", &archive_path);

        // A bare archive is extracted and the package inside is found.
        let resolved = resolve(Some(&spec(&archive_path))).unwrap();
        assert!(is_package(resolved.dir()));
        assert!(resolved.dir().ends_with("flame-v1-linux-amd64"));

        // A checksum next to it must match.
        let digest = format!("{:x}", Sha256::digest(fs::read(&archive_path).unwrap()));
        let checksum = temp.path().join("flame-v1-linux-amd64.tar.gz.sha256");
        fs::write(
            &checksum,
            format!("{digest}  flame-v1-linux-amd64.tar.gz\n"),
        )
        .unwrap();
        assert!(resolve(Some(&spec(&archive_path))).is_ok());

        fs::write(&checksum, format!("{}  x\n", "0".repeat(64))).unwrap();
        let error = resolve(Some(&spec(&archive_path))).err().unwrap();
        assert!(error.to_string().contains("Checksum mismatch"));
    }

    #[test]
    fn an_archive_without_a_package_is_rejected() {
        let temp = tempdir().unwrap();
        let not_a_package = temp.path().join("stuff");
        fs::create_dir_all(&not_a_package).unwrap();
        fs::write(not_a_package.join("file"), "").unwrap();
        let archive_path = temp.path().join("stuff.tar.gz");
        archive(&not_a_package, "stuff", &archive_path);

        let error = resolve(Some(&spec(&archive_path))).err().unwrap();

        assert!(error
            .to_string()
            .contains("does not hold a release package"));
    }

    #[test]
    fn rejects_a_path_that_does_not_exist() {
        let error = resolve(Some("/nonexistent/flame.tar.gz")).err().unwrap();

        assert!(error
            .to_string()
            .contains("not a directory, an archive, or an http(s) URL"));
    }

    #[test]
    fn tells_urls_from_paths() {
        assert!(is_url("https://example.com/flame.tar.gz"));
        assert!(is_url("http://example.com/flame.tar.gz"));
        assert!(!is_url("/tmp/flame.tar.gz"));
        assert!(!is_url("ftp://example.com/flame.tar.gz"));
    }

    #[test]
    fn checksum_verification_reads_the_first_field_of_a_sha256sum_line() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("a");
        fs::write(&file, "content").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"content"));

        assert!(verify_checksum(&file, &format!("{digest}  a\n")).is_ok());
        assert!(verify_checksum(&file, &format!("{}  a\n", digest.to_uppercase())).is_ok());
        assert!(verify_checksum(&file, "").is_err());
        assert!(verify_checksum(&file, &format!("{}  a\n", "1".repeat(64))).is_err());
    }

    #[test]
    fn finds_the_single_wheel() {
        let temp = tempdir().unwrap();
        make_package(temp.path());
        assert!(wheel(temp.path()).is_err());

        let wheel_path = temp.path().join("wheels/flamepy-0.7.0-py3-none-any.whl");
        fs::write(&wheel_path, "").unwrap();
        assert_eq!(wheel(temp.path()).unwrap(), wheel_path);

        fs::write(temp.path().join("wheels/other-1.0-py3-none-any.whl"), "").unwrap();
        assert!(wheel(temp.path()).is_err());
    }
}
