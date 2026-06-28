# Design: Unified Miette Errors in taskit-types

## Goal

Introduce a `taskit-types` crate as the domain leaf, move all shared types
from `taskit-core`, and define `TaskitError` with nested domain enums
implementing `miette::Diagnostic` — replacing `anyhow::Result` at every
public API boundary.

## Approved Approach

Hybrid A+C: all domain error enums defined in `taskit-types` (leaf crate).
Adapter crates use `anyhow` internally and convert to `TaskitError` at
their public API surface.

## Crate Ownership

- **New crate**: `taskit-types` — shared vocabulary: config types, step
  types, output format, and all error enums. Leaf with zero workspace
  crate dependencies.
- **Modified**: `taskit-core` — becomes ports-only (traits). Depends on
  `taskit-types`, re-exports nothing.
- **Modified**: `taskit-engine` — depends on `taskit-types` + `taskit-core`.
  Public functions return `Result<_, TaskitError>`. Existing
  `PipelineError`/`StepError`/`StepDiagnostic` in `output.rs` are removed
  (unified into `TaskitError::Pipeline`). `DiagnosticFormatter` uses
  `TaskitError` directly.
- **Modified**: `taskit-init` — depends on `taskit-types` + `taskit-core`.
  Public `run()` returns `Result<_, TaskitError>`.
- **Modified**: `taskit-crux` — same pattern.
- **Modified**: `taskit` (binary) — installs `miette::set_hook` in `main()`,
  returns `miette::Result`.

## Public API

### taskit-types: Error Types

```rust
// crates/taskit-types/src/error.rs

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum TaskitError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Pipeline(#[from] PipelineError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Protocol(#[from] ProtocolError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Init(#[from] InitError),

    #[error("io error: {0}")]
    #[diagnostic(code(taskit::io))]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    #[error("config file not found: {path}")]
    #[diagnostic(
        code(taskit::config::not_found),
        help("run `taskit init` to generate taskit.toml")
    )]
    NotFound { path: String },

    #[error("failed to parse config")]
    #[diagnostic(code(taskit::config::parse))]
    Parse {
        #[source_code]
        src: NamedSource<String>,
        #[label("parse error here")]
        span: SourceSpan,
        #[source]
        reason: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("invalid config: {message}")]
    #[diagnostic(code(taskit::config::invalid), help("{hint}"))]
    Invalid { message: String, hint: String },
}

#[derive(Debug, Error, Diagnostic)]
pub enum PipelineError {
    #[error("pipeline failed: {failed_count} step(s) failed")]
    #[diagnostic(
        code(taskit::pipeline::failed),
        help("fix the failing steps above, then re-run")
    )]
    Failed {
        failed_count: usize,
        #[source_code]
        src: NamedSource<String>,
        #[label("pipeline result")]
        span: SourceSpan,
        #[related]
        step_errors: Vec<StepError>,
    },

    #[error("gate '{name}' failed, aborting pipeline")]
    #[diagnostic(
        code(taskit::pipeline::gate_failed),
        help("gates are mandatory — fix before continuing")
    )]
    GateFailed {
        name: String,
        #[source]
        reason: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

#[derive(Debug, Error, Diagnostic)]
#[error("step \"{name}\" failed")]
#[diagnostic(severity(error))]
pub struct StepError {
    pub name: String,
    #[help]
    pub detail: Option<String>,
}

#[derive(Debug, Error, Diagnostic)]
pub enum ProtocolError {
    #[error("protocol drift detected in surface '{name}'")]
    #[diagnostic(
        code(taskit::protocol::drift),
        help("run `taskit check-protocol-drift --update` to accept")
    )]
    Drift {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("lockfile not found: {path}")]
    #[diagnostic(
        code(taskit::protocol::lockfile_missing),
        help("run `taskit check-protocol-drift --update` to generate")
    )]
    LockfileMissing { path: String },

    #[error("lockfile is stale")]
    #[diagnostic(
        code(taskit::protocol::stale),
        help("re-run `taskit check-protocol-drift --update`")
    )]
    Stale,
}

#[derive(Debug, Error, Diagnostic)]
pub enum InitError {
    #[error("taskit.toml already exists")]
    #[diagnostic(
        code(taskit::init::exists),
        help("use --force to overwrite")
    )]
    AlreadyExists,

    #[error("cargo metadata failed: {reason}")]
    #[diagnostic(code(taskit::init::metadata))]
    CargoMetadata { reason: String },

    #[error("failed to write {file}: {reason}")]
    #[diagnostic(code(taskit::init::write))]
    WriteFile { file: String, reason: String },
}
```

