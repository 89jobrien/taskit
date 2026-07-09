# Design: Pipeline Diagnostics Mode

## Goal

Improve CI failure reporting by attaching command provenance, reproduction hints, workspace context, and richer failure summaries to pipeline outcomes.

## Approved Approach

Pipeline Diagnostics Mode was selected from brainstorm: improve `taskit ci` diagnostics so failures explain what ran, where it ran, and how to reproduce the failed step locally.

## Crate Ownership

- **Owner crate**: `taskit-engine` — owns pipeline execution, command dispatch, and collection of runtime context from `Ctx`.
- **Affected crates**:
  - `taskit-types` — owns public pipeline result data structures extended with diagnostic context.
  - `taskit-output` — owns rendering the enriched diagnostics for human, diagnostic, JSON, GitHub, JUnit, and SARIF outputs.
  - `taskit` — owns CLI help text only if wording changes are needed; no new CLI parser shape is required.

## Context Map

### Files to Modify

| File | Purpose | Changes Needed |
| --- | --- | --- |
| `crates/taskit-types/src/step.rs` | Public `PipelineOutcome`, `StepResult`, and diagnostic data model | Add run-level and step-level diagnostic context types and fields |
| `crates/taskit-engine/src/ctx.rs` | Command execution boundary for `xshell::Cmd` | Record commands executed through `Ctx::run` and `Ctx::run_capture`; collect process/workspace provenance |
| `crates/taskit-engine/src/step.rs` | Pipeline execution and `StepResult` construction | Drain step diagnostic context sinks into each step result; attach run context to the outcome |
| `crates/taskit-engine/src/ci.rs` | CI pipeline assembly and configured step dispatch | Wrap CI step closures with command capture, provide reproduction commands, and ensure empty/dispatch-failure outcomes include context |
| `crates/taskit-output/src/formatter.rs` | Output rendering adapters | Render failure diagnostics in human/diagnostic/github/json outputs; include command context in machine-readable outputs |
| `crates/taskit-output/src/lib.rs` | Formatter re-exports | Re-export any new helper if it is public |
| `src/main.rs` | CLI help text | Optional wording update for existing `--output diagnostic` format |

### Dependencies

| File | Relationship |
| --- | --- |
| `crates/taskit-engine/src/quick.rs` | Uses `Ctx::with_silent` and pipeline-style command execution; must continue to receive command output on failure |
| `crates/taskit-engine/src/lint.rs` | Uses `Ctx::run` and `Ctx::run_capture`; command recording must not break structured diagnostic capture |
| `crates/taskit-engine/src/testing/run.rs` | Uses `Ctx::run` and `Ctx::run_capture`; command recording must not break nextest JSON parsing |
| `crates/taskit-engine/src/testing/compile.rs` | Uses `Ctx::run`; compile-test failures should show the failed package command |
| `crates/taskit-types/src/error.rs` | `PipelineError` and `StepError` currently surface failed step details; may include reproduction hints without changing error variants |
| `crates/taskit-output/src/message.rs` | Existing runtime message model references `DiagnosticRecord`; no new message variant is required |

### Test Coverage

| Test Location | Covers |
| --- | --- |
| `crates/taskit-types/src/step.rs` inline tests | Public result type construction and diagnostic record preservation |
| `crates/taskit-engine/src/ctx.rs` inline tests | `Ctx` accessors, dry-run, silent mode behavior |
| `crates/taskit-engine/src/step.rs` inline tests | Pipeline pass/fail/fail-fast/gate behavior and `StepResult` construction |
| `crates/taskit-engine/src/ci.rs` inline tests | Configured step dispatch and empty CI behavior |
| `crates/taskit-output/src/formatter.rs` inline tests | Human, JSON, GitHub, JUnit, diagnostic, SARIF formatter invariants |
| Gap | No current tests assert command provenance, reproduction hints, run context, or GitHub summary diagnostics |

### Reference Patterns

| File | Pattern to Follow |
| --- | --- |
| `crates/taskit-output/src/formatter.rs` | Existing `OutputFormatter` port and adapter implementations |
| `crates/taskit-engine/src/lint.rs` | Structured capture path returning `(success, diagnostics)` |
| `crates/taskit-engine/src/testing/run.rs` | Structured nextest capture and parser boundary |
| `crates/taskit-engine/src/discovery.rs` | Cargo metadata adapter pattern using `cargo_metadata` without adding dependencies |
| `crates/taskit-engine/src/ctx.rs` | Centralized command execution through `Ctx::run` and `Ctx::run_capture` |

## Public API

### Traits

No new public trait is required. The existing `taskit_output::OutputFormatter` port remains the formatter boundary.

### Types

```rust
// crates/taskit-types/src/step.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRecord {
    pub command: String,
    pub success: Option<bool>,
    pub exit_code: Option<i32>,
}
```

```rust
// crates/taskit-types/src/step.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepDiagnosticContext {
    pub commands: Vec<CommandRecord>,
    pub reproduction: Option<String>,
    pub notes: Vec<String>,
}
```

