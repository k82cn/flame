# RFE375 Status

**Current Status:** Draft  
**Last Updated:** 2026-03-17  
**Tracking Issue:** [#375](https://github.com/xflops/flame/issues/375)  

## Progress Tracker

### Phase 1: Critical Safety Fixes

| Task                              | Status      | Assignee | PR  | Notes                                         |
| --------------------------------- | ----------- | -------- | --- | --------------------------------------------- |
| P1.1: Eliminate `unwrap()` panics | Not Started | -        | -   | 167 occurrences in 35 files                   |
| P1.2: Fix CLI `todo!()` panic     | Not Started | -        | -   | `flmctl/src/helper.rs:17`                     |
| P1.3: Fix WasmShim task failure   | Not Started | -        | -   | `executor_manager/src/shims/wasm_shim.rs:131` |
| P1.4: Fix executor persistence    | Not Started | -        | -   | `session_manager/src/storage/mod.rs:579`      |

### Phase 2: Foundation Improvements

| Task                          | Status      | Assignee | PR  | Notes                  |
| ----------------------------- | ----------- | -------- | --- | ---------------------- |
| P2.1: Add API versioning      | Not Started | -        | -   | All proto files        |
| P2.2: Split `apis.rs`         | Not Started | -        | -   | 1284 lines → 3 modules |
| P2.3: Add code coverage to CI | Not Started | -        | -   | Codecov integration    |

### Phase 3: Quality Improvements

| Task                        | Status      | Assignee | PR  | Notes                              |
| --------------------------- | ----------- | -------- | --- | ---------------------------------- |
| P3.1: Python SDK unit tests | Not Started | -        | -   | Target 80% coverage                |
| P3.2: State machine tests   | Not Started | -        | -   | session_manager + executor_manager |
| P3.3: Shell completion      | Not Started | -        | -   | bash, zsh, fish                    |
| P3.4: API documentation     | Not Started | -        | -   | Generate from protos               |

### Phase 4: Polish (Ongoing)

| Task                             | Status      | Assignee | PR  | Notes                       |
| -------------------------------- | ----------- | -------- | --- | --------------------------- |
| P4.1: CI/CD enhancements         | Not Started | -        | -   | Security, caching, releases |
| P4.2: Documentation improvements | Not Started | -        | -   | Troubleshooting, config ref |
| P4.3: Code deduplication         | Not Started | -        | -   | State machines, validators  |

## Metrics

| Metric                        | Baseline | Target | Current |
| ----------------------------- | -------- | ------ | ------- |
| `unwrap()` in production code | 167      | 0      | 167     |
| `TODO/FIXME` comments         | 5        | 0      | 5       |
| Code coverage (Rust)          | Unknown  | 70%    | Unknown |
| Code coverage (Python SDK)    | 0%       | 80%    | 0%      |
| CI pipeline duration          | ~5 min   | ~3 min | ~5 min  |
| Documentation completeness    | 60%      | 90%    | 60%     |

## Decision Log

| Date       | Decision       | Rationale                                                             |
| ---------- | -------------- | --------------------------------------------------------------------- |
| 2026-03-17 | Created RFE372 | Comprehensive codebase review identified 89 enhancement opportunities |

## Blockers

None currently identified.