### taskit-types: Moved Types

These move from `taskit-core` unchanged:

```rust
// crates/taskit-types/src/config.rs   — Config, WorkspaceConfig, etc.
// crates/taskit-types/src/step.rs     — StepResult, StepStatus, PipelineOutcome
// crates/taskit-types/src/output_format.rs — OutputFormat
```

### taskit-types: Module Structure

```rust
// crates/taskit-types/src/lib.rs
pub mod config;
pub mod error;
pub mod output_format;
pub mod step;
```

### taskit-core: Trait (Port) Updates

```rust
// crates/taskit-core/src/pipeline_runner.rs
use std::path::Path;
use taskit_types::error::TaskitError;
use taskit_types::step::PipelineOutcome;

pub trait PipelineRunner {
    fn run_pipeline(
        &self,
        config_path: &Path,
        fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError>;
}
```

```rust
// crates/taskit-core/src/lib.rs
pub mod pipeline_runner;
// config, step, output_format modules removed — live in taskit-types
```

### taskit (binary): main()

```rust
fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    // ...
    match cli.cmd {
        // each arm returns Result<_, TaskitError>
        // TaskitError implements Diagnostic, so miette renders it
    }
}
```

## Data Flow

1. **Config load** (`taskit-engine::config::load`): reads TOML, returns
   `Result<Workspace, TaskitError>` — parse failures become
   `ConfigError::Parse` with source snippet at the boundary.
2. **Pipeline execution** (`taskit-engine::ci::run`): runs steps via
   `PipelineBuilder`, internally uses `anyhow`, converts to
   `PipelineError::Failed` with step diagnostics at the public return.
3. **CLI rendering** (`main()`): `TaskitError` implements `Diagnostic`,
   miette renders it with codes, labels, help text, and source snippets.

## Hexagonal Boundaries

- **Port** (trait): `PipelineRunner` in `taskit-core::pipeline_runner`
- **Adapter** (impl): `BuiltinRunner` in `taskit-engine::pipeline_runner`
- **Adapter** (impl): `SubprocessCruxRunner` in
  `taskit-engine::pipeline_runner`
- **Adapter** (impl): `EmbeddedCruxRunner` in `taskit-crux`
- **Domain types**: all in `taskit-types` (leaf, no adapter knowledge)

## Dependency Graph

```
taskit-types  (leaf: miette, thiserror, serde, clap)
     ^
     |
taskit-core   (taskit-types)
     ^
     |
  +--+--+----------+
  |     |           |
engine init       crux
  |     |           |
  +--+--+----------+
     |
   taskit (binary: miette/fancy)
```

## Migration Path

### Phase 1: Create taskit-types

- New crate with error enums
- Move config.rs, step.rs, output_format.rs from taskit-core
- Add miette (no "fancy") and thiserror deps

### Phase 2: Update taskit-core

- Remove moved modules, depend on taskit-types
- Update PipelineRunner trait signature

### Phase 3: Update taskit-engine

- Depend on taskit-types
- Remove PipelineError/StepError/StepDiagnostic from output.rs
- Update DiagnosticFormatter to use TaskitError::Pipeline
- Convert all public `fn run(...)` returns from `anyhow::Result`
  to `Result<_, TaskitError>`
- Keep anyhow internally, convert at each public function boundary

### Phase 4: Update taskit-init, taskit-crux

- Same boundary conversion pattern

### Phase 5: Update taskit binary

- Return `miette::Result` from main
- Install miette hook for fancy rendering
- Remove anyhow dep

## Out of Scope

- Changing internal error handling within adapter crates (anyhow stays)
- Adding source snippets to errors where byte spans are unavailable
  (use `#[help]`/`#[label]` text instead)
- Async error handling
- Error recovery/retry logic

## Risk

- [x] Breaking API changes: yes — `PipelineRunner` trait signature
      changes from `anyhow::Result` to `Result<_, TaskitError>`. All
      implementors must update. Semver: minor bump (pre-1.0).
- [x] New external dependency: yes — `miette` added to `taskit-types`
      (derive-only, no "fancy" feature). `thiserror` added. Both are
      standard Rust error ecosystem crates.
- [ ] Feature flag required: no
