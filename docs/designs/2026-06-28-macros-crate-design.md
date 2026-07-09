# Design: taskit-macros, taskit-testing, taskit-output

## Goal

Eliminate boilerplate and introduce structured output across the taskit
workspace via three new crates: proc macros, test helpers, and a
format-aware output layer that bridges to crux for tracing.

## Approved Approach

Full suite (Approach C) with three crates. Tracing delegates to crux's
runtime trace model rather than building a custom system. Pipeline
dispatch (`dispatch_cmd`) stays hand-written.

## Crate Ownership

### New Crates

- **`taskit-macros`** (`proc-macro = true`) -- compile-time only
  - Depends on: `syn`, `quote`, `proc-macro2`
  - No runtime dependency on any taskit crate
- **`taskit-testing`** (library) -- test helpers and declarative macros
  - Depends on: `taskit-types`, `tempfile`
  - Dev-dependency of: `taskit-engine`, `taskit-init`, `taskit-crux`
- **`taskit-output`** (library) -- structured message types,
  format-aware rendering, crux trace bridge
  - Depends on: `taskit-types`, `miette`, `serde`, `serde_json`
  - Optional: `taskit-crux` (behind `crux` feature for trace emission)

### Affected Crates

- `taskit-types` -- re-exports `TaskitResultExt` from `taskit-testing`
- `taskit-engine` -- migrates `output.rs` rendering to `taskit-output`,
  replaces ~321 `eprintln!` with structured message API, adopts
  `#[taskit_test]` and `TaskitResultExt`
- `taskit-init` -- adopts `#[taskit_test(tempdir)]`
- `taskit-crux` -- adopts `step_result!` / `single_step_outcome`

## Public API

### taskit-macros

#### `#[taskit_test]` -- composable test attribute

```rust
/// A bare `#[taskit_test]` is equivalent to `#[test]`.
/// Layer on capabilities via arguments:
///
///   tempdir  -- run in a temp dir (set as cwd), inject `dir: &Path`
///   shell    -- inject `sh: &Shell`
///   offline  -- skip when TASKIT_OFFLINE=1
///
/// Injected params are provided by the harness, not the test runner.
/// Original fn signature is rewritten accordingly.
#[proc_macro_attribute]
pub fn taskit_test(
    attr: TokenStream,
    item: TokenStream,
) -> TokenStream;
```

Example usage:

```rust
#[taskit_test(tempdir, shell)]
fn my_test(dir: &Path, sh: &Shell) {
    // cwd is dir, Shell ready, auto-cleanup on drop
}

#[taskit_test(offline)]
fn network_test() {
    // skipped when TASKIT_OFFLINE=1
}
```

#### `#[taskit_pipeline]` -- pipeline metadata generator

```rust
/// Generates `DEFAULT_STEPS` constant and `step_names()` fn from
/// annotated step declarations. Does NOT generate dispatch_cmd.
///
/// Input: a module with step!/gate! invocations.
/// Output: same module with added metadata.
#[proc_macro_attribute]
pub fn taskit_pipeline(
    attr: TokenStream,
    item: TokenStream,
) -> TokenStream;
```

#### `#[derive(ConfigDefaults)]` -- optional-field getter generation

```rust
/// Generates getter methods for `Option<String>` fields that return
/// `&str` with a compile-time default.
///
/// Field attribute: `#[default_value = "main"]`
/// Generates: `pub fn <field>(&self) -> &str`
///
/// Also supports `Option<f64>` with parsed literal defaults.
#[proc_macro_derive(ConfigDefaults, attributes(default_value))]
pub fn derive_config_defaults(
    input: TokenStream,
) -> TokenStream;
```

---

### taskit-testing

#### Declarative Macros

```rust
/// Run a block with a temporary directory set as cwd.
/// Restores original cwd on drop via RAII guard.
///
///   in_temp_dir! { std::fs::write("f.txt", "x").unwrap(); }
///   in_temp_dir! { dir => assert!(dir.exists()); }
#[macro_export]
macro_rules! in_temp_dir { ... }

