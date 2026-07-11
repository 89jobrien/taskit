# Design: Flow Resumability + Output Overhaul + Config Validation

## Goal

Improve `taskit` across three interlocking dimensions: make `flow auto` resumable after
interruption, replace `NullResolver`/`unreachable!` with type-safe dispatch, emit compact
scannable output by default with flow-phase grouping, and validate `taskit.toml` at load
time with structured diagnostics.

## Approved Approach

Combined: NullResolver elimination (type-safe `FlowAction::Auto`), flow state machine
persisted to `.taskit-state.json`, compact one-line-per-step output with flow-phase
grouping and recovery hints, and `Config::validate()` with `ConfigDiagnostic`.

## Crate Ownership

- **`taskit-types`** — owns `FlowState`, `ConfigDiagnostic`, `OutputConfig`; leaf crate,
  no new external deps
- **`taskit-engine`** — owns resume logic in `flow.rs`, config validation call in
  `config.rs`; all flow state I/O goes here
- **`taskit-output`** — owns `CompactFormatter`; no new deps
- **`taskit` (binary)** — removes `NullResolver`; updates `Flow` construction only

## Public API

### Types — `taskit-types`

```rust
// crates/taskit-types/src/flow_state.rs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowPhase {
    Promoting,
    CiGate,
    Finishing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowState {
    pub phase: FlowPhase,
    pub staging: String,
    pub release: String,
    pub main: String,
    /// SHA of the merge commit on `release` after promote succeeds; None until then.
    pub merge_sha: Option<String>,
    /// Step names that failed in the CI gate phase; empty outside CiGate.
    pub failed_steps: Vec<String>,
}

impl FlowState {
    pub fn promoting(staging: &str, release: &str, main: &str) -> Self;
    pub fn hint(&self) -> &'static str;
}
```

```rust
// crates/taskit-types/src/config.rs  (additions)

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct OutputConfig {
    /// Default output format. Defaults to `OutputFormat::Compact`.
    pub default_format: Option<String>,
    /// If true, expand failing step output even in compact mode. Default: true.
    #[serde(default = "default_verbose_on_failure")]
    pub verbose_on_failure: bool,
}
```

```rust
// crates/taskit-types/src/config.rs  (method additions to Config)

impl Config {
    /// Validate config fields; returns all diagnostics (errors and warnings).
    /// Callers should treat any `DiagnosticSeverity::Error` as fatal.
    pub fn validate(&self) -> Vec<ConfigDiagnostic>;
}
```

### Types — `taskit-engine`

```rust
// crates/taskit-engine/src/flow_state_store.rs

/// Read `.taskit-state.json` from workspace root; returns None if absent or unreadable.
pub fn load(root: &std::path::Path) -> Option<FlowState>;

/// Write `.taskit-state.json` atomically (write-then-rename).
pub fn save(root: &std::path::Path, state: &FlowState) -> Result<(), TaskitError>;

/// Delete `.taskit-state.json`; no-op if absent.
pub fn clear(root: &std::path::Path) -> Result<(), TaskitError>;
```

### Enum change — `taskit-engine`

```rust
// crates/taskit-engine/src/command.rs

#[non_exhaustive]
pub enum FlowAction {
    Status,
    Promote,
    Finish,
    Guard,
    Auto {
        resolver: Box<dyn taskit_core::ConflictResolver>,
        ci_runner: Box<dyn Fn(&Ctx) -> PipelineOutcome + Send + Sync>,
    },
}

pub struct Flow {
    pub action: FlowAction,
}
```

### Formatter — `taskit-output`

```rust
// crates/taskit-output/src/formatter.rs  (addition)

pub struct CompactFormatter {
    pub verbose_on_failure: bool,
}

impl OutputFormatter for CompactFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String;
}
```

```rust
// crates/taskit-types/src/output_format.rs  (addition)

pub enum OutputFormat {
    #[default]
    Compact,   // NEW — replaces Human as default
    Human,
    Json,
    Github,
    Junit,
    Diagnostic,
    Sarif,
}
```

### Config — `taskit-types`

```rust
// crates/taskit-types/src/config.rs  (addition to Config struct)

pub struct Config {
    // ... existing fields ...
    #[serde(default)]
    pub output: OutputConfig,
}
```

## Data Flow

### Flow resumability

1. **Source**: `flow::auto()` called with `resolver` and `ci_runner`
2. **Read**: `flow_state_store::load(root)` — if `Some(state)`, skip phases already past
3. **Promoting**: call `merge_with_resolution`; on success write state `FlowPhase::CiGate`
   with `merge_sha`
4. **CiGate**: call `ci_runner`; on pass write state `FlowPhase::Finishing`; on fail write
   `failed_steps` and return `FlowError::CiFailed`
5. **Finishing**: call `finish`; on success call `flow_state_store::clear(root)`
6. **Sink**: on any `Err`, state file remains for next resume; on success it is deleted

### Config validation

1. **Source**: `taskit-engine::config::load()` parses TOML into `Config`
2. **Transform**: calls `config.validate()` → `Vec<ConfigDiagnostic>`
3. **Sink**: any `DiagnosticSeverity::Error` → return `Err(ConfigError::Invalid(diagnostics))`;
   warnings emitted via `taskit_output::taskit_warn!`

### Compact output

1. **Source**: `PipelineOutcome` with `Vec<StepResult>`
2. **Transform**: `CompactFormatter::render()` — one `✓`/`✗` line per step; on failure
   appends the step's `error` field if `verbose_on_failure` is true
3. **Sink**: returned `String` printed by `main.rs` formatter dispatch

## Hexagonal Boundaries

- **Port** (trait): `OutputFormatter` in `taskit-output::formatter` — unchanged
- **Adapter** (impl): `CompactFormatter` in `taskit-output::formatter`
- **Port** (trait): `ConflictResolver` in `taskit-core::conflict_resolver` — unchanged
- **Adapter** (impl): `BamlConflictResolver` in `taskit::flow_resolver` — unchanged
- **I/O boundary**: `flow_state_store` in `taskit-engine` wraps all `.taskit-state.json`
  reads/writes — the rest of `flow.rs` never touches the filesystem directly for state

## Out of Scope

- Watch mode / live reloading
- New CI steps
- BAML changes
- Breaking changes to `--output json` format
- Config migration / backwards-compat shims for old `taskit.toml` files
- `Compact` becoming the default in `--output` clap arg (stays `Human` for CLI compat;
  `Compact` is the default only when `[output]` section absent in `taskit.toml`)

## Risk

- [ ] Breaking API change: `FlowAction::Auto` gains fields — all engine consumers updated
      in same plan (one call site: `command.rs:346`)
- [ ] `OutputFormat::Compact` added — additive, `#[non_exhaustive]` already on enum
- [ ] `.taskit-state.json` new file — must add to `.gitignore` in `taskit-init` scaffold
      and workspace root `.gitignore`
- [ ] No new external dependencies
- [ ] No feature flag required — all changes are behind existing public API
