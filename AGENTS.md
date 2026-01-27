# Flame – A Distributed System for Agentic AI

Repo: https://github.com/xflops-io/flame

## Project Structure & Module Organization

- **Core components:**
  - `session_manager/`: Session management and scheduling (Rust)
  - `executor_manager/`: Executor lifecycle and binding (Rust)
  - `object_cache/`: Distributed object caching (Rust)
  - `flmctl/`: CLI for managing Flame resources (Rust)
  - `flmexec/`: Executor service for running scripts (Rust)
  - `rpc/`: gRPC protocol definitions (protobuf)
  - `common/`: Shared Rust utilities and types
  - `cri/`: Container runtime interface
  - `stdng/`: Standard library extensions (collections, logs, rand)

- **SDK:**
  - `sdk/python/`: Python SDK for Flame applications
  - `sdk/rust/`: Rust SDK (if applicable)

- **Examples:**
  - `examples/agents/`: Agent examples and demos
  - `examples/pi/`: Pi calculation examples (Rust, Python)
  - `examples/crawler/`: Web crawler example
  - `examples/ps/`: Parameter Server example

- **Infrastructure:**
  - `installer/`: Kubernetes deployment manifests
  - `ci/`: CI configuration (kind, supervisord, YAML configs)
  - `docker/`: Dockerfiles for all services
  - `.github/workflows/`: CI/CD pipelines

- **Documentation:**
  - `docs/designs/`: Design documents for RFEs and bugs; all related documents (FS.md, IMPLEMENTATION.md, STATUS.md) should be in the same directory under `docs/designs/RFE<number>-<name>/` to keep design details aligned
  - `docs/tutorials/`: Tutorial documents for both user and admin
  - `docs/blogs/`: Blog posts about Flame

## Build, Test, and Development Commands

- **Build:** `docker compose build` (compiles all images of Flame); if Docker is unavailable, Podman is an alternative
- **Dev Environment:** 
  - Start: `docker compose up -d` (start Flame cluster after rebuilding)
  - Stop: `docker compose down` (stop the cluster)
  - Restart: `docker compose restart` (restart specific services)
- **Test:**
  - All tests: `make e2e` (runs all tests)
  - Python tests: `make e2e-py`
  - Rust tests: `make e2e-rs`
  - Individual Rust tests: `cargo test` (from component directory)
- **Lint/Format:** `make format` (auto-format all code using rustfmt and Python formatters)
- **Build specific component:** `cargo build --release` (from component directory)
- **Check without building:** `cargo check` (fast compilation check)
- **Clippy linting:** `cargo clippy` (Rust linter)

## Coding Style & Naming Conventions

- **Language:** Rust (for core services), Python (for SDK and examples)
- **Rust:**
  - Follow standard Rust conventions (see `rustfmt.toml` and `clippy.toml`)
  - Prefer strong typing; avoid excessive `unwrap()` in production code
  - Use `Result<T, E>` for error handling
  - Keep functions concise; aim for single responsibility
  - Use design patterns: factory, state machine (see `session_manager/src/controller/states/`, `executor_manager/src/states/`)
  
- **Python:**
  - Follow PEP 8 style guidelines
  - Use type hints for function signatures
  - Keep SDK code clean and well-documented

- **General principles:**
  - Avoid code duplication; extract helper functions, const values, shared modules
  - Avoid large/long functions (aim for <100 lines when reasonable)
  - Avoid deeply nested if/else blocks and for loops; refactor for clarity
  - Add brief code comments for tricky or non-obvious logic
  - Use meaningful variable and function names
  - Keep files focused; split large modules into sub-modules

- **Naming:**
  - Use **Flame** for product/documentation headings
  - Use `flame` for package names, paths, and config keys
  - Component names: `flame-session-manager`, `flame-executor-manager`, `flame-object-cache`, etc.

## Testing Guidelines

- **CI/CD:** The CI plan can be found in the `.github/workflows/` directory (code-verify, e2e-py, e2e-rust)
- **Pre-test setup:** Restart the Flame cluster before starting testing to ensure clean state
- **Test timing:** Most tests should complete within 30 seconds. If not, check the logs:
  - `docker logs flame-executor-manager`
  - `docker logs flame-object-cache`
  - `docker logs flame-session-manager`
  - Or use: `docker compose logs <service-name>`
  