/// Construct a StepResult with sensible defaults.
///
///   step_result!("lint", Pass)
///   step_result!("test", Fail, error: "assertion failed")
///   step_result!("gate", Pass, gate: true)
///   step_result!("slow", Pass, duration: Duration::from_secs(5))
#[macro_export]
macro_rules! step_result { ... }
```

#### Functions

```rust
/// Single-step PipelineOutcome constructor.
pub fn single_step_outcome(
    name: &str,
    passed: bool,
    duration: Duration,
    error: Option<String>,
) -> PipelineOutcome;
```

#### RAII Guard

```rust
/// Sets cwd to a tempdir and restores on drop.
/// Used by `in_temp_dir!` and `#[taskit_test(tempdir)]`.
pub struct TempDirGuard { ... }

impl TempDirGuard {
    pub fn new() -> Self;
    pub fn path(&self) -> &Path;
}
```

#### Trait Extension

```rust
/// Ergonomic error-context mapping for Result types.
///
/// Replaces:
///   .map_err(|e| TaskitError::other(format!("msg: {e}")))
/// With:
///   .err_context("msg")?
pub trait TaskitResultExt<T> {
    fn err_context(self, msg: &str) -> Result<T, TaskitError>;
    fn err_context_with<F: FnOnce() -> String>(
        self,
        f: F,
    ) -> Result<T, TaskitError>;
}

impl<T, E: std::fmt::Display> TaskitResultExt<T>
    for Result<T, E> { ... }
```

---

### taskit-output

#### Message Types

```rust
/// Structured message emitted during pipeline execution.
/// Replaces raw `eprintln!` calls with typed, routable messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// Step starting/completing/skipping
    StepProgress {
        step: String,
        event: StepEvent,
    },
    /// "Running coverage for {pkg}...", "Checking for unused deps..."
    Progress(String),
    /// "No affected crates detected, skipping."
    Skip(String),
    /// "dry-run: cargo fmt --check --all"
    DryRun(String),
    /// "Pre-commit checks passed.", "Coverage 85% >= 80% -- OK"
    Success(String),
    /// Inline error detail during execution
    Error(String),
    /// Structured diagnostic (clippy warning, test failure location)
    Diagnostic(DiagnosticRecord),
}

#[derive(Debug, Clone)]
pub enum StepEvent {
    Started,
    Passed { duration: Duration },
    Failed { duration: Duration, error: String },
    Skipped,
}
```

#### Message Sink (port)

```rust
/// Port: receives structured messages during pipeline execution.
/// Implementations route to stderr, JSON, crux traces, etc.
pub trait MessageSink {
    fn emit(&self, msg: &Message);
    fn flush(&self);
}
```

#### Built-in Sinks

```rust
/// Human-readable stderr output (default).
/// Replaces all raw `eprintln!` calls.
pub struct StderrSink;

/// Collects messages into a Vec for testing or buffered output.
pub struct BufferSink { ... }

/// Routes messages to crux's trace model.
/// Behind `crux` feature flag.
#[cfg(feature = "crux")]
pub struct CruxTraceSink { ... }

/// Fan-out: sends to multiple sinks simultaneously.
pub struct TeeeSink { ... }
```

#### Output Formatters (moved from taskit-engine)

The `OutputFormatter` trait and all 6 impls (`Human`, `Json`, `Github`,
`Junit`, `Diagnostic`, `Sarif`) move from `taskit-engine/src/output.rs`
to `taskit-output`. The `write_output()` dispatch function moves too.

```rust
pub trait OutputFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String;
}

pub fn formatter_for(format: OutputFormat) -> Box<dyn OutputFormatter>;
pub fn write_output(
    format: OutputFormat,
    outcome: &PipelineOutcome,
) -> Result<(), PipelineError>;
```

`print_summary()` in `step.rs` is removed -- replaced by
`StderrSink` + `HumanFormatter`, eliminating the duplicate column
constants.

#### Convenience Macros

```rust
/// Format-aware message emission. Routes through the active
/// MessageSink rather than writing directly to stderr.
///
///   taskit_progress!("Running coverage for {pkg}...");
///   taskit_skip!("No affected crates, skipping.");
///   taskit_dry!("cargo fmt --check --all");
///   taskit_ok!("Pre-commit checks passed.");
///   taskit_err!("FAILED [{pkg}]: {e}");
#[macro_export]
macro_rules! taskit_progress { ... }
// etc.
```

#### Sink Threading

```rust
/// Set the active message sink for the current thread.
/// Called once at startup in main.rs.
pub fn set_sink(sink: Box<dyn MessageSink>);

