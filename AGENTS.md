# Flame – A Distributed System for Agentic AI

## Development Environment Tips

- The development environment is based on Docker Compose. If Docker is unavailable, Podman is an alternative.
- Rebuild images using `docker compose build`. Start the Flame cluster with `docker compose up -d`.
- Always stop the Flame cluster with `docker compose down` before rebuilding any images.
- The designs and implementation details documents should be in `docs/designs` directory accordingly.
- All tutorial documents should be in `docs/tutorials`

## Testing Instructions

- The CI plan can be found in the `.github/workflows` directory.
- For Python, run end-to-end tests with `make e2e-ci`.
- For Rust, run E2E tests using `cargo test --workspace --exclude cri-rs -- --nocapture`.
- Most tests should complete within 30 seconds. If not, check the logs for the `flame-executor-manager` and `flame-session-manager` containers.
- If any code in `sdk/python` is updated, be sure to rebuild both `flame-console` and `flame-executor-manager`.

## PR Instructions

- Title format: `[<project_name>] <Title>`
- Always run `cargo clippy` and `cargo fmt` before committing changes.