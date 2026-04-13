# RFE375 Changelog

**Tracking Issue:** [#375](https://github.com/xflops/flame/issues/375)

All notable changes to the Codebase Enhancement initiative will be documented in this file.

## [Unreleased]

### Added
- Initial RFE375 created with comprehensive action plan
- Functional Specification (FS.md) documenting 14 enhancement tasks across 4 phases
- Status tracking document (STATUS.md) with progress metrics

### Analysis Completed
- Identified 167 `unwrap()` occurrences across 35 files (potential panic risks)
- Identified 5 TODO/FIXME comments indicating incomplete features
- Found code duplication in state machines between session_manager and executor_manager
- Identified `common/src/apis.rs` as 1284-line monolith needing split
- Assessed test coverage gaps: no Python SDK unit tests, missing state machine tests
- Identified CI/CD gaps: no security scanning, no code coverage reporting
- Documented 17 documentation gaps including missing API docs and troubleshooting guide

### Design Decisions
- `common/` and `sdk/rust/` cannot share any code: `common/` is for internal server components, `sdk/rust/` is for end-users with intentionally different APIs and dependencies
- API versioning via package name only (`flame.v1`), no new message fields

## Phase 2 Complete (2026-04-13)

### P2.1: API Versioning
- [x] Changed proto package from `flame` to `flame.v1` in all proto files
- [x] Updated go_package to `github.com/flame-sh/flame/sdk/go/rpc/v1`
- [x] Updated rpc/build.rs type attributes to use `flame.v1.*`
- [x] Updated sdk/rust/build.rs type attributes
- [x] Updated rpc/src/lib.rs to expose `flame::v1` module
- [x] Updated all Rust imports from `rpc::flame` to `rpc::flame::v1`
- [x] Copied updated protos to sdk/python/protos/ and sdk/rust/protos/

### P2.2: Split apis.rs
- [x] Created `common/src/apis/` module directory
- [x] Split 1350-line apis.rs into 5 files:
  - `mod.rs` (63 lines) - module re-exports and tests
  - `types.rs` (408 lines) - all domain type definitions
  - `session.rs` (156 lines) - Session impl blocks
  - `to_rpc.rs` (374 lines) - domain → rpc conversions
  - `from_rpc.rs` (400 lines) - rpc → domain conversions
- [x] Maintained public API compatibility via `pub use types::*`

### P2.3: Code Coverage CI
- [x] Added cargo-tarpaulin installation step to code-verify.yaml
- [x] Added coverage report generation with xml output
- [x] Added Codecov upload action with token support

## Phase 1 Complete (Previous)

### P1.1-P1.4: Critical Safety Fixes
- [x] P1.1: Eliminated production `unwrap()` panics (~26 occurrences fixed)
- [x] P1.2: Fixed CLI `todo!()` panic
- [x] P1.3: Fixed WasmShim task failure handling
- [x] P1.4: Executor persistence (already implemented)

## Future Releases

### Phase 3 (Planned)
- [ ] P3.1: Add Python SDK unit tests
- [ ] P3.2: Add state machine tests
- [ ] P3.3: Add shell completion to CLIs
- [ ] P3.4: Generate API documentation

### Phase 4 (Ongoing)
- [ ] P4.1: CI/CD enhancements
- [ ] P4.2: Documentation improvements (including proto file comments aligned with design docs)
- [ ] P4.3: Code deduplication
