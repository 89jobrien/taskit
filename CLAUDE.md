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

## Architecture

### Workspace Structure

```
taskit (root bin)
+-- crates/taskit-types   -- shared types: Config, Error, StepResult, OutputFormat
+-- crates/taskit-core    -- ports: PipelineRunner trait
+-- crates/taskit-engine  -- CI pipeline engine, config loading, formatters
+-- crates/taskit-init    -- `taskit init`: discovery + file generation
+-- crates/taskit-crux    -- EmbeddedCruxRunner (optional, `crux` feature)
```

### Crate Responsibilities

| Crate           | Role                                                        |
| --------------- | ----------------------------------------------------------- |
| `taskit`        | Binary entry point; CLI parsing (clap) and dispatch         |
| `taskit-types`  | Leaf crate: Config, TaskitError, StepResult, OutputFormat   |
| `taskit-core`   | Ports only: PipelineRunner trait                            |
| `taskit-engine` | CI pipeline, config loading, output formatters, step engine |
| `taskit-init`   | InitPlan discovery, TOML/Cruxfile rendering, interactive UI |
| `taskit-crux`   | EmbeddedCruxRunner stub (feature-gated)                     |

### Key Modules

- **`src/main.rs`** -- CLI parsing and dispatch; Init before config load
- **`taskit-types/config.rs`** -- Config, WorkspaceConfig, CiConfig types
- **`taskit-types/error.rs`** -- TaskitError, ConfigError, PipelineError, etc.
- **`taskit-types/step.rs`** -- StepResult, StepStatus, PipelineOutcome
- **`taskit-types/output_format.rs`** -- OutputFormat enum
- **`taskit-core/pipeline_runner.rs`** -- PipelineRunner trait (port)
- **`taskit-engine/config.rs`** -- load(), discover(), config parsing
- **`taskit-engine/ci.rs`** -- CI pipeline assembly and step dispatch
- **`taskit-engine/step.rs`** -- Pipeline builder with step/gate/fail-fast
- **`taskit-engine/pipeline_runner.rs`** -- BuiltinRunner, SubprocessCruxRunner
- **`taskit-engine/output.rs`** -- OutputFormatter trait + 5 format impls
- **`taskit-init/plan.rs`** -- InitPlan, plan_from_discovery, plan_interactive
- **`taskit-init/render_toml.rs`** -- Hand-built TOML renderer
- **`taskit-init/render_cruxfile.rs`** -- Cruxfile YAML generator

### Affected Crate Detection

Configured via `[[workspace.propagation]]` in `taskit.toml`. When a source
crate changes, all listed dependents are automatically included.

### Protocol Drift

`taskit-protocol.lock` at workspace root tracks SHA-256 hashes of contract
surfaces from `[[protocol.surfaces]]`. Use `taskit check-protocol-drift
--update` to regenerate.

## Testing

```bash
cargo nextest run --workspace                          # all tests
cargo nextest run -p taskit-engine                     # one crate
cargo nextest run -p taskit-engine -E 'test(pipeline)' # filter
```

Tests are colocated in each module under `#[cfg(test)]`.

# currentDate

Today's date is 2026-06-28.
