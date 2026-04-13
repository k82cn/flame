# RFE375 Status

**Current Status:** Phase 2 Complete  
**Last Updated:** 2026-04-13  
**Tracking Issue:** [#375](https://github.com/xflops/flame/issues/375)  

## Progress Tracker

### Phase 1: Critical Safety Fixes ✓

| Task                              | Status | Assignee | PR  | Notes                                              |
| --------------------------------- | ------ | -------- | --- | -------------------------------------------------- |
| P1.1: Eliminate `unwrap()` panics | Done   | -        | -   | ~26 production occurrences fixed with TryFrom/expect |
| P1.2: Fix CLI `todo!()` panic     | Done   | -        | -   | Shows help instead of panicking                    |
| P1.3: Fix WasmShim task failure   | Done   | -        | -   | Proper TaskState::Failed handling                  |
| P1.4: Fix executor persistence    | N/A    | -        | -   | Already implemented in sqlite/filesystem           |

### Phase 2: Foundation Improvements ✓

| Task                          | Status | Assignee | PR  | Notes                                              |
| ----------------------------- | ------ | -------- | --- | -------------------------------------------------- |
| P2.1: Add API versioning      | Done   | -        | -   | All protos now use `flame.v1` package              |
| P2.2: Split `apis.rs`         | Done   | -        | -   | 1350 lines → 5 modules (types, session, from_rpc, to_rpc, mod) |
| P2.3: Add code coverage to CI | Done   | -        | -   | cargo-tarpaulin + Codecov integration added        |

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
| `unwrap()` in production code | 167      | 0      | ~26     |
| `TODO/FIXME` comments         | 5        | 0      | 5       |
| Code coverage (Rust)          | Unknown  | 70%    | Pending (CI added) |
| Code coverage (Python SDK)    | 0%       | 80%    | 0%      |
| CI pipeline duration          | ~5 min   | ~3 min | ~5 min  |
| Documentation completeness    | 60%      | 90%    | 60%     |

## Decision Log

| Date       | Decision                    | Rationale                                                             |
| ---------- | --------------------------- | --------------------------------------------------------------------- |
| 2026-03-17 | Created RFE372              | Comprehensive codebase review identified 89 enhancement opportunities |
| 2026-04-13 | API versioning: flame.v1    | Proto package changed from `flame` to `flame.v1` for future compatibility |
| 2026-04-13 | Split apis.rs into modules  | Original 1350-line file split into 5 files, each under 410 lines      |

## Blockers

None currently identified.
