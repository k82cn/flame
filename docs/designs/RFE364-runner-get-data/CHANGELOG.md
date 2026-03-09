# Changelog

All notable changes to the "Retrieve Input/Output of Runner Task" feature will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `get_data` helper function in `flamepy.runner` to retrieve task input/output data.
- `RunnerError` and `ErrorType` for structured error handling.
- Recursive resolution of `ObjectRef` instances in nested data structures (lists, dicts, tuples).
- Support for both `ObjectRef` encoded data and direct pickled data.
- Comprehensive unit and E2E tests.
