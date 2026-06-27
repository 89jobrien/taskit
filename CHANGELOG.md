# Changelog

## [0.1.1](https://github.com/89jobrien/taskit/releases/tag/v0.1.1) - 2026-06-26

### ✨ Features

- **ci**: dispatch pipeline steps from CiConfig; fall back to hardcoded default when unconfigured ([08e9179](https://github.com/89jobrien/taskit/commit/08e9179190bdcdcfb3c264307af02d526d2a65bc))
- **main**: replace CARGO_MANIFEST_DIR root detection with config::load() ([5a41f9e](https://github.com/89jobrien/taskit/commit/5a41f9ef257c3de2130f0047423410aa0a417a1a))
- **config**: add config.rs with taskit.toml discovery and cargo metadata fallback ([b2496e9](https://github.com/89jobrien/taskit/commit/b2496e951ccbe749935bf154d4df5256f72f122b))

### 🐛 Bug Fixes

- move make_executable before mod tests; generalize test runner flags; add offline_skip config field ([0598b0e](https://github.com/89jobrien/taskit/commit/0598b0e050c9735e2f521ede55d0d5a36533c62b))
- scope CrateEntry/PropagationEntry imports to cfg(test) in affected.rs ([df6a457](https://github.com/89jobrien/taskit/commit/df6a45768dbb26056d7edaa2f4d1f5b4b6657f13))

### 🔧 Chores

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

### ♻️ Refactoring

- **drift**: remove hardcoded SURFACES, accept Option<&ProtocolConfig>; un-ignore calculate_lockfile test ([6858ed1](https://github.com/89jobrien/taskit/commit/6858ed1883e312da18be8c674572c9e9d7f9ee31))
- **affected**: remove hardcoded crate constants, accept &WorkspaceConfig throughout ([9109629](https://github.com/89jobrien/taskit/commit/91096293b74597b8b1803aa68a55667aac339a69))

### ✅ Testing

- ignore calculate_lockfile_hashes_all_surfaces outside maestro workspace ([a81d6b0](https://github.com/89jobrien/taskit/commit/a81d6b0f46b9d086aea215b6a22218220954413c))

All notable changes to taskit will be documented in this file.
