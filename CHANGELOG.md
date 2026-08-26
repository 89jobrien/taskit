# Changelog

All notable changes to this project will be documented in this file.
## [unreleased]

### Features

- *(core)* Add StepBuilder, conformance module, and property tests
- V0.7.0 accumulated changes
- *(fuzz)* Add fuzz_render_toml target with round-trip invariant
- *(engine)* Add pipeline diagnostics — command log, step context, run context
- Merge test/address-todo-cleanup — todo cleanup, CI fixes, pipeline diagnostics
- *(engine)* Wire ci.rs for pipeline diagnostics
- *(output)* Render pipeline diagnostic context in all formatters
- *(types)* Add ConflictUnresolved, NeedsHuman, CiFailed to FlowError
- *(engine)* Add ConflictResolver trait, ConflictFile, ResolvedFile types
- *(engine)* Add merge_with_resolution helper with conflict detection and resolver hook
- *(engine)* Implement flow::auto — promote, CI gate, finish with conflict resolution
- *(taskit)* Add FlowCmd::Auto, BamlConflictResolver stub, wire FlowAction::Auto
- *(taskit)* Implement BamlConflictResolver via BAML ResolveConflict function (fa7)
- *(init,types)* Xtask scaffold, config sections for inspect/clean/ci/release
- *(flow)* Add develop branch and sync/stage subcommands
- *(flow,output,config)* Resumable flow auto, CompactFormatter, config validation
- *(flow)* Collapse stage/finish/auto into flow promote
- *(flow)* Restore auto as alias for promote
- *(flow)* Promote advances one stage based on current branch
- *(flow)* Conflict_resolver is now config-driven
- *(engine,tui)* Add CI telemetry, drift detection, and a live TUI dashboard
- Add Nushell completion generation
- *(tui)* Add tabs, scrolling, and per-crate/history views to dashboard
- *(taskit-engine)* Add read-only protocol::drift::check
- *(taskit-engine)* Merge_with_resolution returns resolved conflict count
- *(taskit-engine)* Emit flow_auto_* telemetry from auto_with_ci
- *(taskit-tui)* Add flow and protocol-drift fields to Snapshot
- *(taskit-tui)* Add Tab::Flow and render the Flow tab
- *(taskit-tui)* Add protocol-drift line to Overview health panel
- *(taskit)* Add todo-sync, coverage --workspace, drift --watch, lint --fix
- *(engine)* Add health --gate for CI-friendly safety regression checks
- *(config)* Add exclude_from_version_check to CrateEntry
- *(flow)* Opt-in push of main and flow branches after successful flow auto
- *(taskit)* Add dev build/install cargo workflows, rename install to bootstrap
- Add changelog command and release automation

### Bug Fixes

- *(engine)* Test-report always generates report even when tests fail
- *(engine)* Make protocol-drift blocking in pre-push with self-healing
- *(engine)* Proptest command uses filter instead of feature flag
- *(engine)* Ignore excluded crates in compile-tests
- *(health)* Skip string/char literals when detecting TODO/FIXME comment markers
- *(engine)* Allow dead_code on step_with_diagnostics_and_context_sink (unused API surface)
- *(examples)* Silence progress spinner in pipeline_builder example
- *(flow)* Promote leaves user on release; finish auto-switches branch
- *(types)* Remove dead ConflictUnresolved variant from FlowError [moa-review]
- *(engine,taskit)* AU/UA tests, #[non_exhaustive] on structs, doc fixes, step numbering [moa-review]
- *(types,engine)* Add #[non_exhaustive] to FlowError and FlowAction [moa-review]
- *(hooks)* Auto-heal protocol-drift in pre-commit, not just pre-push
- *(engine)* Move ResolvedFile import into test module to fix unused-import lint
- *(ci)* Install protobuf-compiler for baml build dependency
- *(engine)* Inspect max_warnings Option<usize>, ctx accessors, clean config tests [moa-review]
- *(init,types)* Address MED review findings
- *(init,types,docs)* Address LOW review findings
- *(engine)* Use helper fns in flag-resolution tests to satisfy clippy
- *(patch)* Isolate version-bump tests in temp workspaces
- *(config)* Collapse nested if blocks to satisfy clippy
- Wire config.output.default_format as fallback for --output flag
- *(ci)* Commit generated baml_client so cargo fmt succeeds without baml CLI in CI
- *(ci)* Install protobuf-compiler for baml build dependency
- *(compile-tests)* Skip fuzz/ directory when scanning crate roots
- *(compile-tests)* Filter ignored dirs from cargo metadata workspace members
- *(ci)* Always install taskit from local path, not crates.io
- *(test)* Use 'true' instead of 'sh --version' in tool_exists_cmd test
- *(engine,output,docs)* Remove dead crux feature flag and unused re-export
- Don't force --lib on test-only crates in pre-push nextest run
- *(types,engine)* Add #[non_exhaustive] to FlowError and FlowAction [moa-review]
- *(types,engine)* Merge #[non_exhaustive] for FlowError and FlowAction [moa-review]
- *(taskit-init)* Disable xtask generation by default
- *(taskit-init)* Generate formatted xtask source
- *(hooks)* Clear git-local environment before pre-push

