# Changelog

All notable changes to taskit-types will be documented in this file.

## Unreleased

### Features

- `ConflictFile` and `ResolvedFile` types moved here from taskit-engine (a5381d7)
- `FlowError` variants: `ConflictUnresolved`, `NeedsHuman`, `CiFailed` (10ee91a)

### Refactoring

- Derive `Default` for `PipelineOutcome` (24bc3c6)
- Add `#[non_exhaustive]` to `FlowError` and `FlowAction` (a635817)
- Remove dead `ConflictUnresolved` variant from `FlowError` (9d91275)
