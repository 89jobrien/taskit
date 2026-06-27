# Design: `taskit init` + PipelineRunner Port

## Goal

Add `taskit init` to generate `taskit.toml` + Cruxfile from auto-discovery,
and refactor CI execution behind a `PipelineRunner` port so taskit can run
Cruxfiles natively (subprocess or embedded via `crux-script`). Split into a
multi-crate workspace for dependency isolation.

## Approved Approach

Port + both adapters with `crux-script` behind a feature flag. Two-phase
`InitPlan` / renderer for config generation. `dialoguer` for interactive
mode. Five-crate workspace (Split D) with thin binary.

## Workspace Structure

```
taskit/
  Cargo.toml              (workspace root)
  crates/
    taskit-core/           shared types, PipelineRunner trait
    taskit-engine/         Pipeline, CI, output, discovery, BuiltinRunner,
                           SubprocessCruxRunner
    taskit-init/           InitPlan, renderers, interactive prompts
    taskit-crux/           EmbeddedCruxRunner
    taskit/                thin bin: CLI parsing + wiring
```

## Crate Ownership

| Crate           | Responsibility                                                                                                                                           | Heavy deps                                                                |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `taskit-core`   | Config types, `PipelineRunner` trait, `PipelineOutcome`, `StepResult`, `StepStatus`, `OutputFormat`                                                      | `serde`, `clap` (derive only)                                             |
| `taskit-engine` | `Pipeline`, CI orchestration, output formatters, `BuiltinRunner`, `SubprocessCruxRunner`, discovery, affected-crate detection, all existing step modules | existing deps (`xshell`, `sha2`, `indicatif`, `cargo_metadata`, `miette`) |
| `taskit-init`   | `InitPlan`, `plan_from_discovery`, `render_toml`, `render_cruxfile`, interactive prompts                                                                 | `dialoguer`                                                               |
| `taskit-crux`   | `EmbeddedCruxRunner`                                                                                                                                     | `crux-script`, `tokio`                                                    |
| `taskit` (bin)  | CLI parsing (clap), subcommand dispatch, composition root                                                                                                | `clap`                                                                    |

### Dependency graph

```
taskit (bin) ──► taskit-engine ──► taskit-core
    │                                  ▲
    ├──► taskit-init ──────────────────┘
    │
    └──► taskit-crux (optional) ──► taskit-core
```

- `taskit-core` has zero workspace dependencies
- `taskit-engine` depends on `taskit-core`
- `taskit-init` depends on `taskit-core`
- `taskit-crux` depends on `taskit-core`
- `taskit` (bin) depends on all four; `taskit-crux` is optional via
  `--features crux`

## Context Map

### Files to Create

| Crate           | Key files                                                                                                   |
| --------------- | ----------------------------------------------------------------------------------------------------------- |
| `taskit-core`   | `src/lib.rs` (re-exports), `src/config.rs`, `src/pipeline_runner.rs`, `src/step.rs`, `src/output_format.rs` |
| `taskit-engine` | `src/lib.rs`, inherits most of current `src/`                                                               |
| `taskit-init`   | `src/lib.rs`, `src/plan.rs`, `src/render_toml.rs`, `src/render_cruxfile.rs`, `src/interactive.rs`           |
| `taskit-crux`   | `src/lib.rs` (EmbeddedCruxRunner)                                                                           |
| `taskit` (bin)  | `src/main.rs` (thin CLI)                                                                                    |

### Files to Move (from current flat layout)

| Current file                                             | Destination crate                          |
| -------------------------------------------------------- | ------------------------------------------ |
| `src/config.rs` (types only)                             | `taskit-core`                              |
| `src/step.rs` (StepResult, StepStatus, PipelineOutcome)  | `taskit-core`                              |
| `src/output.rs` (OutputFormat enum)                      | `taskit-core`                              |
| `src/step.rs` (Pipeline, print_summary)                  | `taskit-engine`                            |
| `src/ci.rs`                                              | `taskit-engine`                            |
| `src/output.rs` (formatters, miette types, write_output) | `taskit-engine`                            |
| `src/discovery.rs`                                       | `taskit-engine`                            |
| `src/config.rs` (load, find_config_file, discover)       | `taskit-engine`                            |
| `src/affected.rs`                                        | `taskit-engine`                            |
| `src/runner.rs`                                          | `taskit-engine`                            |
| `src/fmt.rs`, `src/lint.rs`, `src/testing/` etc.         | `taskit-engine`                            |
| `src/main.rs`                                            | `taskit` (bin, rewritten as thin dispatch) |