### Refactor

- *(engine)* Simplify release to GitHub-only
- *(engine)* Drop indicatif spinner, emit step results via MessageSink
- *(types)* Derive Default for PipelineOutcome; simplify auto_with_ci test construction
- *(types,core)* Move ConflictFile/ResolvedFile to taskit-types, ConflictResolver to taskit-core (mr5)
- *(engine)* Move ConflictResolver from FlowAction::Auto onto Flow struct (mr6)
- *(engine)* Extract CiRunner closure onto Flow struct, decouple flow::auto from ci (mr7)
- Remove dead Ctx::run_ok and unused NoOpResolver
- Tighten public surface across cache, discovery, testing, output, and core
- *(engine,output)* Consolidate ephemeral artifacts under target/taskit/
- *(taskit-engine)* Extract flow::status_report from status
- *(taskit)* Restructure CLI into categorized subcommands
- *(taskit)* Move Install/SelfCheck/SelfTest under new `self` subcommand

### Documentation

- *(examples)* Add runnable examples for all five library crates
- Update README, CLAUDE.md, DESIGN.md for flow auto + architecture refactor (rx6)
- *(changelog)* Regenerate changelogs for flow auto + type refactor work (rx7)
- *(changelog)* Update unreleased section for flow promote refactor
- *(claude)* Fix flow auto description and add flow config section
- Update handoff and fix .ctx gitignore to track HANDOFF.yaml
- Expand project docs and analysis config
- Bump workspace to 0.8.0, sync docs with current code, fix stale version pin
- *(taskit)* Add design and plan docs for TUI Flow tab
- *(taskit)* Add grounded feature-idea docs from ideation passes
- *(taskit)* Backfill changelog and add unreleased release notes
- *(taskit)* Document taskit-tui crate in CLAUDE.md
- Update handoff
- Update handoff

### Testing

- *(types)* Add proptest property tests for StepStatus, StepResult, PipelineOutcome
- *(init)* Add integration tests for plan_from_discovery -> render pipeline
- *(init)* Add proptest property tests for topo_sort_members
- *(output)* Add conformance tests for all 6 OutputFormatter impls
- *(engine)* Add dry-run unit tests for untested wrapper modules
- *(workspace)* Address remaining todo markers
- *(engine)* Integration tests for flow::auto — wrong branch, dirty worktree, dry-run happy path
- *(engine)* Integration tests for merge_with_resolution — all 4 branches; fix ignore_status and abs path [moa-review]
- *(engine)* Add CI failure and pass paths for flow::auto via auto_with_ci seam [moa-review]
- *(engine)* Ci effective_fail_fast config fallback coverage [moa-review]
- *(engine)* Add AU/UA coverage for parse_conflict_paths [moa-review]
- *(engine)* Merge AU/UA coverage for parse_conflict_paths [moa-review]
- *(engine)* Integration tests for merge_with_resolution — all 4 branches [moa-review]
- *(engine)* Merge integration tests for merge_with_resolution — all 4 branches [moa-review]
- *(taskit-init)* Require rustfmt-clean xtask scaffolds