- **Rebuild requirements:**
  - If any code in `sdk/python` is updated, rebuild both `flame-console` and `flame-executor-manager`
  - If RPC definitions in `rpc/protos/` change, rebuild all dependent services
  - If core component changes, rebuild the cluster: `docker compose build && docker compose up -d`

- **Test locations:**
  - E2E tests: `e2e/tests/` (Python)
  - Rust unit/integration tests: `<component>/tests/` or inline in source files
  - Test helpers: `e2e/src/e2e/helpers.py`

- **Running specific tests:**
  - Python: `pytest e2e/tests/test_<name>.py`
  - Rust: `cargo test <test_name>` (from component directory)

## Commit & Pull Request Guidelines

- **Commit messages:**
  - Follow concise, action-oriented format: `[component] brief description`
  - Examples:
    - `[session_manager] add fairshare scheduling plugin`
    - `[sdk/python] fix agent context serialization`
    - `[docs] update runner setup tutorial`
  - Group related changes; avoid bundling unrelated refactors
  - If addressing a specific RFE/bug, reference it: `[RFE318] implement cache eviction policy`

- **Before committing:**
  - Always run `make format` to auto-format all code
  - Run relevant tests: `make e2e-py` or `make e2e-rs`
  - Check for linter errors: `cargo clippy` for Rust
  - Verify Docker services still work: `docker compose up -d`

- **Pull Request format:**
  - Title: `[<project_name>] <Title>` (e.g., `[session_manager] Add application resource management`)
  - Include:
    - Summary of changes and motivation
    - Testing performed (which tests ran, any manual testing)
    - Any user-facing changes or breaking changes
    - References to related RFE/bug documents

- **PR review process:**
  - Ensure CI passes (all workflow checks green)
  - Review must check for code style adherence
  - Verify documentation updates if needed
  - Test locally if changes are significant

## Troubleshooting & Debugging

- **Container logs:**
  - View all logs: `docker compose logs -f`
  - Specific service: `docker compose logs -f flame-session-manager`
  - Recent errors: `docker compose logs --tail=100 flame-executor-manager`

- **Common issues:**
  - **Tests timeout:** Check if services are running (`docker compose ps`); restart cluster if needed
  - **Build failures:** Clear Docker cache: `docker compose build --no-cache`
  - **Port conflicts:** Ensure no other services using Flame ports (check `compose.yaml` for port mappings)
  - **Python SDK changes not reflected:** Rebuild console and executor-manager containers
  - **gRPC connection errors:** Verify network configuration in `compose.yaml`; check service health: `docker compose ps`

- **Database inspection (session_manager):**
  - SQLite migrations: `session_manager/migrations/sqlite/`
  - Access DB: `docker compose exec flame-session-manager sh -c 'sqlite3 /data/sessions.db'`

- **Testing with Kubernetes:**
  - Local cluster: `kind create cluster --config ci/kind.yaml`
  - Deploy: `kubectl apply -k installer/`
  - Check pods: `kubectl get pods -n flame-system`
  - Logs: `kubectl logs -n flame-system <pod-name>`

## Agent-Specific Notes

- **Always check file contents** before making changes; use Read tool first
- **When adding features**, review similar existing implementations in the codebase for patterns
- **State machines:** Both session_manager and executor_manager use state machine patterns; study existing states before adding new ones
- **RPC changes:** If modifying `.proto` files, remember to rebuild all services that depend on those definitions
- **Documentation:** When adding or changing features, update relevant docs in `docs/designs/`, `docs/tutorials/`, or `docs/blogs/`
- **Multi-language project:** Be mindful of both Rust and Python conventions; respect each language's idioms
- **Breaking changes:** If introducing breaking changes to SDK, document migration path
- **Performance:** Flame is designed for distributed systems; consider scalability and concurrency in designs
- **Error handling:** Prefer explicit error handling; avoid panics in production code paths
- **When answering questions**, respond with high-confidence answers only: verify in code; do not guess
- **Security:** Never commit secrets, credentials, or sensitive data to the repository
- **Container rebuilds:** Be aware that changing core components may require rebuilding multiple containers
- **Dependencies:** Check `Cargo.toml` (Rust) or `pyproject.toml` (Python) before adding new dependencies
- **Focus on task:** When working on a specific issue/PR, stay focused on that scope; avoid unrelated refactoring unless explicitly requested