### Risk

- [x] Breaking API change: `CiConfig` gains `cruxfile` field -- additive,
      `#[serde(default)]`
- [x] New external deps: `dialoguer` (taskit-init), `crux-script` + `tokio`
      (taskit-crux, optional)
- [x] Feature flag required: `crux` on the bin crate
- [x] Workspace restructure: all existing imports change from
      `crate::` / `taskit::` to `taskit_core::` / `taskit_engine::`
- [x] `crux-script` is a local path dep until crux is published

## Public API

### taskit-core

```rust
// crates/taskit-core/src/pipeline_runner.rs
pub trait PipelineRunner {
    fn run_pipeline(
        &self,
        config_path: &Path,
        fail_fast: bool,
    ) -> anyhow::Result<PipelineOutcome>;
}

// crates/taskit-core/src/step.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus { Pass, Fail, Skipped }

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration: Duration,
    pub error: Option<String>,
    pub gate: bool,
}

#[derive(Debug)]
pub struct PipelineOutcome {
    pub results: Vec<StepResult>,
    pub total: Duration,
    pub passed: bool,
}

// crates/taskit-core/src/output_format.rs
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Github,
    Junit,
    Diagnostic,
}

// crates/taskit-core/src/config.rs
pub struct Config { ... }
pub struct WorkspaceConfig { ... }
pub struct CrateEntry { ... }
pub struct PropagationEntry { ... }
pub struct ProtocolConfig { ... }
pub struct SurfaceEntry { ... }
pub struct CoverageConfig { ... }

pub struct CiConfig {
    pub steps: Vec<CiStep>,
    pub cruxfile: Option<String>,  // NEW
}

pub struct CiStep { ... }
```

### taskit-engine

```rust
// crates/taskit-engine/src/pipeline_runner.rs
pub struct BuiltinRunner<'a> {
    pub sh: &'a Shell,
    pub ws: &'a WorkspaceConfig,
    pub proto: Option<&'a ProtocolConfig>,
    pub cov: Option<&'a CoverageConfig>,
    pub ci: Option<&'a CiConfig>,
    pub offline: bool,
}

pub struct SubprocessCruxRunner {
    pub cruxfile_path: PathBuf,
}

// Both implement taskit_core::PipelineRunner
```

### taskit-init

```rust
// crates/taskit-init/src/plan.rs
pub struct InitPlan {
    pub crates: Vec<CrateEntry>,
    pub propagation: Vec<PropagationEntry>,
    pub surfaces: Vec<SurfaceEntry>,
    pub coverage: Option<CoveragePlan>,
    pub ci_steps: Vec<CiStepPlan>,
    pub offline_skip: Option<String>,
}

pub struct CoveragePlan {
    pub crate_name: String,
    pub threshold: f64,
}

pub struct CiStepPlan {
    pub name: String,
    pub cmd: String,
    pub gate: bool,
}

pub fn plan_from_discovery(root: &Path) -> anyhow::Result<InitPlan>;

// crates/taskit-init/src/render_toml.rs
pub fn render_toml(plan: &InitPlan) -> String;

// crates/taskit-init/src/render_cruxfile.rs
pub fn render_cruxfile(plan: &InitPlan, project_name: &str) -> String;

// crates/taskit-init/src/interactive.rs
pub fn refine_plan(plan: &mut InitPlan) -> anyhow::Result<()>;

// crates/taskit-init/src/lib.rs
pub fn run(force: bool, interactive: bool) -> anyhow::Result<()>;
```

### taskit-crux

```rust
// crates/taskit-crux/src/lib.rs
pub struct EmbeddedCruxRunner {
    pub cruxfile_path: PathBuf,
}

// Implements taskit_core::PipelineRunner
// Uses tokio::runtime::Runtime::block_on internally
```

## Data Flow

### `taskit init`