/// Get a reference to the active sink. Returns StderrSink if
/// none was set.
pub fn sink() -> &'static dyn MessageSink;
```

Thread-local storage, similar to how `runner::set_dry_run()` works
today. The sink is set once in `main()` based on `--output` format
and optional `--trace` flag.

## Data Flow

1. `main.rs` parses CLI, calls `taskit_output::set_sink()` with the
   appropriate sink (StderrSink, or TeeSink with CruxTraceSink)
2. Engine code uses `taskit_progress!()` etc. instead of `eprintln!()`
3. Messages flow through the `MessageSink` trait
4. Pipeline completion produces `PipelineOutcome`
5. `write_output(format, &outcome)` renders the final summary via
   the appropriate `OutputFormatter`
6. When crux feature is active, `CruxTraceSink` emits trace spans
   for each step and message

## Hexagonal Boundaries

- **Port**: `MessageSink` trait in `taskit-output`
- **Port**: `OutputFormatter` trait in `taskit-output`
- **Adapter**: `StderrSink` (human stderr output)
- **Adapter**: `BufferSink` (testing)
- **Adapter**: `CruxTraceSink` (crux trace bridge, feature-gated)
- **Adapter**: 6 `OutputFormatter` impls (Human, Json, Github, Junit,
  Diagnostic, Sarif)

## Out of Scope

- `dispatch_cmd` generation -- stays hand-written
- Config TOML parsing macros
- CLI argument generation (clap handles this)
- Custom tracing infrastructure -- crux owns tracing
- New output formats beyond the existing 6

## Risk

- Breaking API changes: no -- additive crates, engine migration is
  internal refactor
- New external dependencies:
  - `syn`, `quote`, `proc-macro2` for taskit-macros (standard)
  - `tempfile` promoted from dev-dep to dep for taskit-testing
  - No new deps for taskit-output (uses existing miette, serde)
- Feature flag: `crux` on taskit-output for CruxTraceSink
- Compile time: proc-macro crate adds ~2-4s to clean builds

## Migration Plan

### Phase 1: taskit-testing (no breaking changes)

Create crate with `TempDirGuard`, `in_temp_dir!`, `step_result!`,
`single_step_outcome()`, `TaskitResultExt`. Adopt incrementally in
test code.

| Pattern                                            | Sites | Target                  |
| -------------------------------------------------- | ----- | ----------------------- |
| tempdir + set_current_dir + restore                | ~41   | `in_temp_dir!`          |
| `.map_err(\|e\| TaskitError::other(format!(...)))` | ~18   | `err_context`           |
| `StepResult { ..6 fields.. }` in tests             | ~10   | `step_result!`          |
| single-step PipelineOutcome                        | ~4    | `single_step_outcome()` |

### Phase 2: taskit-macros

Create crate with `#[taskit_test]`, `#[derive(ConfigDefaults)]`,
`#[taskit_pipeline]`. Migrate test sites, config getters, and
pipeline metadata.

| Pattern                            | Sites | Target                      |
| ---------------------------------- | ----- | --------------------------- |
| tempdir tests (proc-macro version) | ~41   | `#[taskit_test(tempdir)]`   |
| `as_deref().unwrap_or()` getters   | ~4    | `#[derive(ConfigDefaults)]` |
| default pipeline step list         | 1     | `#[taskit_pipeline]`        |

### Phase 3: taskit-output

Create crate. Move `OutputFormatter` trait + 6 impls from engine.
Introduce `MessageSink` trait and `Message` enum. Replace `eprintln!`
calls with structured macros. Wire `CruxTraceSink` behind feature.

| Pattern                     | Sites | Target                         |
| --------------------------- | ----- | ------------------------------ |
| raw `eprintln!` calls       | ~321  | `taskit_progress!` etc.        |
| `print_summary()` duplicate | 1     | remove, use formatter          |
| `quick::run()` format-blind | 1     | route through `write_output`   |
| duplicate column constants  | 2     | single source in taskit-output |
