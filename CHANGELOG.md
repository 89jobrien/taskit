# Changelog

All notable changes to this project will be documented in this file.

## [unreleased]

### Features

- _(taskit)_ Add auto-discovery from cargo metadata (v0.2.0)
- Structured output with Human/Json/Github/Junit formatters and miette diagnostics (v0.3.0)
- _(taskit)_ Add OutputFormat::Diagnostic with miette graphical/narrated rendering
- _(taskit-engine)_ PipelineRunner adapters + conformance tests
- _(taskit)_ Add taskit-init and taskit-crux crates, wire Init subcommand
- _(workspace)_ V0.4 multi-crate restructure with PipelineRunner port and taskit init
- _(taskit-engine)_ Add `taskit health` subcommand

### Bug Fixes

- _(workspace)_ Apply MOA review findings [moa-review]
- _(taskit-engine)_ Use pid in tmp_file to avoid nextest collision
- _(workspace)_ Empty CiConfig.steps runs nothing, add MOA review TODOs

### Refactor

- _(taskit)_ Create workspace + taskit-core crate skeleton
- _(taskit-core)_ Extract core types and PipelineRunner trait
- _(taskit-engine)_ Create engine crate with all pipeline modules
- _(taskit)_ Convert root package to thin bin crate
- _(taskit)_ Remove old src/ modules (now in taskit-engine)

### Documentation

- _(project)_ Add AGENTS.md agent operating guide

### Miscellaneous

- Add health baseline (341 tests, 0 clippy warnings, 8 TODOs)
- Add cargo-rail release commands for Claude Code
- Configure cargo-rail release for all workspace crates

## [0.1.1] - 2026-06-27

### Features

- _(config)_ Add config.rs with taskit.toml discovery and cargo metadata fallback
- _(main)_ Replace CARGO_MANIFEST_DIR root detection with config::load()
- _(ci)_ Dispatch pipeline steps from CiConfig; fall back to hardcoded default when unconfigured

### Bug Fixes

- Scope CrateEntry/PropagationEntry imports to cfg(test) in affected.rs
- Move make_executable before mod tests; generalize test runner flags; add offline_skip config field

### Refactor

- _(affected)_ Remove hardcoded crate constants, accept &WorkspaceConfig throughout
- _(drift)_ Remove hardcoded SURFACES, accept Option<&ProtocolConfig>; un-ignore calculate_lockfile test

### Testing

- Ignore calculate_lockfile_hashes_all_surfaces outside maestro workspace

### Miscellaneous

- Remove maestro-specific modules (schema, k8s, docker, smolvm, smoke, conformance)
- Add Cargo.toml publish metadata, LICENSE files, README, .gitignore
- Untrack target/ and .DS_Store (now in .gitignore)
- Add CI and release workflows; init cargo-rail config
- Split publish into its own workflow
- Add nightly workflow (audit, deny, geiger, coverage, mutants)
- Add crux pipeline for taskit CI
- Run pipeline via crux run ci.crux
- Flatten crux pipeline steps (remove wrapper pipe)