### Miscellaneous

- *(ctx)* Add rustqual baseline
- Install taskit required tools
- Install taskit from checkout
- Update protocol-drift lockfile after mr5 type moves
- Update taskit cache hashes
- Untrack .taskit-cache (gitignored)
- *(deny)* Add CDLA-Permissive-2.0, remove unused license allowlist entries
- Install cargo-llvm-cov, cargo-deny, cargo-machete in CI
- *(quality)* Tune rustqual workspace signal
- Set output style to GmCtx in local settings
- Update handoff state and lockfile
- *(types)* Remove duplicate non_exhaustive attribute on FlowError
- Update handoff state
- Update handoff state and health baseline
- *(xtask)* Add depgraph-report.py for architecture-health audits
- Gitignore generated session-report/graph artifacts
- *(rust-audit)* Fix C-METADATA and C-LINK findings across workspace
- *(handoff)* Session-end update
- Add run-taskit skill for building/driving CLI + TUI dashboard
- Integrate daily orchestration remediation

## Unreleased

### Features

- **todo-sync**: new subcommand scanning TODO/FIXME source markers and syncing them to
  GitHub issues via `gh`; `--update`/`--warn-only` mirror `check-protocol-drift`'s shape
  (0583e14)
- **lint --fix**: auto-apply clippy's suggested fixes (`--fix --allow-dirty --allow-staged`);
  wired into pre-commit's protocol-drift-style self-healing (0583e14)
- **coverage --workspace**: measure line coverage across the whole workspace instead of a
  single crate (0583e14)
- **check-protocol-drift --watch**: continuously poll and auto-remediate lockfile drift
  instead of failing (0583e14)
- **check-freshness --warn-only**: report outdated workspace dependencies via
  `cargo-outdated` without failing the command (0583e14)
- **health --with-coverage**: opt into a workspace-wide coverage measurement as part of the
  health baseline; also adds `SafetyCounts` (unwrap/expect and `warn!()` site counts) and
  CI-duration telemetry to the baseline (0583e14)
- **TUI Flow tab**: new `Tab::Flow` rendering git-flow pipeline position, resumable
  `flow auto` state, configured conflict resolver, and `flow auto` run telemetry; adds a
  protocol-drift status line to the Overview tab's health panel (e6f4580, 6bbb510, 3914e56)
- **flow_auto_\* telemetry**: `flow::auto_with_ci` now emits duration/outcome telemetry
  readings (caf9d80)
- **merge_with_resolution**: now returns the count of conflicts it resolved (d0c25d0)
- **protocol::drift::check**: new read-only variant that recomputes and compares surface
  hashes without writing the lockfile, for callers (like the TUI) that poll on a timer
  (c27256e)
- **taskit-tui dashboard**: initial tabs, scrolling, and per-crate/history views
  (d8fb41c, 4a01447)
- **CI telemetry + drift detection**: new telemetry recording and drift-comparison
  machinery backing the live TUI dashboard (4a01447)
- Nushell completion generation (70699b2)

### Refactoring

- Extract `flow::status_report` from `flow::status` (fb7252d)
- Consolidate ephemeral taskit artifacts (compile cache, telemetry, TUI snapshots) under
  `target/taskit/` (d5ebf09)
- Tighten public surface across `cache`, `discovery`, `testing`, `output`, and `core`
  modules (ea3c5fe)
- Remove dead `Ctx::run_ok` and unused `NoOpResolver` (0348baa)

### Fixes

- Don't force `--lib` on test-only crates in pre-push nextest run (bcfad63)
- Remove dead crux feature flag and unused re-export (75b5dd1)
- `compile-tests`: filter ignored dirs from `cargo metadata` workspace members; skip
  `fuzz/` directory when scanning crate roots (af1fa56, bb15aec)
- Wire `config.output.default_format` as fallback for `--output` flag (58c00ba)
- CI: always install taskit from local path, not crates.io; install protobuf-compiler for
  baml build dependency; commit generated `baml_client` so `cargo fmt` succeeds without the
  baml CLI in CI; install cargo-llvm-cov/cargo-deny/cargo-machete
  (e922905, 9ce3b10, 3a9e087, 45836ef)
