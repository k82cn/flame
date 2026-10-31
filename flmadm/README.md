# flmadm - Flame Administration Tool

`flmadm` is the administration CLI for Flame, designed for installing, configuring, and managing Flame clusters on bare-metal servers or VMs.

## Overview

Flame provides two separate CLI tools:
- **`flmctl`**: User-facing CLI for submitting jobs and managing sessions
- **`flmadm`**: Administrator CLI for installation and cluster management

## Features

- **Automated Installation**: One-command installation of all Flame components
- **Release Packages**: Installs only from a release package (`flmadm package` builds one from source, and every GitHub release publishes one)
- **Python SDK Installation**: Automatically installs the Flame Python SDK
- **Systemd Integration**: Generates and manages systemd service files
- **Configuration Generation**: Creates default configuration files
- **Clean Uninstallation**: Safely removes Flame with backup support
- **Flexible Installation**: Supports both system-wide (root) and user-local installations
- **Installation Profiles**: Install specific components (control-plane, worker, client)

## Installation

### From a release package

Each GitHub release has, for `linux-amd64` and `linux-arm64`, a `flame-<tag>-linux-<arch>.tar.gz`
with every Flame binary, `uv`, the Python SDK wheel, the database migrations, the application
manifests and the examples, and `flmadm-<tag>-linux-<arch>`, the installer by itself. Each has a
`.sha256` file. Download the installer and point it at the archive's URL; it downloads the archive,
verifies it against the `.sha256` published next to it, and installs it. Nothing is built:

```bash
base=https://github.com/xflops/flame/releases/download/v0.7.0
curl -fLO $base/flmadm-v0.7.0-linux-amd64
chmod +x flmadm-v0.7.0-linux-amd64
sudo ./flmadm-v0.7.0-linux-amd64 install --all \
    --package $base/flame-v0.7.0-linux-amd64.tar.gz
```

`--package` also takes a downloaded archive (a `.sha256` next to it is checked) or a directory: an
extracted package, or the directory it was extracted into (it holds the one package directory).
Without it `flmadm` installs the package that holds it:

```bash
tar -xzf flame-v0.7.0-linux-amd64.tar.gz
sudo ./flame-v0.7.0-linux-amd64/sbin/flmadm install --all
```