1. **Source**: `taskit-engine::Config::discover()` reads cargo metadata
2. **Transform**: `taskit-init::plan_from_discovery()` maps into `InitPlan`
3. **Render**: `render_toml()` + `render_cruxfile()` produce file contents
4. **Sink**: write `taskit.toml` and `ci.crux` to workspace root
   (interactive mode: `dialoguer` per-section prompts via `refine_plan()`)

### `taskit ci` with Cruxfile

1. **Source**: `taskit-engine::config::load()` reads `taskit.toml`
2. **Dispatch** (in bin crate): `ci.cruxfile` present?
   - Yes + `crux` feature: `EmbeddedCruxRunner` (block_on)
   - Yes + no feature: `SubprocessCruxRunner` (shells out)
   - No: `BuiltinRunner` (existing Pipeline logic)
3. **Sink**: `PipelineOutcome` passed to `write_output()`

## Hexagonal Boundaries

- **Port** (trait): `PipelineRunner` in `taskit-core::pipeline_runner`
- **Adapter**: `BuiltinRunner` in `taskit-engine` -- wraps `Pipeline`
- **Adapter**: `SubprocessCruxRunner` in `taskit-engine` -- shells out
  to `crux run`, parses JSON stdout into `PipelineOutcome`
- **Adapter**: `EmbeddedCruxRunner` in `taskit-crux` -- links
  `crux_script::Runner`, converts `Crux<Value>` into `PipelineOutcome`

## Cruxfile Format

`taskit init` generates `ci.crux`:

```yaml
project: <workspace-name>
default: ci
budget: { calls: 50 }

targets:
  gate:
    steps:
      - step: self_check
        handler: shell::exec
        args:
          cmd: "taskit self-check"

  lint:
    depends: [gate]
    steps:
      - join_all: checks
        arms:
          - step: fmt_check
            handler: shell::capture
            args:
              cmd: "taskit fmt --check"
          - step: clippy
            handler: shell::capture
            args:
              cmd: "taskit lint"

  test:
    depends: [lint]
    steps:
      - step: compile_tests
        handler: shell::capture
        args:
          cmd: "taskit compile-tests"
      - step: nextest
        handler: shell::capture
        args:
          cmd: "taskit test --offline"

  verify:
    depends: [test]
    steps:
      - step: check_deps
        handler: shell::exec
        args:
          cmd: "taskit check-deps"
      - step: drift
        handler: shell::exec
        args:
          cmd: "taskit check-protocol-drift"

  ci:
    depends: [gate, lint, test, verify]
```

## `taskit.toml` linkage

```toml
[ci]
cruxfile = "ci.crux"
```

When `cruxfile` is set, `[[ci.steps]]` is ignored.

## Interactive Mode

`taskit init --interactive` uses `dialoguer` per section:

1. **Crates**: show discovered list, confirm or edit
2. **Propagation**: show derived graph, confirm
3. **Protocol surfaces**: show discovered files, toggle each
4. **Coverage**: suggest first lib crate, confirm crate + threshold
5. **CI pipeline**: show default steps, allow reorder/remove
6. **Cruxfile**: confirm generation

Sections skipped by the user are omitted from output files.

## Conformance Tests

```rust
// Shared conformance suite in taskit-core or a test helper
fn assert_pipeline_runner_contract(runner: &dyn PipelineRunner, cruxfile: &Path) {
    // 1. Valid pipeline returns PipelineOutcome with passed=true
    // 2. Failing step produces PipelineOutcome with passed=false
    // 3. Missing config_path returns Err, not panic
    // 4. Empty pipeline returns passed=true with zero results
}
```

Each adapter tested through the same function.

## Out of Scope

- Async conversion of `main()` -- `block_on` at embedded runner boundary
- Plugin system for custom handlers
- Windows CI gate
- Partial merge of discovered vs explicit config
- Migration from `[[ci.steps]]` to Cruxfile (manual)

## Risk

- [x] Breaking API changes: no -- `CiConfig.cruxfile` is additive with
      `#[serde(default)]`
- [x] New external deps: `dialoguer` (taskit-init), `crux-script` + `tokio`
      (taskit-crux, optional)
- [x] Feature flag required: `crux` on bin crate for `taskit-crux`
- [x] Workspace restructure changes all internal imports
- [x] `crux-script` is a local path dep until crux is published
