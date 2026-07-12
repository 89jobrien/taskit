# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Context

`taskit` is a standalone, config-driven Rust binary for running CI pipelines in
any Rust workspace. It uses a multi-crate workspace with hexagonal architecture.

## Build & Run

```bash
cargo run -p taskit -- <subcommand>

# Or, if installed:
taskit <subcommand>

taskit quick                    # fast local feedback
taskit ci                       # full CI pipeline
taskit ci --fail-fast           # stop on first failure
taskit ci --include-network     # include network tests
taskit flow auto                # full pipeline: promote + CI gate + finish to main,
                                #   with LLM conflict resolution; resumes from
                                #   .taskit-state.json if interrupted
taskit init                     # generate taskit.toml + Cruxfile
taskit init --force             # overwrite existing
taskit init --interactive       # interactive prompts
taskit --dry-run <subcommand>   # print without executing
```

## Common Subcommands

| Command                                                  | Purpose                                |
| -------------------------------------------------------- | -------------------------------------- |
| `fmt [--check] [--affected]`                             | Format (or check) all Rust code        |
| `lint [--crate-name X] [--affected]`                     | Run clippy                             |
| `test [--crate-name X] [--affected] [--offline]`         | Run tests via nextest                  |
| `coverage [--crate-name X]`                              | Coverage with 80% threshold            |
| `compile-tests`                                          | Compile test binaries without running  |
| `check-deps`                                             | Check for unused dependencies          |
| `check-protocol-drift [--update] [--warn-only] [--hook]` | Verify core contract hashes            |
| `check-protocol-sites --file F --pattern P --expected N` | Count construction sites for structs   |
| `check-freshness`                                        | Verify drift lockfile freshness        |
| `pre-commit` / `pre-push`                                | Git hook delegates                     |
| `audit`                                                  | Run cargo-deny                         |
| `clean [--older-than Nd]`                                | Clean target/ + prune taskit artifacts |
| `health [--update]`                                      | Measure health, compare to baseline    |
| `inspect [--max-warnings N] [--max-todo N]`              | Check metrics against thresholds       |
| `publish [--skip-docs] [--allow-dirty]`                  | Generate docs and publish to crates.io |
| `init [--force] [--interactive]`                         | Generate taskit.toml, Cruxfile, hooks  |
| `flow status`                                            | Show current branch / staging state    |
| `flow sync`                                              | Merge main -> develop                  |
| `flow promote`                                           | Full pipeline: develop -> staging ->   |
|                                                          | release -> main with CI gate; LLM      |
|                                                          | conflict resolution via BAML;          |
|                                                          | escalates via FlowError::NeedsHuman    |
| `flow guard`                                             | Assert branch invariants               |

## Architecture

### Workspace Structure

```
taskit (root bin)
+-- crates/taskit-types    -- shared types: Config, Error, StepResult, ConflictFile
+-- crates/taskit-core     -- ports: PipelineRunner, ConflictResolver traits
+-- crates/taskit-engine   -- CI pipeline engine, config loading, flow commands
+-- crates/taskit-init     -- `taskit init`: discovery + file generation
+-- crates/taskit-crux     -- EmbeddedCruxRunner (optional, `crux` feature)
+-- crates/taskit-macros   -- proc-macros for taskit derive utilities
+-- crates/taskit-output   -- output formatters (OutputFormatter trait + impls)
+-- crates/taskit-testing  -- shared test helpers and conformance harness
```

### Crate Responsibilities

| Crate              | Role                                                           |
| ------------------ | -------------------------------------------------------------- |
| `taskit`           | Binary entry point; CLI parsing (clap), dispatch, adapters     |
| `taskit-types`     | Leaf crate: Config, TaskitError, StepResult, ConflictFile      |
| `taskit-core`      | Ports only: PipelineRunner, ConflictResolver traits            |
| `taskit-engine`    | CI pipeline, config loading, flow commands, step engine        |
| `taskit-init`      | InitPlan discovery, TOML/Cruxfile rendering, interactive UI    |
| `taskit-crux`      | EmbeddedCruxRunner stub (feature-gated)                        |
| `taskit-macros`    | Proc-macros for derive utilities used across crates            |
| `taskit-output`    | OutputFormatter trait and format implementations               |
| `taskit-testing`   | Shared test helpers; PipelineRunner conformance harness        |

### Key Modules

- **`src/main.rs`** -- CLI parsing and dispatch; Init before config load
- **`taskit-types/config.rs`** -- Config, WorkspaceConfig, CiConfig types
- **`taskit-types/error.rs`** -- TaskitError, ConfigError, PipelineError, etc.
- **`taskit-types/step.rs`** -- StepResult, StepStatus, PipelineOutcome
- **`taskit-types/output_format.rs`** -- OutputFormat enum
- **`taskit-core/pipeline_runner.rs`** -- PipelineRunner trait (port)
- **`taskit-core/conflict_resolver.rs`** -- ConflictResolver trait (port)
- **`taskit-types/conflict.rs`** -- ConflictFile, ResolvedFile domain types
- **`taskit-engine/config.rs`** -- load(), discover(), config parsing
- **`taskit-engine/ci.rs`** -- CI pipeline assembly and step dispatch
- **`taskit-engine/step.rs`** -- Pipeline builder with step/gate/fail-fast
- **`taskit-engine/pipeline_runner.rs`** -- BuiltinRunner, SubprocessCruxRunner
- **`taskit-engine/flow.rs`** -- flow commands: status, promote, sync, guard, auto (auto = promote + CI + finish with resumption)
- **`taskit-init/plan.rs`** -- InitPlan, plan_from_discovery, plan_interactive
- **`taskit-init/render_toml.rs`** -- Hand-built TOML renderer
- **`taskit-init/render_cruxfile.rs`** -- Cruxfile YAML generator
- **`src/flow_resolver.rs`** -- BamlConflictResolver adapter (BAML LLM integration)

### Affected Crate Detection

Configured via `[[workspace.propagation]]` in `taskit.toml`. When a source
crate changes, all listed dependents are automatically included.

### Protocol Drift

`taskit-protocol.lock` at workspace root tracks SHA-256 hashes of contract
surfaces from `[[protocol.surfaces]]`. Use `taskit check-protocol-drift
--update` to regenerate.

## Config Reference (`taskit.toml`)

Key optional sections and their top-level fields:

| Section      | Fields                                                              |
| ------------ | ------------------------------------------------------------------- |
| `[ci]`       | `steps`, `cruxfile`, `fail_fast` (bool — stop on first failure)    |
| `[inspect]`  | `max_clippy_warnings`, `max_clippy_errors`, `max_test_failures`,   |
|              | `max_todo_fixme` (all `usize`; absent = not checked)               |
| `[clean]`    | `older_than` (e.g. `"7d"` — uses `cargo sweep`; absent = full     |
|              | `cargo clean`)                                                      |
| `[release]`  | `github_repo`, `publish_order`, `skip_docs` (bool), `allow_dirty` |
|              | (bool)                                                              |
| `[flow]`     | `main`, `develop`, `staging`, `release` (branch names);           |
|              | `conflict_resolver` (`baml` \| `none` — default: `baml`)           |

CLI flags always override the corresponding config values.

## Testing

```bash
cargo nextest run --workspace                          # all tests
cargo nextest run -p taskit-engine                     # one crate
cargo nextest run -p taskit-engine -E 'test(pipeline)' # filter
```

Tests are colocated in each module under `#[cfg(test)]`.

# currentDate

Today's date is 2026-06-28.
