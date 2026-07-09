# Changelog

All notable changes to taskit-engine will be documented in this file.

## Unreleased

### Features

- `flow::auto` — agentic promote → CI gate → finish pipeline with conflict resolution
  hook (7b082a2)
- `merge_with_resolution` — git merge helper with conflict detection, resolver
  dispatch, and patch staging (cc9c740)
- `ConflictResolver` trait (now in taskit-core), `ConflictFile`, `ResolvedFile` types
  initially introduced here (a24ed07, moved in a5381d7)

### Refactoring

- Derive `Default` for `PipelineOutcome`; simplify `auto_with_ci` seam (24bc3c6)
- Add `#[non_exhaustive]` to `FlowError` and `FlowAction` (a635817)
- Fix `promote`/`finish` branch-switching behaviour (4b58717)
- Move `ResolvedFile` import into test module (38144da)

### Tests

- Integration tests for `flow::auto`: wrong-branch, dirty-worktree, dry-run (6fdee10)
- Integration tests for `merge_with_resolution`: all 4 branches (e541232)
- CI failure and pass paths via `auto_with_ci` seam (8aaa78a)

### Fixes

- AU/UA tests, doc fixes, step numbering (17eca40)
- Remove dead `ConflictUnresolved` variant from `FlowError` (9d91275)
- Auto-heal protocol-drift in pre-commit hook (86db27f)

## [0.7.0] - 2026-06-28

### Refactoring

- Move output formatters to taskit-output crate (re-exported)
- Migrate eprintln! to structured output macros
- Migrate map_err chains to err_context()
- Remove duplicate print_summary()
- Replace anyhow::Result with TaskitError at public boundaries
- Rename .taskit-cache/ (was .xtask-cache/)

## [0.6.0] - 2026-06-28

### Features

- Integrate inspect/publish with output formatters (a2c1d65)
- Add taskit publish subcommand with doc generation (b8992a3)
- Add taskit inspect subcommand for threshold metrics (c586e32)
- Add `taskit health` subcommand (1f5f581)

### Fixes

- Use edition 2024 in templates, prune artifacts on clean (16444ff)

## [0.5.0] - 2026-06-28

- Restructure to workspace dependencies (e453269)