```rust
// crates/taskit-types/src/step.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PipelineRunContext {
    pub taskit_binary: Option<String>,
    pub taskit_version: String,
    pub workspace_root: String,
    pub git_sha: Option<String>,
    pub rustc_version: Option<String>,
    pub cargo_version: Option<String>,
    pub workspace_members: Vec<String>,
}
```

```rust
// crates/taskit-types/src/step.rs
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration: Duration,
    pub error: Option<String>,
    pub gate: bool,
    pub diagnostics: Vec<DiagnosticRecord>,
    pub context: StepDiagnosticContext,
}
```

```rust
// crates/taskit-types/src/step.rs
#[derive(Debug)]
pub struct PipelineOutcome {
    pub results: Vec<StepResult>,
    pub total: Duration,
    pub passed: bool,
    pub context: Option<PipelineRunContext>,
}
```


### Functions

```rust
// crates/taskit-engine/src/ctx.rs
impl Ctx {
    pub fn command_capture_start(&self) -> usize;
    pub fn command_capture_finish(&self, start_index: usize) -> Vec<CommandRecord>;
    pub fn pipeline_run_context(&self) -> PipelineRunContext;
}
```
```rust
// crates/taskit-engine/src/step.rs
pub type StepContextSink = Rc<RefCell<StepDiagnosticContext>>;
```

```rust
// crates/taskit-engine/src/step.rs
impl<'a> Pipeline<'a> {
    pub fn with_context(self, context: PipelineRunContext) -> Self;
    pub fn step_with_context_sink(
        self,
        name: &str,
        sink: StepContextSink,
        f: impl FnOnce() -> Result<(), TaskitError> + 'a,
    ) -> Self;
    pub fn gate_with_context_sink(
        self,
        name: &str,
        sink: StepContextSink,
        f: impl FnOnce() -> Result<(), TaskitError> + 'a,
    ) -> Self;
}
```

```rust
// crates/taskit-engine/src/ci.rs
pub(crate) fn reproduction_for_ci_step(cmd: &str) -> String;
pub(crate) fn diagnostic_context_for_ci_step(cmd: &str) -> StepDiagnosticContext;
```

```rust
// crates/taskit-output/src/formatter.rs
pub fn render_failure_diagnostics(outcome: &PipelineOutcome) -> String;
```

## Data Flow

1. Source: `Ctx::run` and `Ctx::run_capture` record command strings and best-effort outcomes whenever a step executes external tools.
2. Source: `Ctx::pipeline_run_context` gathers runtime provenance from the current executable path, package version, workspace root, Cargo metadata, and best-effort local tool/version commands.
3. Transform: `ci.rs` creates a `StepContextSink` for each CI step, preloads the reproduction command, starts command capture immediately before invoking the step closure, finishes capture afterward, and appends the resulting `CommandRecord`s to the sink.
4. Transform: `Pipeline::run` drains each step's `StepContextSink` into `StepResult.context` after the closure completes.
5. Sink: `taskit-output` renderers include the enriched context in failure output, GitHub annotations/step summary, and JSON output.

## Hexagonal Boundaries

- **Port**: `OutputFormatter` in `taskit-output::formatter` remains the rendering boundary.
- **Adapter**: `HumanFormatter`, `DiagnosticFormatter`, `GithubFormatter`, `JsonFormatter`, `JunitFormatter`, and `SarifFormatter` render the same enriched `PipelineOutcome` into different output targets.
- **Command execution boundary**: `Ctx` remains the adapter around process execution. CI step wrappers consume recorded command metadata through `Ctx` capture helpers instead of having the pipeline call processes directly.

## Rendering Rules

- Human and diagnostic failure output must include:
  - failed step name and duration;
  - primary error string;
  - commands attempted during the step;
  - reproduction command;
  - run context containing taskit binary path, taskit version, workspace root, git SHA when available, Cargo/Rust versions when available, and workspace member list.
- GitHub output must include:
  - one `::error` annotation per failed step;
  - a `$GITHUB_STEP_SUMMARY` section named `Pipeline diagnostics` with reproduction command and run context.
- JSON output must remain valid JSON and include additive fields:
  - `context` at the top level;
  - `context` per step;
  - existing `version` remains `1` unless a compatibility test shows consumers require a version bump.
- JUnit output remains focused on test-suite compatibility and only includes the richer error text inside failure messages.
- SARIF output remains focused on structured tool diagnostics and does not duplicate run context unless later required by SARIF consumers.

## Out of Scope

- Generating new GitHub workflow templates.
- Posting PR comments or calling the GitHub API.
- Parsing every Cargo tool output into structured source diagnostics beyond the existing clippy and nextest capture paths.
- Replacing existing output formats or removing `--output diagnostic`.
- Adding external dependencies.

## Risk

- [x] Breaking API changes: yes — adding public fields to `StepResult` and `PipelineOutcome` requires internal tests and any downstream struct literals to be updated.
- [x] New external dependency: no.
- [x] Feature flag required: no.
- [x] Serialization format change: yes for JSON output, but additive only.
- [x] CLI output change: yes for failed human/diagnostic/GitHub output; passing output should remain compact.