- Use `'true'` instead of `'sh --version'` in `tool_exists_cmd` test (1a7ce77)

### Docs

- Add design and plan docs for the TUI Flow tab (b9dd927)
- Bump workspace to 0.8.0, sync docs with current code, fix stale version pin (73170cc)
- Expand project docs and analysis config; update handoff, fix `.ctx` gitignore to track
  `HANDOFF.yaml` (c866eec, 73170bc)
- Fix flow auto description, add flow config section to CLAUDE.md (9a1ccb2)

### Chores

- Add taskit-tui to publish order, ignore paste advisory (f160dac)
- Tune rustqual workspace signal (32fe988)
- Add CDLA-Permissive-2.0, remove unused license allowlist entries (d6832f8)
- Untrack `.taskit-cache` (gitignored) (5ab72e0)

### Features

- **flow promote**: position-aware one-stage advance — develop→staging, staging→release,
  release→main+sync; removes the need to know which branch you're on
- **flow auto**: now runs the full pipeline (all stages) end-to-end; `promote` handles
  single-stage advance; `auto` is restored as the full-pipeline alias
- **ConflictResolverKind**: config-driven resolver selection (`baml` | `none`) read from
  `[flow] conflict_resolver` in `taskit.toml`; `flow auto` uses this setting; defaults to
  `baml` when unset
- **FlowError::NotAFlowBranch**: new variant signalled when `flow promote` is run from a
  branch not in the configured flow sequence

### Removed

- **FlowCmd::Stage** and **FlowCmd::Finish**: subcommands collapsed into `flow promote`;
  position-aware promote covers both cases

### Features

- **flow auto**: agentic pipeline (`taskit flow auto`) that promotes staging to release,
  runs the full CI gate, and finishes the merge — with configurable conflict resolution
  (7b082a2, 7898349, 0b8d0a6)
- **BamlConflictResolver**: BAML-backed LLM conflict resolver implementing the
  `ConflictResolver` port; calls `ResolveConflict` function and maps structured output
  to `ResolvedFile` (0b8d0a6)
- **FlowCmd::Auto**: wire `taskit flow auto` CLI subcommand with resolver dispatch
  (7898349)
- **merge_with_resolution**: helper on `FlowEngine` that runs `git merge`, detects
  conflicts, calls the resolver, stages patches, and completes the merge (cc9c740)
- **ConflictResolver trait**: pluggable port accepting `Vec<ConflictFile>` and returning
  `Vec<ResolvedFile>` (a24ed07)
- **FlowError extensions**: `ConflictUnresolved`, `NeedsHuman`, and `CiFailed` variants
  to signal unrecoverable flow states (10ee91a)

### Refactoring

- Move `ConflictFile` and `ResolvedFile` types to `taskit-types`; move
  `ConflictResolver` trait to `taskit-core` (a5381d7)
- Derive `Default` for `PipelineOutcome`; simplify `auto_with_ci` test seam
  construction (24bc3c6)
- Remove dead `ConflictUnresolved` variant from `FlowError` (9d91275)
- Add `#[non_exhaustive]` to `FlowError` and `FlowAction` for forward
  compatibility (a635817)
- Fix `promote` branch-switch: user stays on release after promote; `finish`
  auto-switches branch (4b58717)

### Tests

- Integration tests for `flow::auto` — wrong-branch guard, dirty-worktree guard,
  dry-run happy path (6fdee10)
- Integration tests for `merge_with_resolution` covering all 4 branches:
  clean merge, conflict + resolver, conflict + no-resolver, dry-run (e541232)
- CI failure and pass paths for `flow::auto` via `auto_with_ci` seam (8aaa78a)

### Fixes

- Move `ResolvedFile` import into test module to fix unused-import lint (38144da)
- Auto-heal protocol-drift lockfile in pre-commit hook (previously only pre-push)
  (86db27f)
- AU/UA tests, `#[non_exhaustive]` on structs, doc fixes, step numbering (17eca40)

### Docs

- Update README, CLAUDE.md, DESIGN.md for `flow auto` and architecture
  refactor (7684247)

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