The options of [Install Options](#install-options) apply. The Python SDK is installed from the wheel
in the package, so the install needs network access to PyPI for its dependencies, and downloading
needs `curl`.

### From source

`flmadm` always installs from a package, so build one first. `flmadm package` builds everything: the
whole workspace (examples included), the Python SDK wheel, and the bundled `uv`. It writes these
release assets to `--output`, each archive and installer with a `.sha256` file:

- `package/`, the package directory;
- `flame-<tag>-<os>-<arch>.tar.gz`, that package as one archive (always written), and
- `flmadm-<tag>-<os>-<arch>`, the installer by itself.

`--tag` sets the version recorded in the package and used in the names. Build `flmadm` itself to run
it:

```bash
cd /path/to/flame
cargo build --release -p flmadm
./target/release/flmadm package --src-dir . --output /tmp/flame-package
sudo /tmp/flame-package/package/sbin/flmadm install --all
```

`flmadm package` needs the Rust toolchain and `uv` on this machine.

## Usage

### Install Flame

**Basic installation (all components):**
```bash
sudo flmadm install --all
```

**Install from a package built from local source:**
```bash
/path/to/flame/target/release/flmadm package --src-dir /path/to/flame --output /tmp/flame-package
sudo /tmp/flame-package/package/sbin/flmadm install --all
```

**Custom installation directory:**
```bash
sudo flmadm install --all --prefix /opt/flame
```

**Install and start services:**
```bash
sudo flmadm install --all --enable
```

**User-local installation (no systemd):**
```bash
flmadm install --all --no-systemd --prefix ~/flame
```

**Clean install (backup and reinstall):**
```bash
sudo flmadm install --all --clean --enable
```

**Install specific profiles:**
```bash
# Install all components explicitly (same as default)
sudo flmadm install --all

# Control plane only (session manager, flmctl, flmadm)
sudo flmadm install --control-plane

# Worker only (executor manager, services, SDK)
sudo flmadm install --worker

# Client only (flmping, flmexec, SDK)
sudo flmadm install --client

# Combined installation (control plane + worker on same node)
sudo flmadm install --control-plane --worker --enable

# Force overwrite existing components
sudo flmadm install --worker --force

# Client installation (no systemd needed)
flmadm install --client --prefix ~/flame

# Client installation with all example binaries
flmadm install --client --with-examples --prefix ~/flame

# ERROR: Cannot use --enable with client-only
# flmadm install --client --enable  # This will fail with an error

# ERROR: Cannot use --all with specific profiles
# sudo flmadm install --all --control-plane  # This will fail with an error
```

### Uninstall Flame

**Basic uninstall (with backup):**
```bash
sudo flmadm uninstall
```

**Uninstall and preserve data:**
```bash
sudo flmadm uninstall --preserve-data --preserve-config
```

**Uninstall from custom location:**
```bash
sudo flmadm uninstall --prefix /opt/flame
```

**Complete removal (no backup):**
```bash
sudo flmadm uninstall --no-backup --force
```

**Custom backup location:**
```bash
sudo flmadm uninstall --backup-dir /backups/flame-backup-2026-01-28
```

## Install Options

- `--package <PATH|URL>`: Release package to install: a package directory (or the directory a package was extracted into), a `.tar.gz` archive, or the `http(s)` URL of an archive, which must be published with its `.sha256` (default: the package holding the running `flmadm`)
- `--prefix <PATH>`: Target installation directory (default: `/usr/local/flame`)
- `--all`: Explicitly install all components (control plane + worker + cache + client)
- `--control-plane`: Install control plane components only (flame-session-manager, flmctl, flmadm)
- `--worker`: Install worker components only (flame-executor-manager, flmping-service, flmexec-service, flmbox, flamepy)
- `--cache`: Install cache component only (flame-object-cache)
- `--client`: Install client components only (flmping, flmexec, flamepy)
- `--with-examples`: Install all supported example binaries and Python sources under `${PREFIX}/examples`, including vLLM (the package carries them)
- `--no-systemd`: Skip systemd service generation (for user-local installs)
- `--enable`: Enable and start systemd services after installation
- `--clean`: Remove existing installation before installing (creates backup)
- `--force`: Force overwrite existing components without prompting

**Note:** 
- You **must** specify at least one profile flag (`--all`, `--control-plane`, `--worker`, `--cache`, or `--client`)
- The `--all` flag cannot be combined with `--control-plane`, `--worker`, `--cache`, or `--client`
- Profile flags can be combined (e.g., `--control-plane --worker --cache` for a combined node)

## Installation Profiles

Flame supports four installation profiles to allow flexible deployment architectures:

### Control Plane Profile (`--control-plane`)

Installs components required for cluster management:
- `flame-session-manager`: Main control plane service
- `flmctl`: CLI for job submission and management
- `flmadm`: Administration CLI

**Use case:** Deploy on dedicated control plane nodes that manage the cluster but don't execute workloads.

### Worker Profile (`--worker`)

Installs components required for executing workloads:
- `flame-executor-manager`: Worker node service
- `flmping-service`: Health check service
- `flmexec-service`: Execution service
- `flmbox`: Sandbox CLI and service (`flmbox -d`); gVisor `runsc` must be installed on the worker
- `flamepy`: Python SDK

**Use case:** Deploy on worker nodes that execute user workloads.

### Cache Profile (`--cache`)

Installs the standalone object cache:
- `flame-object-cache`: Dedicated object cache server

**Use case:** Deploy on nodes that provide object caching for the cluster. Can be co-located with workers or run as a dedicated service.

### Client Profile (`--client`)

Installs client tools for submitting jobs:
- `flmctl`: CLI for job submission and session management
- `flmping`: CLI for health checks
- `flmexec`: CLI for job execution
- `flamepy`: Python SDK

**Use case:** Deploy on client machines or jump hosts for users to submit jobs without running services.

**Note:** The client profile doesn't install any systemd services. The `--enable` flag is not applicable when only `--client` is specified. The `--no-systemd` flag is automatically implied for client-only installations.

### Combined Deployments

Profiles can be combined in a single installation:

```bash
# Single-node deployment (all-in-one)
sudo flmadm install --control-plane --worker --cache --enable

# Control plane + client tools
sudo flmadm install --control-plane --client

# Worker + cache (typical worker node setup)
sudo flmadm install --worker --cache --enable
```

### Component Overwrite Behavior

When installing over an existing installation:
- Without `--force`: Prompts for confirmation before overwriting each existing component
- With `--force`: Automatically overwrites all components without prompting
- With `--clean`: Backs up and removes the entire installation before installing

Cluster configuration and application manifests are preserved on reinstall,
including with `--force`. Use `--clean` to restore the shipped application
manifests, or edit `${PREFIX}/conf/applications/*.yaml` to customize the
applications registered by session manager.

## Uninstall Options

- `--prefix <PATH>`: Installation directory to uninstall (default: `/usr/local/flame`)
- `--preserve-data`: Preserve data directory (useful for reinstallation)
- `--preserve-config`: Preserve configuration files
- `--preserve-logs`: Preserve log files
- `--backup-dir <PATH>`: Custom backup directory
- `--no-backup`: Do not create backup (PERMANENTLY DELETE - use with caution!)
- `--force`: Skip confirmation prompts

## Directory Structure

After installation, Flame uses the following directory structure:

```
${PREFIX}/
├── bin/                    # Flame binaries
│   ├── flame-session-manager
│   ├── flame-executor-manager
│   ├── flmctl
│   └── flmadm
├── sdk/python/             # Python SDK
├── examples/               # Optional example binaries, docs, and manifests from --with-examples
│   ├── agents/scripts/
│   ├── pi/rust/
│   ├── pi/python/
│   ├── vllm/offline/
│   ├── vllm/serving/
│   └── candle/based/
├── work/                   # Working directory
│   ├── sessions/
│   └── executors/
├── logs/                   # Log files
│   ├── fsm.log
│   └── fem.log
├── conf/                   # Configuration
│   ├── flame-cluster.yaml
│   └── applications/       # Applications registered when session manager starts
│       ├── flmbox.yaml
│       ├── flmexec.yaml
│       ├── flmping.yaml
│       └── flmrun.yaml
└── data/                   # Data directory (cache, database)
    ├── cache/
    └── sessions.db
```

## Service Management

After installation with systemd, manage services using standard `systemctl` commands:

```bash
# Start services
sudo systemctl start flame-session-manager
sudo systemctl start flame-executor-manager

# Stop services
sudo systemctl stop flame-executor-manager
sudo systemctl stop flame-session-manager

# Enable on boot
sudo systemctl enable flame-session-manager flame-executor-manager

# Check status
sudo systemctl status flame-session-manager
sudo systemctl status flame-executor-manager

# View logs
sudo journalctl -u flame-session-manager -f
tail -f /usr/local/flame/logs/fsm.log
```

## Prerequisites

- **Rust toolchain**: Required only by `flmadm package` to build from source (not by `install`)
  - Install: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **uv**: Required by `flmadm package`, which bundles it into the package; installing from a package needs none
- **Network access to PyPI**: The Python SDK's dependencies are installed from PyPI
- **systemd**: Required for service management (unless `--no-systemd`)
- **Root privileges**: Required for system-wide installation with systemd

## Examples

### Development Workflow

```bash
# Developer working on Flame source
cd ~/projects/flame
cargo build --release -p flmadm
./target/release/flmadm package --src-dir . --output /tmp/flame-package --force
/tmp/flame-package/package/sbin/flmadm install --all --prefix ~/flame --no-systemd

# Make changes, repackage (this rebuilds what changed) and reinstall
./target/release/flmadm package --src-dir . --output /tmp/flame-package --force
/tmp/flame-package/package/sbin/flmadm install --all --prefix ~/flame --no-systemd --clean
```

### Production Deployment

```bash
# Single-node deployment (all components)
sudo flmadm install --all --enable

# Multi-node deployment: Control plane node
sudo flmadm install --control-plane --enable

# Multi-node deployment: Worker nodes
sudo flmadm install --worker --enable

# Client machine (no services)
flmadm install --client --prefix ~/flame --no-systemd

# Verify installation
sudo systemctl status flame-session-manager flame-executor-manager
/usr/local/flame/bin/flmctl --version

# Upgrade to new version
sudo systemctl stop flame-executor-manager flame-session-manager
sudo flmadm install --all --clean
sudo systemctl start flame-session-manager flame-executor-manager
```

### Distributed Cluster Setup

```bash
# On control plane node (control01)
sudo flmadm install --control-plane --enable

# On worker nodes (worker01, worker02, ...)
sudo flmadm install --worker --enable

# On client/jump host (for users)
flmadm install --client --prefix ~/flame --no-systemd

# Update only worker nodes
sudo flmadm install --worker --force
```

### Testing and Cleanup

```bash
# Install for testing
sudo flmadm install --all --prefix /tmp/flame-test --no-systemd

# Test the installation
/tmp/flame-test/bin/flmctl --version

# Clean up
flmadm uninstall --prefix /tmp/flame-test --no-backup --force
```

## Troubleshooting

### Build Failures

`flmadm install` builds nothing. If `flmadm package` fails to build, use `--verbose` to see the full
cargo output:

```bash
flmadm package --src-dir . --output /tmp/flame-package --verbose
```

### Permission Errors

For system-wide installation, run with `sudo`:

```bash
sudo flmadm install --all
```

For user-local installation, use `--no-systemd`:

```bash
flmadm install --all --no-systemd --prefix ~/flame
```

### Service Start Failures

Check service status and logs:

```bash
sudo systemctl status flame-session-manager
sudo journalctl -u flame-session-manager -n 50
tail -50 /usr/local/flame/logs/fsm.log
```

### Uninstall Safety

`flmadm` includes safety checks to prevent accidental system damage:
- Cannot uninstall from system paths like `/`, `/usr`, `/etc`
- Installation path must contain "flame" in its name
- Requires confirmation unless `--force` is used
- Creates backup by default before removal

## Exit Codes

- `0`: Success
- `1`: Invalid arguments or configuration
- `2`: Build failure
- `3`: Installation failure (permissions, disk space, etc.)
- `4`: Systemd configuration failure
- `5`: Service start failure

## See Also

- `flmctl`: User CLI for job submission and session management
- [Installation Tutorial](../docs/tutorials/) (coming soon)
- [Flame Documentation](../docs/)
- [RFE333 Functional Specification](../docs/designs/RFE333-flmadm/FS.md)

## License

Same as Flame project license.
