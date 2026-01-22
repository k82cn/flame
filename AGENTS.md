# Flame – A Distributed System for Agentic AI

## Tools
- **Build:** `docker compose build` (compiles all images of Flame); if Docker is unavailable, Podman is an alternative.
- **DevEnv:** `docker compose up -d` (start Flame cluster after rebuilding); `docker compose down` (stop the cluster)
- **Test:** `make e2e` (runs all test); `make e2e-py` for python and `make e2e-rs` for rust
- **Lint:** `make format` (auto-format all codes)

## Document Struct

- `docs/designs`: the designs doc of all RFEs and bugs; all related documents (FS.md, IMPLEMENTATION.md, STATUS.md) should be in the same directory under `docs/designs/RFE<number>-<name>/` to keep design details aligned.
- `docs/tutorials`: the tutorial documents for both user and admin.
- `docs/blogs`: the blogs of Flame.

## Code style

- Try to avoid duplicated codes by helper functions, const values and so on
- Try to use design pattern, e.g. factory, state machine
- Try to avoid large/long function
- Try to avoid deep nested if/else, and for loop

## Testing Instructions

- The CI plan can be found in the `.github/workflows` directory.
- Restart the Flame cluster before start the testing.
- Most tests should complete within 30 seconds. If not, check the logs for the `flame-executor-manager`, `flame-object-cache` and `flame-session-manager` containers.
- If any code in `sdk/python` is updated, be sure to rebuild both `flame-console` and `flame-executor-manager`.

## PR Instructions

- Title format: `[<project_name>] <Title>`
- Always run `make format` before committing changes.