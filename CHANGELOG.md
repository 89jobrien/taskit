# Changelog

## [0.6.0](https://github.com/89jobrien/taskit/releases/tag/v0.6.0) - 2026-06-28

### ✨ Features

- integrate output formatters, self-updating xtask shim, cargo alias on init ([9dba361](https://github.com/89jobrien/taskit/commit/9dba361e02fbc50c15463ba76a372490a0e552dc))
- integrate inspect/publish with output formatters; generate xtask crate and cargo alias on init ([a2c1d65](https://github.com/89jobrien/taskit/commit/a2c1d65168d97dba06d9d9ef83dc299f8cf79885))
- add taskit publish subcommand with doc generation and ordered crate publishing ([b8992a3](https://github.com/89jobrien/taskit/commit/b8992a39e81a6aa7445cdb42593c1fef3eb964be))
- add taskit inspect subcommand for threshold-based metrics pass/fail ([c586e32](https://github.com/89jobrien/taskit/commit/c586e32f439b552539cec2a20cb64a8f8dbfd0e0))
- **taskit**: add OutputFormat::Diagnostic with miette graphical/narrated rendering ([8434518](https://github.com/89jobrien/taskit/commit/8434518b973e58ded8f868604a8f1ade43fcc87b))
- **ci**: dispatch pipeline steps from CiConfig; fall back to hardcoded default when unconfigured ([08e9179](https://github.com/89jobrien/taskit/commit/08e9179190bdcdcfb3c264307af02d526d2a65bc))
- **main**: replace CARGO_MANIFEST_DIR root detection with config::load() ([5a41f9e](https://github.com/89jobrien/taskit/commit/5a41f9ef257c3de2130f0047423410aa0a417a1a))
- **config**: add config.rs with taskit.toml discovery and cargo metadata fallback ([b2496e9](https://github.com/89jobrien/taskit/commit/b2496e951ccbe749935bf154d4df5256f72f122b))

### 🐛 Bug Fixes

- use edition 2024 in xtask template, cargo xtask alias, prune artifacts on clean ([16444ff](https://github.com/89jobrien/taskit/commit/16444ffd8a078c2ab65f576b26fee77e84ed6685))
- **workspace**: empty CiConfig.steps runs nothing, add MOA review TODOs ([d21e831](https://github.com/89jobrien/taskit/commit/d21e831c3c7e3a5b43414a06cb14d20726a89198))
- move make_executable before mod tests; generalize test runner flags; add offline_skip config field ([0598b0e](https://github.com/89jobrien/taskit/commit/0598b0e050c9735e2f521ede55d0d5a36533c62b))
- scope CrateEntry/PropagationEntry imports to cfg(test) in affected.rs ([df6a457](https://github.com/89jobrien/taskit/commit/df6a45768dbb26056d7edaa2f4d1f5b4b6657f13))

### 🔧 Chores

- disable github release and release notes requirement in rail.toml ([62a573e](https://github.com/89jobrien/taskit/commit/62a573e7d8c2c3aef9adfa0fa218c32186934267))
- add per-crate tag prefixes to rail.toml ([56e5c3a](https://github.com/89jobrien/taskit/commit/56e5c3aa74d6e5aae4299fdc18f8d7aac3e098d5))
- **release**: taskit v0.5.0 ([cc34a86](https://github.com/89jobrien/taskit/commit/cc34a867c1cd3c4432ce56c4a45b25b7b32738a3))
- restructure to workspace dependencies ([e453269](https://github.com/89jobrien/taskit/commit/e453269d54930fe92ead772a26044b1a089299e0))
- **release**: taskit v0.4.0 ([86b65ec](https://github.com/89jobrien/taskit/commit/86b65ecb0a17be982224bec2e99f3d68bbbf01d2))
- add GitHub templates, release notes, and command templates ([b260d2c](https://github.com/89jobrien/taskit/commit/b260d2c99c369d0ef2b797e9febf1cc3e1fb0db6))
- add cargo-rail release commands for Claude Code ([97b7754](https://github.com/89jobrien/taskit/commit/97b7754a45e7018143a59bc9c9a56102bf74bc1c))
- untrack target/ and .DS_Store (now in .gitignore) ([5dfb0f6](https://github.com/89jobrien/taskit/commit/5dfb0f67545f2bfa301336b3324bd58799c50eb7))
- add Cargo.toml publish metadata, LICENSE files, README, .gitignore ([05a31f1](https://github.com/89jobrien/taskit/commit/05a31f1de67e2c1ccfa21d4b83ef28d3d47a7114))
- remove maestro-specific modules (schema, k8s, docker, smolvm, smoke, conformance) ([6165126](https://github.com/89jobrien/taskit/commit/616512639787d61392d24c02f9c57fef31147355))

### 👷 CI

- flatten crux pipeline steps (remove wrapper pipe) ([0b577b6](https://github.com/89jobrien/taskit/commit/0b577b6b20bfd9669a5eeef86c3b6ae27b32dc0d))
- run pipeline via crux run ci.crux ([a0c30c5](https://github.com/89jobrien/taskit/commit/a0c30c5582054c9bf922a7f0cd90a51c1a29f0e2))
- add crux pipeline for taskit CI ([27ee5b9](https://github.com/89jobrien/taskit/commit/27ee5b922f0b89573eccbd7b6ac8e7473ae29d5d))
- add nightly workflow (audit, deny, geiger, coverage, mutants) ([bc75e82](https://github.com/89jobrien/taskit/commit/bc75e827e1970d2ab9e35f849f52af241d42a652))
- split publish into its own workflow ([0c86c45](https://github.com/89jobrien/taskit/commit/0c86c45df2b6f60fbff6cbc6d953a986d5b2367b))
- add CI and release workflows; init cargo-rail config ([3b28215](https://github.com/89jobrien/taskit/commit/3b2821575d5f0f3940e42a7ddb0beb0138784a2f))

### 📝 Documentation

- update CLAUDE.md with clean and init behavior ([345a3d1](https://github.com/89jobrien/taskit/commit/345a3d199389ab951f4adb02f1e2c9dd18ed687b))

### ♻️ Refactoring

- **drift**: remove hardcoded SURFACES, accept Option<&ProtocolConfig>; un-ignore calculate_lockfile test ([6858ed1](https://github.com/89jobrien/taskit/commit/6858ed1883e312da18be8c674572c9e9d7f9ee31))
- **affected**: remove hardcoded crate constants, accept &WorkspaceConfig throughout ([9109629](https://github.com/89jobrien/taskit/commit/91096293b74597b8b1803aa68a55667aac339a69))

### ✅ Testing

- ignore calculate_lockfile_hashes_all_surfaces outside maestro workspace ([a81d6b0](https://github.com/89jobrien/taskit/commit/a81d6b0f46b9d086aea215b6a22218220954413c))

## [0.5.0](https://github.com/89jobrien/taskit/releases/tag/v0.5.0) - 2026-06-28

### ✨ Features

- integrate output formatters, self-updating xtask shim, cargo alias on init ([9dba361](https://github.com/89jobrien/taskit/commit/9dba361e02fbc50c15463ba76a372490a0e552dc))
- integrate inspect/publish with output formatters; generate xtask crate and cargo alias on init ([a2c1d65](https://github.com/89jobrien/taskit/commit/a2c1d65168d97dba06d9d9ef83dc299f8cf79885))
- add taskit publish subcommand with doc generation and ordered crate publishing ([b8992a3](https://github.com/89jobrien/taskit/commit/b8992a39e81a6aa7445cdb42593c1fef3eb964be))
- add taskit inspect subcommand for threshold-based metrics pass/fail ([c586e32](https://github.com/89jobrien/taskit/commit/c586e32f439b552539cec2a20cb64a8f8dbfd0e0))
- **taskit**: add OutputFormat::Diagnostic with miette graphical/narrated rendering ([8434518](https://github.com/89jobrien/taskit/commit/8434518b973e58ded8f868604a8f1ade43fcc87b))
- **ci**: dispatch pipeline steps from CiConfig; fall back to hardcoded default when unconfigured ([08e9179](https://github.com/89jobrien/taskit/commit/08e9179190bdcdcfb3c264307af02d526d2a65bc))
- **main**: replace CARGO_MANIFEST_DIR root detection with config::load() ([5a41f9e](https://github.com/89jobrien/taskit/commit/5a41f9ef257c3de2130f0047423410aa0a417a1a))
- **config**: add config.rs with taskit.toml discovery and cargo metadata fallback ([b2496e9](https://github.com/89jobrien/taskit/commit/b2496e951ccbe749935bf154d4df5256f72f122b))

### 🐛 Bug Fixes

- use edition 2024 in xtask template, cargo xtask alias, prune artifacts on clean ([16444ff](https://github.com/89jobrien/taskit/commit/16444ffd8a078c2ab65f576b26fee77e84ed6685))
- **workspace**: empty CiConfig.steps runs nothing, add MOA review TODOs ([d21e831](https://github.com/89jobrien/taskit/commit/d21e831c3c7e3a5b43414a06cb14d20726a89198))
- move make_executable before mod tests; generalize test runner flags; add offline_skip config field ([0598b0e](https://github.com/89jobrien/taskit/commit/0598b0e050c9735e2f521ede55d0d5a36533c62b))
- scope CrateEntry/PropagationEntry imports to cfg(test) in affected.rs ([df6a457](https://github.com/89jobrien/taskit/commit/df6a45768dbb26056d7edaa2f4d1f5b4b6657f13))

### 🔧 Chores

- restructure to workspace dependencies ([e453269](https://github.com/89jobrien/taskit/commit/e453269d54930fe92ead772a26044b1a089299e0))
- **release**: taskit v0.4.0 ([86b65ec](https://github.com/89jobrien/taskit/commit/86b65ecb0a17be982224bec2e99f3d68bbbf01d2))
- add GitHub templates, release notes, and command templates ([b260d2c](https://github.com/89jobrien/taskit/commit/b260d2c99c369d0ef2b797e9febf1cc3e1fb0db6))
- add cargo-rail release commands for Claude Code ([97b7754](https://github.com/89jobrien/taskit/commit/97b7754a45e7018143a59bc9c9a56102bf74bc1c))
- untrack target/ and .DS_Store (now in .gitignore) ([5dfb0f6](https://github.com/89jobrien/taskit/commit/5dfb0f67545f2bfa301336b3324bd58799c50eb7))
- add Cargo.toml publish metadata, LICENSE files, README, .gitignore ([05a31f1](https://github.com/89jobrien/taskit/commit/05a31f1de67e2c1ccfa21d4b83ef28d3d47a7114))
- remove maestro-specific modules (schema, k8s, docker, smolvm, smoke, conformance) ([6165126](https://github.com/89jobrien/taskit/commit/616512639787d61392d24c02f9c57fef31147355))

### 👷 CI

- flatten crux pipeline steps (remove wrapper pipe) ([0b577b6](https://github.com/89jobrien/taskit/commit/0b577b6b20bfd9669a5eeef86c3b6ae27b32dc0d))
- run pipeline via crux run ci.crux ([a0c30c5](https://github.com/89jobrien/taskit/commit/a0c30c5582054c9bf922a7f0cd90a51c1a29f0e2))
- add crux pipeline for taskit CI ([27ee5b9](https://github.com/89jobrien/taskit/commit/27ee5b922f0b89573eccbd7b6ac8e7473ae29d5d))
- add nightly workflow (audit, deny, geiger, coverage, mutants) ([bc75e82](https://github.com/89jobrien/taskit/commit/bc75e827e1970d2ab9e35f849f52af241d42a652))
- split publish into its own workflow ([0c86c45](https://github.com/89jobrien/taskit/commit/0c86c45df2b6f60fbff6cbc6d953a986d5b2367b))
- add CI and release workflows; init cargo-rail config ([3b28215](https://github.com/89jobrien/taskit/commit/3b2821575d5f0f3940e42a7ddb0beb0138784a2f))

### 📝 Documentation

- update CLAUDE.md with clean and init behavior ([345a3d1](https://github.com/89jobrien/taskit/commit/345a3d199389ab951f4adb02f1e2c9dd18ed687b))

### ♻️ Refactoring

- **drift**: remove hardcoded SURFACES, accept Option<&ProtocolConfig>; un-ignore calculate_lockfile test ([6858ed1](https://github.com/89jobrien/taskit/commit/6858ed1883e312da18be8c674572c9e9d7f9ee31))
- **affected**: remove hardcoded crate constants, accept &WorkspaceConfig throughout ([9109629](https://github.com/89jobrien/taskit/commit/91096293b74597b8b1803aa68a55667aac339a69))

### ✅ Testing

- ignore calculate_lockfile_hashes_all_surfaces outside maestro workspace ([a81d6b0](https://github.com/89jobrien/taskit/commit/a81d6b0f46b9d086aea215b6a22218220954413c))

All notable changes to this project will be documented in this file.

## [0.4.0] - 2026-06-28

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
- Add git-cliff changelog config and /changelog command
- Add GitHub templates, release notes, and command templates

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
