# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

## [0.7.0](https://github.com/89jobrien/taskit/releases/tag/v0.7.0) - 2026-06-28

### Features

- **taskit-testing**: new crate with TempDirGuard, `in_temp_dir!`,
  `step_result!`, `single_step_outcome`, and TaskitResultExt (6d672c0)
- **taskit-macros**: new proc-macro crate with `#[taskit_test(tempdir,
  offline)]` and `#[derive(ConfigDefaults)]` (6d672c0)
- **taskit-output**: new crate with MessageSink trait, StderrSink,
  BufferSink, TeeSink, structured output macros, and moved formatters
  from taskit-engine (6d672c0)
- **init**: add mdBook scaffold generator (42ee04e)
- **init**: expand scaffolding with hooks, CI, deny.toml, .ctx/, and
  smart discovery (c6cd574)
- **flow**: add `taskit flow` subcommand for branching workflow (e007411)
- **taskit-types**: scaffold crate with TaskitError, ConfigError,
  PipelineError, StepError, ProtocolError, InitError (7be47f2..af0f262)
- **taskit**: add miette with fancy feature to binary (47ebfe8)

### Refactoring

- Migrate ~140 `eprintln!` calls to structured output macros (6d672c0)
- Migrate 18 verbose `.map_err` chains to `err_context()` (6d672c0)
- Remove duplicate `print_summary()` from step.rs (6d672c0)
- Make `quick::run()` format-aware via `write_output()` (6d672c0)
- Replace anyhow::Result with TaskitError at all public API
  boundaries (45654a0)
- Move config, step, and output_format types to taskit-types (c77d2ba,
  a87bcf1)
- taskit-core is now ports-only (PipelineRunner trait) (6c29918)
- Unify taskit-engine output to TaskitError (dcec4c4)

### Tests

- Add conformance, property, integration, and fuzz tests (751d8f6)

### Fixes

- **hooks**: use `--no-tests warn` for proc-macro crates in pre-push
  (8555d46)
- **init**: respect --dry-run flag in taskit init (dde1b81)
- Remove redundant closure in ci pipeline gate (2b6dcd2)

## [0.6.0](https://github.com/89jobrien/taskit/releases/tag/v0.6.0) - 2026-06-28

Release infrastructure: per-crate tag prefixes, cargo-rail config.

## [0.5.0](https://github.com/89jobrien/taskit/releases/tag/v0.5.0) - 2026-06-28

### Features

- Integrate output formatters and cargo alias on init (9dba361, a2c1d65)
- Add taskit publish subcommand with doc generation and ordered crate
  publishing (b8992a3)
- Add taskit inspect subcommand for threshold-based metrics
  pass/fail (c586e32)

### Fixes

- Use edition 2024 in templates, prune artifacts on clean (16444ff)

## [0.4.0] - 2026-06-28

### Features

- Auto-discovery from cargo metadata (v0.2.0) (94b4db6)
- Structured output with Human/Json/Github/Junit formatters and miette
  diagnostics (v0.3.0) (22d9d66)
- OutputFormat::Diagnostic with miette graphical/narrated
  rendering (8434518)
- PipelineRunner adapters + conformance tests (ba5fa16)
- Add taskit-init and taskit-crux crates, wire Init subcommand (d7d3703)
- Add `taskit health` subcommand (1f5f581)

### Refactoring

- Create multi-crate workspace: taskit-core, taskit-engine (8e93e20,
  9fcf6b4, b706e3e, 441f9d7, a9e15e6)

### Fixes

- Empty CiConfig.steps runs nothing (d21e831)
- Use pid in tmp_file to avoid nextest collision (a9fd392)
- Apply MOA review findings (ea64604)

## [0.1.1] - 2026-06-27

### Features

- Config-driven CI pipeline with taskit.toml discovery and cargo metadata
  fallback (b2496e9, 5a41f9e)
- Dispatch pipeline steps from CiConfig; fall back to hardcoded
  default (08e9179)

### Refactoring

- Remove hardcoded crate constants; accept &WorkspaceConfig (9109629)
- Remove hardcoded SURFACES; accept Option<&ProtocolConfig> (6858ed1)
- Remove maestro-specific modules (6165126)

### CI

- Add CI, release, nightly, and publish workflows (3b28215, 0c86c45,
  bc75e82)
- Add crux pipeline for taskit CI (27ee5b9, a0c30c5)
