# Changelog

All notable changes to taskit-engine will be documented in this file.

## Unreleased

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
