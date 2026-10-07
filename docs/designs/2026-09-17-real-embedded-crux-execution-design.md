# Design: Real Embedded Crux Execution

## Goal

Execute configured Crux pipelines in-process with full Crux CLI capability parity while keeping Taskit's base build and MSRV independent through an optional feature.

## Approved Approach

Extract the Crux CLI's execution orchestration into a new `crux-executor` crate, then consume that shared API from both `crux-cli` and Taskit's `EmbeddedCruxRunner`.

## Context Map

### Crux Files to Modify

| File                                       | Purpose                    | Changes Needed                                                                   |
| ------------------------------------------ | -------------------------- | -------------------------------------------------------------------------------- |
| `Cargo.toml`                               | Crux workspace manifest    | Add `crux-executor` workspace membership and dependency policy                   |
| `crates/crux-executor/Cargo.toml`          | New executor manifest      | Declare script, handler, plugin, runtime, and trace dependencies                 |
| `crates/crux-executor/src/lib.rs`          | Shared execution API       | Own parsing, registry construction, validation, target resolution, and execution |
| `crates/crux-cli/Cargo.toml`               | CLI manifest               | Depend on `crux-executor` and mirror optional capabilities                       |
| `crates/crux-cli/src/bin/crux/registry.rs` | Existing registry wiring   | Remove duplicated registry construction after migration                          |
| `crates/crux-cli/src/bin/crux/run.rs`      | Existing run orchestration | Render `crux-executor::ExecutionReport` instead of executing directly            |

### Taskit Files to Modify

| File                                  | Purpose                             | Changes Needed                                                       |
| ------------------------------------- | ----------------------------------- | -------------------------------------------------------------------- |
| `Cargo.toml`                          | Workspace and root package manifest | Add optional `taskit-crux` dependency and `embedded-crux` feature    |
| `crates/taskit-crux/Cargo.toml`       | Embedded adapter manifest           | Depend on the released `crux-executor` API and Tokio                 |
| `crates/taskit-crux/src/lib.rs`       | Embedded adapter                    | Replace the synthetic pass with real execution and outcome mapping   |
| `crates/taskit-engine/src/ci.rs`      | CI orchestration                    | Add shared runner outcome finalization                               |
| `crates/taskit-engine/src/command.rs` | Command adapters                    | Add a CI command carrying an external `PipelineRunner`               |
| `src/main.rs`                         | Composition root                    | Select embedded or subprocess Crux according to feature availability |

### Dependencies

- `crux-executor` depends on `crux-script`, `crux-agentic`, `crux-plugin`, `crux-runtime`, and `crux-types`; none depend on `crux-executor`, so no cycle is introduced.
- `crux-cli` delegates execution to `crux-executor` and retains presentation-only behavior.
- `taskit-crux` remains an adapter over `taskit-core::PipelineRunner` and maps executor reports into `taskit-types` outcomes.
- The root Taskit binary is the composition root; `taskit-engine` does not depend on `taskit-crux`.

### Existing Test Coverage

| Test                                                | Current Coverage                   | Required Change                                                     |
| --------------------------------------------------- | ---------------------------------- | ------------------------------------------------------------------- |
| `crates/crux-script/tests/pipeline.rs`              | Pipeline parsing and execution     | Reuse fixtures for executor parity tests                            |
| `crates/crux-script/tests/validation.rs`            | Handler validation                 | Cover strict executor failures                                      |
| `crates/taskit-crux/src/lib.rs` tests               | Synthetic success and missing path | Replace synthetic success with real fixtures and mapping assertions |
| `crates/taskit-engine/src/pipeline_runner.rs` tests | Built-in and subprocess adapters   | Add external-runner command orchestration tests                     |
| `src/main.rs` tests                                 | CLI parsing                        | Add feature-on/off runner selection tests                           |

### Risk

- Public Crux API addition: yes; `crux-executor` becomes a reusable published crate.
- Public Taskit API addition: yes; a new external-runner CI command is added without changing `Ci`.
- Serialization change: no persisted Taskit schema changes.
- CLI behavior change: `[ci].cruxfile` begins affecting `taskit check ci` as documented.
- Cross-repository ordering: Crux must release `crux-executor` before Taskit consumes it.

## Crate Ownership

- **`crux-executor`** owns reusable full-parity pipeline execution. This does not belong in `crux-script`, because `crux-agentic` and `crux-plugin` already depend on `crux-script`; adding reverse dependencies would create cycles.
- **`crux-cli`** owns argument parsing and terminal presentation only.
- **`taskit-crux`** owns conversion from Crux execution reports to Taskit pipeline outcomes.
- **`taskit-engine`** owns CI telemetry and final output handling for all runner implementations.
- **Taskit root binary** owns feature-aware adapter selection.

## Crux Public API

### Types

```rust
#[derive(Debug)]
pub struct ExecuteOptions {
    pub target: Option<String>,
    pub input: serde_json::Value,
    pub plugins_path: Option<std::path::PathBuf>,
    pub strict: bool,
}

impl Default for ExecuteOptions;

#[derive(Debug)]
pub struct ExecutionReport {
    pub units: Vec<ExecutionUnit>,
    pub total: std::time::Duration,
    pub passed: bool,
}

#[derive(Debug)]
pub struct ExecutionUnit {
    pub name: String,
    pub status: ExecutionStatus,
    pub duration: std::time::Duration,
    pub error: Option<String>,
    pub trace: Option<crux_types::Crux<serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: std::path::PathBuf,
        message: String,
    },
    Invalid {
        message: String,
    },
    Plugin {
        message: String,
    },
    UnknownCapabilities {
        handlers: Vec<String>,
        agents: Vec<String>,
    },
    Target {
        target: String,
        message: String,
    },
    Runtime {
        message: String,
    },
}
```

