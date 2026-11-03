# taskit-types

`taskit-types` is the shared contract crate for the taskit workspace. It owns configuration,
pipeline values, conflict payloads, persisted flow state, output selection, and typed diagnostics.
It has no dependency on another taskit crate, so all higher layers can share these types without a
dependency cycle.

## Workspace role

Most workspace crates depend directly on `taskit-types`. Put cross-crate data contracts here;
keep behavior that invokes tools or orchestrates commands in `taskit-engine`, and keep abstract
ports in `taskit-core`.

The crate has no Cargo feature flags.

## Public modules

| Module          | Main exports                                                      |
| --------------- | ----------------------------------------------------------------- |
| `config`        | `Config`, section structs, diagnostics, defaults, and validation. |
| `conflict`      | `ConflictFile`, `ResolvedFile`.                                   |
| `error`         | `TaskitError`, domain errors, `StepError`, `TaskitResultExt`.     |
| `flow_state`    | `FlowPhase`, serializable `FlowState`.                            |
| `output_format` | Clap-compatible `OutputFormat`.                                   |
| `step`          | Pipeline outcomes, step results, diagnostics, and run provenance. |

`ConflictFile` and `ResolvedFile` are re-exported from the crate root. Other types are addressed
through their modules.

## Configuration contract

`Config` deserializes `taskit.toml` into these sections:

| Section       | Type              | Purpose                                                  |
| ------------- | ----------------- | -------------------------------------------------------- |
| `[workspace]` | `WorkspaceConfig` | Crates, propagation, root, and offline test filter.      |
| `[protocol]`  | `ProtocolConfig`  | Tracked files and protocol lockfile path.                |
| `[ci]`        | `CiConfig`        | Ordered steps, optional Cruxfile, and fail-fast default. |
| `[coverage]`  | `CoverageConfig`  | Default crate and threshold.                             |
| `[flow]`      | `FlowConfig`      | Branches, conflict resolver, and optional push behavior. |
| `[release]`   | `ReleaseConfig`   | GitHub repository and publish settings.                  |
| `[inspect]`   | `InspectConfig`   | Optional quality thresholds.                             |
| `[clean]`     | `CleanConfig`     | Optional artifact age policy.                            |
| `[output]`    | `OutputConfig`    | Default format and compact failure verbosity.            |

Important defaults and helpers include:

- coverage defaults to `DEFAULT_COVERAGE_THRESHOLD` (`80.0`) when absent or non-positive;
- protocol lockfiles default to `taskit-protocol.lock`;
- flow branches default to `main`, `develop`, `staging`, and `release`;
- flow conflict resolution defaults to `baml`, while pushing defaults to disabled and `origin`;
- `CrateEntry::pkg_name` uses `pkg` when set and otherwise uses `dir`; and
- `OutputConfig::verbose_on_failure` defaults to `true` when deserialized.

`Config::validate` returns all discovered diagnostics instead of stopping at the first one. It
currently rejects invalid coverage percentages and empty/duplicate flow branch names, and warns
when `release.github_repo` is not in `owner/name` form. Callers decide that error-severity
diagnostics are fatal.

## Pipeline contracts

`StepResult` records the step name, `Pass`/`Fail`/`Skipped` status, duration, optional error, gate
flag, structured diagnostics, and `StepDiagnosticContext`. The context can retain attempted
commands, a local reproduction command, and free-form notes.

`PipelineOutcome` aggregates step results, total duration, overall pass/fail state, and optional
`PipelineRunContext`. Run context records taskit/tool versions, workspace root and members, binary
path, and git SHA when available. These fields feed the human, JSON, GitHub, JUnit, diagnostic, and
SARIF formatters in `taskit-output`.

```rust
use std::time::Duration;

use taskit_types::step::{PipelineOutcome, StepDiagnosticContext, StepResult, StepStatus};

let outcome = PipelineOutcome {
    results: vec![StepResult {
        name: "fmt --check".into(),
        status: StepStatus::Pass,
        duration: Duration::from_millis(120),
        error: None,
        gate: false,
        diagnostics: vec![],
        context: StepDiagnosticContext::default(),
    }],
    total: Duration::from_millis(120),
    passed: true,
    context: None,
};

assert!(outcome.passed);
```

## Errors and diagnostics

`TaskitError` wraps configuration, pipeline, protocol, initialization, flow, I/O, and contextual
errors. Domain variants derive `miette::Diagnostic` and expose stable diagnostic codes. `FlowError`
is non-exhaustive so consumers must allow new flow failures.

`TaskitResultExt` maps any displayable error into `TaskitError::Other`:

```rust
use taskit_types::error::{TaskitError, TaskitResultExt};

fn read_config() -> Result<String, TaskitError> {
    std::fs::read_to_string("taskit.toml").err_context("reading taskit config")
}
```

## Conflict and flow state

`ConflictFile` contains the relative path, ours/theirs content, and optional raw conflict-marked
base content. `ResolvedFile` contains a relative path and final content. Both structs are
non-exhaustive.

`FlowState` is serde-serializable state for resuming `flow auto`. It tracks the `Promoting`,
`CiGate`, or `Finishing` phase, branch names, merge SHA, and failed CI steps. `hint` returns the
phase-specific recovery command.

## Output formats

`OutputFormat` derives `clap::ValueEnum` and supports `human`, `compact`, `json`, `github`,
`junit`, `diagnostic`, and `sarif`. Rendering and destinations are implemented by `taskit-output`.

## Examples and development

```bash
cargo run -p taskit-types --example build_outcome
cargo check -p taskit-types
cargo clippy -p taskit-types --all-targets -- -D warnings
cargo nextest run -p taskit-types
```

Tests include unit and property tests plus a full TOML fixture in
`tests/fixtures/full-taskit.toml`. Changes to `src/error.rs` are protocol-surface changes in this
workspace and may require an intentional lockfile update.
