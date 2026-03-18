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
- Keep separate `FlameError` definitions: `common/` for internal components, `sdk/rust/` for end-users (intentionally different APIs)
- API versioning via package name only (`flame.v1`), no new message fields

## Future Releases

### Phase 1 (Planned)
- [ ] P1.1: Eliminate production `unwrap()` panics
- [ ] P1.2: Fix CLI `todo!()` panic
- [ ] P1.3: Fix WasmShim task failure handling
- [ ] P1.4: Fix executor persistence gap

### Phase 2 (Planned)
- [ ] P2.1: Add API versioning to proto files
- [ ] P2.2: Split `apis.rs` into focused modules
- [ ] P2.3: Add code coverage to CI

### Phase 3 (Planned)
- [ ] P3.1: Add Python SDK unit tests
- [ ] P3.2: Add state machine tests
- [ ] P3.3: Add shell completion to CLIs
- [ ] P3.4: Generate API documentation

### Phase 4 (Ongoing)
- [ ] P4.1: CI/CD enhancements
- [ ] P4.2: Documentation improvements (including proto file comments aligned with design docs)
- [ ] P4.3: Code deduplication