### Functions

```rust
pub async fn execute_file(
    path: &std::path::Path,
    options: &ExecuteOptions,
) -> Result<ExecutionReport, ExecuteError>;
```

`execute_file` auto-detects pipeline YAML versus a multi-target Cruxfile. It returns setup and validation failures as `ExecuteError`; executed pipeline failures are represented in `ExecutionReport`.

## Taskit Public API

### Types

```rust
pub struct EmbeddedCruxRunner {
    cruxfile_path: std::path::PathBuf,
}

pub struct CiWithRunner {
    pub fail_fast: bool,
    pub config_path: std::path::PathBuf,
    pub runner: Box<dyn taskit_core::PipelineRunner>,
}
```

### Functions

```rust
impl EmbeddedCruxRunner {
    pub fn new(cruxfile_path: std::path::PathBuf) -> Self;
}

pub fn run_with_runner(
    ctx: &taskit_engine::Ctx,
    runner: &dyn taskit_core::PipelineRunner,
    config_path: &std::path::Path,
    fail_fast: bool,
) -> Result<(), taskit_types::TaskitError>;
```

The existing `PipelineRunner` trait remains synchronous. `EmbeddedCruxRunner` creates and owns the Tokio runtime bridge for each execution.

## Data Flow

1. Taskit loads `taskit.toml` and resolves `[ci].cruxfile` relative to the workspace root.
2. The composition root selects `EmbeddedCruxRunner` when `embedded-crux` is enabled, otherwise `SubprocessCruxRunner`.
3. `EmbeddedCruxRunner` calls `crux_executor::execute_file` with strict mode enabled and default plugin discovery.
4. `crux-executor` parses the file, builds the same built-in and plugin registry as `crux-cli`, validates all referenced handlers and agents, resolves targets, and executes them in dependency order.
5. `taskit-crux` maps each pipeline or target unit to one `StepResult`, preserving pass, fail, skipped, duration, and error state.
6. `taskit-engine::ci::run_with_runner` records telemetry and writes the selected Taskit output format.

## Result Mapping

- `ExecutionStatus::Pass` maps to `StepStatus::Pass`.
- `ExecutionStatus::Fail` maps to `StepStatus::Fail`.
- `ExecutionStatus::Skipped` maps to `StepStatus::Skipped`.
- Each execution unit becomes one Taskit step, avoiding invalid duration aggregation for parallel internal Crux steps.
- `PipelineOutcome::passed` copies `ExecutionReport::passed` and must satisfy Taskit's conformance invariants.
- Crux traces remain in the executor report; Taskit's initial adapter does not persist them.
- Crux has no Taskit gate field, so mapped steps use `gate: false`.

## Feature And Release Strategy

- Crux publishes `crux-executor` before Taskit integration lands.
- Taskit adds `embedded-crux = ["dep:taskit-crux"]` without including it in default features.
- The base Taskit build can retain its planned Rust 1.88 MSRV; enabling embedded Crux requires Crux's Rust 1.89 MSRV.
- Optional Crux BAML and Docker handlers remain separately feature-gated and are not enabled by Taskit's initial feature.
- Taskit strict mode is always enabled initially; no permissive-stub CLI flag is added.

## Hexagonal Boundaries

- **Crux execution service**: `crux-executor::execute_file` separates reusable orchestration from CLI presentation.
- **Taskit port**: `taskit_core::PipelineRunner` remains the engine-facing pipeline contract.
- **Embedded adapter**: `taskit_crux::EmbeddedCruxRunner` converts Crux reports to Taskit outcomes.
- **Subprocess adapter**: `taskit_engine::SubprocessCruxRunner` remains the feature-disabled fallback.
- **Composition root**: `src/main.rs` selects adapters without creating reverse crate dependencies.

## Verification

- Crux executor tests cover simple pipelines, multi-target ordering, plugin discovery, strict unknown capabilities, failed targets, dependency skips, and trace retention.
- CLI parity tests execute the same fixture through `crux-cli` and `crux-executor` and compare unit names and statuses.
- Taskit adapter tests cover missing and invalid files, success/failure/skipped mapping, duration invariants, and strict-mode behavior.
- Taskit feature matrix checks default features and `--features embedded-crux` separately.
- Taskit CI tests verify `[ci].cruxfile` selects a runner while ordinary configured steps still use built-in execution.

## Out Of Scope

- Making `PipelineRunner` async.
- Persisting Crux traces through Taskit.
- Adding a permissive unknown-handler mode to Taskit.
- Enabling BAML or Docker handlers in Taskit's initial embedded feature.
- Routing `flow auto` through configured Crux execution.
- Changing the generated Cruxfile command vocabulary; that remains tracked separately by issue #4.

## Risk Summary

- [ ] Breaking API changes: no removals; new public APIs only.
- [ ] New external dependency: yes, but only behind Taskit's `embedded-crux` feature.
- [ ] Feature flag required: yes, `embedded-crux`.
- [ ] MSRV impact: none for default Taskit builds; feature-enabled builds require Rust 1.89.
- [ ] Security posture: full CLI handler and plugin parity is intentionally broad; strict validation prevents unknown capabilities from degrading into successful stubs.
