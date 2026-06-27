# Plan: `taskit init` + PipelineRunner Port (v0.4)

## Goal

Restructure taskit into a 5-crate workspace, add `PipelineRunner` port with
three adapters, and add `taskit init` for config + Cruxfile generation.

## Architecture

- Crates affected: `taskit-core` (new), `taskit-engine` (new),
  `taskit-init` (new), `taskit-crux` (new), `taskit` (bin, rewritten)
- New traits: `PipelineRunner` in `taskit-core`
- New types: `InitPlan`, `CoveragePlan`, `CiStepPlan` in `taskit-init`;
  `BuiltinRunner`, `SubprocessCruxRunner` in `taskit-engine`;
  `EmbeddedCruxRunner` in `taskit-crux`
- Data flow: discovery -> InitPlan -> render_toml + render_cruxfile;
  config::load -> PipelineRunner dispatch -> PipelineOutcome -> write_output

## Tech Stack

- Rust edition 2024
- New deps: `dialoguer` (taskit-init), `crux-script` + `tokio`
  (taskit-crux, optional)
- Existing deps redistributed across crates by responsibility

## Tasks

### Phase 1: Workspace Restructure

### Task 1: Create workspace Cargo.toml and taskit-core crate

**Crate**: `taskit-core`
**File(s)**: `Cargo.toml` (workspace root), `crates/taskit-core/Cargo.toml`,
`crates/taskit-core/src/lib.rs`
**Run**: `cargo check -p taskit-core`

1. Write failing test:

   ```rust
   // crates/taskit-core/src/lib.rs
   #[cfg(test)]
   mod tests {
       #[test]
       fn core_crate_compiles() {
           assert!(true);
       }
   }
   ```

   Run: `cargo test -p taskit-core`
   Expected: FAIL (crate doesn't exist yet)

2. Implement:

   Convert root `Cargo.toml` to workspace:

   ```toml
   [workspace]
   resolver = "2"
   members = ["crates/*"]

   [workspace.package]
   edition = "2024"
   license = "MIT OR Apache-2.0"
   repository = "https://github.com/89jobrien/taskit"
   ```

   Create `crates/taskit-core/Cargo.toml`:

   ```toml
   [package]
   name = "taskit-core"
   version = "0.4.0"
   edition.workspace = true
   license.workspace = true
   description = "Core types and traits for taskit"

   [dependencies]
   anyhow = "1"
   clap = { version = "4", features = ["derive"] }
   serde = { version = "1", features = ["derive"] }
   ```

   Create `crates/taskit-core/src/lib.rs`:

   ```rust
   pub mod config;
   pub mod output_format;
   pub mod pipeline_runner;
   pub mod step;
   ```

3. Verify:

   ```
   cargo check -p taskit-core
   cargo clippy -p taskit-core -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "refactor(taskit): create workspace + taskit-core crate skeleton"`

### Task 2: Extract core types into taskit-core

**Crate**: `taskit-core`
**File(s)**: `crates/taskit-core/src/config.rs`,
`crates/taskit-core/src/step.rs`,
`crates/taskit-core/src/output_format.rs`,
`crates/taskit-core/src/pipeline_runner.rs`
**Run**: `cargo test -p taskit-core`

1. Write failing test:

   ```rust
   // crates/taskit-core/src/step.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use std::time::Duration;

       #[test]
       fn pipeline_outcome_default_is_passed() {
           let outcome = PipelineOutcome {
               results: vec![],
               total: Duration::ZERO,
               passed: true,
           };
           assert!(outcome.passed);
       }

       #[test]
       fn step_status_display() {
           assert_eq!(format!("{}", StepStatus::Pass), "PASS");
           assert_eq!(format!("{}", StepStatus::Fail), "FAIL");
           assert_eq!(format!("{}", StepStatus::Skipped), "SKIP");
       }
   }
   ```

   Run: `cargo test -p taskit-core`
   Expected: FAIL (types don't exist yet)

2. Implement:

   Move `StepResult`, `StepStatus`, `PipelineOutcome` and their Display
   impls from `src/step.rs` into `crates/taskit-core/src/step.rs`.

   Move config types (`Config`, `WorkspaceConfig`, `CrateEntry`,
   `PropagationEntry`, `ProtocolConfig`, `SurfaceEntry`, `CiConfig`,
   `CiStep`, `CoverageConfig`) from `src/config.rs` into
   `crates/taskit-core/src/config.rs`. Add `cruxfile` field to `CiConfig`:

   ```rust
   #[derive(Debug, Default, Deserialize)]
   pub struct CiConfig {
       #[serde(default)]
       pub steps: Vec<CiStep>,
       #[serde(default)]
       pub cruxfile: Option<String>,
   }
   ```

   Move `OutputFormat` enum into `crates/taskit-core/src/output_format.rs`.

   Create `crates/taskit-core/src/pipeline_runner.rs`:

   ```rust
   use std::path::Path;
   use anyhow::Result;
   use crate::step::PipelineOutcome;

   /// Port: executes a CI pipeline and returns structured results.
   pub trait PipelineRunner {
       fn run_pipeline(
           &self,
           config_path: &Path,
           fail_fast: bool,
       ) -> Result<PipelineOutcome>;
   }
   ```

3. Verify:

   ```
   cargo test -p taskit-core
   cargo clippy -p taskit-core -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "refactor(taskit-core): extract core types, config, PipelineRunner trait"`

### Task 3: Create taskit-engine crate

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/Cargo.toml`,
`crates/taskit-engine/src/lib.rs`
**Run**: `cargo check -p taskit-engine`

1. Write failing test:

   ```rust
   // crates/taskit-engine/src/lib.rs
   #[cfg(test)]
   mod tests {
       #[test]
       fn engine_crate_compiles() {
           assert!(true);
       }
   }
   ```

   Run: `cargo test -p taskit-engine`
   Expected: FAIL (crate doesn't exist)

2. Implement:

   Create `crates/taskit-engine/Cargo.toml`:

   ```toml
   [package]
   name = "taskit-engine"
   version = "0.4.0"
   edition.workspace = true
   license.workspace = true
   description = "CI pipeline engine for taskit"

   [dependencies]
   taskit-core = { path = "../taskit-core" }
   anyhow = "1"
   cargo_metadata = "0.19"
   clap = { version = "4", features = ["derive"] }
   hex = "0.4"
   indicatif = "0.17"
   miette = { version = "7", features = ["fancy"] }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   sha2 = "0.10"
   toml = "0.8"
   xshell = "0.2"

   [dev-dependencies]
   tempfile = "3"
   ```

   Move all remaining source modules from `src/` into
   `crates/taskit-engine/src/`. Update all `crate::` imports to reference
   `taskit_core::` for types that moved. Re-export key engine functions
   from `crates/taskit-engine/src/lib.rs`.

3. Verify:

   ```
   cargo check -p taskit-engine
   cargo test -p taskit-engine
   cargo clippy -p taskit-engine -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "refactor(taskit-engine): move engine modules, update imports"`

### Task 4: Create thin bin crate

**Crate**: `taskit` (bin)
**File(s)**: `crates/taskit/Cargo.toml`, `crates/taskit/src/main.rs`
**Run**: `cargo build -p taskit`

1. Write failing test (manual): run `cargo build -p taskit` -- should fail
   because crate doesn't exist.

2. Implement:

   Create `crates/taskit/Cargo.toml`:

   ```toml
   [package]
   name = "taskit"
   version = "0.4.0"
   edition.workspace = true
   license.workspace = true
   description = "Config-driven cargo xtask runner"
   publish = true

   [[bin]]
   name = "taskit"
   path = "src/main.rs"

   [dependencies]
   taskit-core = { path = "../taskit-core" }
   taskit-engine = { path = "../taskit-engine" }
   anyhow = "1"
   clap = { version = "4", features = ["derive"] }
   xshell = "0.2"

   [features]
   crux = ["dep:taskit-crux"]

   [dependencies.taskit-crux]
   path = "../taskit-crux"
   optional = true
   ```

   Create `crates/taskit/src/main.rs`: rewrite as thin CLI dispatch that
   imports types from `taskit_core` and functions from `taskit_engine`.
   Mirror the current `src/main.rs` structure but with updated imports.

3. Verify:

   ```
   cargo build -p taskit
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "refactor(taskit): thin bin crate with workspace dispatch"`

### Task 5: Remove old flat src/ and verify workspace

**Crate**: workspace root
**File(s)**: `src/` (remove), old root `Cargo.toml` lib/bin sections
**Run**: `cargo test --workspace`

1. Remove `src/` directory (all code now lives in `crates/`).
   Remove `[lib]` and `[[bin]]` sections from root `Cargo.toml`.
   Delete old root `Cargo.lock` if workspace generates a new one.

2. Verify:

   ```
   cargo test --workspace           -> all tests pass
   cargo clippy --workspace -- -D warnings -> clean
   cargo build -p taskit            -> binary works
   taskit --help                    -> outputs help text
   ```

3. Run: `git branch --show-current`
   Commit: `git commit -m "refactor(taskit): remove old flat src/, workspace restructure complete"`

---

### Phase 2: PipelineRunner Adapters

### Task 6: BuiltinRunner adapter

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/pipeline_runner.rs`
**Run**: `cargo test -p taskit-engine -- builtin_runner`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use taskit_core::pipeline_runner::PipelineRunner;

       #[test]
       fn builtin_runner_implements_trait() {
           let sh = xshell::Shell::new().unwrap();
           let ws = taskit_core::config::WorkspaceConfig::default();
           let runner = BuiltinRunner {
               sh: &sh,
               ws: &ws,
               proto: None,
               cov: None,
               ci: None,
               offline: false,
           };
           // Type-check: runner implements PipelineRunner
           let _: &dyn PipelineRunner = &runner;
       }
   }
   ```

   Run: `cargo test -p taskit-engine -- builtin_runner`
   Expected: FAIL (struct doesn't exist)

2. Implement `BuiltinRunner` struct and `impl PipelineRunner`:
   wraps existing `run_default` / `run_from_config` logic from `ci.rs`.

3. Verify:

   ```
   cargo test -p taskit-engine
   cargo clippy -p taskit-engine -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-engine): BuiltinRunner adapter for PipelineRunner"`

### Task 7: SubprocessCruxRunner adapter

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/pipeline_runner.rs`
**Run**: `cargo test -p taskit-engine -- subprocess_crux`

1. Write failing test:

   ```rust
   #[test]
   fn subprocess_runner_missing_cruxfile_returns_err() {
       let runner = SubprocessCruxRunner {
           cruxfile_path: PathBuf::from("/nonexistent/ci.crux"),
       };
       let result = runner.run_pipeline(
           Path::new("/nonexistent/ci.crux"),
           false,
       );
       assert!(result.is_err());
   }
   ```

   Run: `cargo test -p taskit-engine -- subprocess_crux`
   Expected: FAIL

2. Implement `SubprocessCruxRunner`:
   - Shells out to `crux run <cruxfile_path>`
   - Parses exit code: 0 = passed, non-zero = failed
   - Attempts to parse stdout as JSON for step details
   - Falls back to single-step outcome on parse failure

3. Verify:

   ```
   cargo test -p taskit-engine
   cargo clippy -p taskit-engine -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-engine): SubprocessCruxRunner adapter"`

### Task 8: Wire PipelineRunner dispatch in ci::run

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/ci.rs`
**Run**: `cargo test -p taskit-engine -- ci`

1. Write failing test:

   ```rust
   #[test]
   fn ci_run_with_cruxfile_delegates_to_runner() {
       let cfg = CiConfig {
           steps: vec![],
           cruxfile: Some("ci.crux".into()),
       };
       // Verify cruxfile field is read and dispatch path is taken
       assert!(cfg.cruxfile.is_some());
   }
   ```

   Run: `cargo test -p taskit-engine -- ci`
   Expected: FAIL (CiConfig doesn't have cruxfile yet in engine)

2. Implement: Update `ci::run()` to check `ci.cruxfile`:
   - If set: construct `SubprocessCruxRunner` (or accept a
     `&dyn PipelineRunner` from the bin crate for embedded)
   - If not set: use `BuiltinRunner` (existing behavior)

3. Verify:

   ```
   cargo test -p taskit-engine
   cargo clippy -p taskit-engine -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-engine): dispatch to PipelineRunner based on ci.cruxfile"`

### Task 9: Conformance test suite

**Crate**: `taskit-engine`
**File(s)**: `crates/taskit-engine/src/pipeline_runner.rs`
**Run**: `cargo test -p taskit-engine -- conformance`

1. Write conformance tests:

   ```rust
   fn assert_pipeline_runner_contract(runner: &dyn PipelineRunner) {
       // Missing config returns Err
       let result = runner.run_pipeline(
           Path::new("/nonexistent"),
           false,
       );
       assert!(result.is_err());
   }

   #[test]
   fn builtin_runner_conformance() {
       let sh = Shell::new().unwrap();
       let ws = WorkspaceConfig::default();
       let runner = BuiltinRunner {
           sh: &sh, ws: &ws, proto: None,
           cov: None, ci: None, offline: false,
       };
       assert_pipeline_runner_contract(&runner);
   }

   #[test]
   fn subprocess_runner_conformance() {
       let runner = SubprocessCruxRunner {
           cruxfile_path: PathBuf::from("/nonexistent"),
       };
       assert_pipeline_runner_contract(&runner);
   }
   ```

2. Verify all adapters pass the contract.

3. Run: `git branch --show-current`
   Commit: `git commit -m "test(taskit-engine): PipelineRunner conformance test suite"`

---

### Phase 3: taskit-init

### Task 10: Create taskit-init crate skeleton

**Crate**: `taskit-init`
**File(s)**: `crates/taskit-init/Cargo.toml`, `crates/taskit-init/src/lib.rs`
**Run**: `cargo check -p taskit-init`

1. Create `crates/taskit-init/Cargo.toml`:

   ```toml
   [package]
   name = "taskit-init"
   version = "0.4.0"
   edition.workspace = true
   license.workspace = true
   description = "Config and Cruxfile generator for taskit"

   [dependencies]
   taskit-core = { path = "../taskit-core" }
   anyhow = "1"
   dialoguer = "0.11"
   ```

2. Create `crates/taskit-init/src/lib.rs`:

   ```rust
   pub mod interactive;
   pub mod plan;
   pub mod render_cruxfile;
   pub mod render_toml;

   pub fn run(force: bool, interactive: bool) -> anyhow::Result<()> {
       todo!()
   }
   ```

3. Verify: `cargo check -p taskit-init`

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-init): crate skeleton"`

### Task 11: InitPlan and plan_from_discovery

**Crate**: `taskit-init`
**File(s)**: `crates/taskit-init/src/plan.rs`
**Run**: `cargo test -p taskit-init -- plan`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn default_ci_steps_match_builtin_pipeline() {
           let plan = InitPlan::default_steps();
           let names: Vec<&str> = plan.iter().map(|s| s.name.as_str()).collect();
           assert!(names.contains(&"self-check"));
           assert!(names.contains(&"fmt --check"));
           assert!(names.contains(&"lint"));
           assert!(names.contains(&"test"));
           assert!(names.contains(&"check-deps"));
       }

       #[test]
       fn default_ci_steps_first_is_gate() {
           let plan = InitPlan::default_steps();
           assert!(plan[0].gate);
       }
   }
   ```

   Expected: FAIL

2. Implement `InitPlan` struct, `CoveragePlan`, `CiStepPlan`,
   `InitPlan::default_steps()`, and `plan_from_discovery()`.

3. Verify:

   ```
   cargo test -p taskit-init
   cargo clippy -p taskit-init -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-init): InitPlan and plan_from_discovery"`

### Task 12: render_toml

**Crate**: `taskit-init`
**File(s)**: `crates/taskit-init/src/render_toml.rs`
**Run**: `cargo test -p taskit-init -- render_toml`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn render_toml_contains_workspace_section() {
           let plan = InitPlan {
               crates: vec![],
               propagation: vec![],
               surfaces: vec![],
               coverage: None,
               ci_steps: vec![],
               offline_skip: None,
           };
           let output = render_toml(&plan);
           assert!(output.contains("[workspace]"));
       }

       #[test]
       fn render_toml_contains_ci_cruxfile() {
           let plan = InitPlan {
               crates: vec![],
               propagation: vec![],
               surfaces: vec![],
               coverage: None,
               ci_steps: vec![],
               offline_skip: None,
           };
           let output = render_toml(&plan);
           assert!(output.contains("cruxfile"));
       }

       #[test]
       fn render_toml_coverage_commented_out_when_none() {
           let plan = InitPlan {
               crates: vec![],
               propagation: vec![],
               surfaces: vec![],
               coverage: None,
               ci_steps: vec![],
               offline_skip: None,
           };
           let output = render_toml(&plan);
           assert!(output.contains("# [coverage]"));
       }
   }
   ```

   Expected: FAIL

2. Implement `render_toml()` -- hand-built string with controlled section
   ordering, inline comments, commented-out optional sections.

3. Verify:

   ```
   cargo test -p taskit-init
   cargo clippy -p taskit-init -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-init): render_toml with hand-controlled formatting"`

### Task 13: render_cruxfile

**Crate**: `taskit-init`
**File(s)**: `crates/taskit-init/src/render_cruxfile.rs`
**Run**: `cargo test -p taskit-init -- render_cruxfile`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn render_cruxfile_has_project_and_targets() {
           let plan = InitPlan {
               crates: vec![],
               propagation: vec![],
               surfaces: vec![],
               coverage: None,
               ci_steps: InitPlan::default_steps(),
               offline_skip: None,
           };
           let output = render_cruxfile(&plan, "my-project");
           assert!(output.contains("project: my-project"));
           assert!(output.contains("targets:"));
           assert!(output.contains("ci:"));
       }

       #[test]
       fn render_cruxfile_uses_shell_handlers() {
           let plan = InitPlan {
               crates: vec![],
               propagation: vec![],
               surfaces: vec![],
               coverage: None,
               ci_steps: InitPlan::default_steps(),
               offline_skip: None,
           };
           let output = render_cruxfile(&plan, "test");
           assert!(output.contains("handler: shell::"));
       }
   }
   ```

   Expected: FAIL

2. Implement `render_cruxfile()` -- generates YAML matching the Cruxfile
   format from the design doc.

3. Verify:

   ```
   cargo test -p taskit-init
   cargo clippy -p taskit-init -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-init): render_cruxfile generates Cruxfile YAML"`

### Task 14: Interactive mode with dialoguer

**Crate**: `taskit-init`
**File(s)**: `crates/taskit-init/src/interactive.rs`
**Run**: `cargo test -p taskit-init -- interactive`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn refine_plan_signature_exists() {
           // Compile-time check that the function signature is correct
           let _: fn(&mut InitPlan) -> anyhow::Result<()> = refine_plan;
       }
   }
   ```

   Expected: FAIL

2. Implement `refine_plan()` using `dialoguer::Confirm` and
   `dialoguer::Select` for each section. Each section is independently
   skippable.

3. Verify:

   ```
   cargo test -p taskit-init
   cargo clippy -p taskit-init -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-init): interactive mode with dialoguer"`

### Task 15: Wire init::run and Init CLI subcommand

**Crate**: `taskit-init`, `taskit` (bin)
**File(s)**: `crates/taskit-init/src/lib.rs`, `crates/taskit/src/main.rs`
**Run**: `cargo build -p taskit`

1. Write failing test:

   ```rust
   // crates/taskit-init/src/lib.rs
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn run_refuses_overwrite_without_force() {
           let dir = tempfile::tempdir().unwrap();
           let toml_path = dir.path().join("taskit.toml");
           std::fs::write(&toml_path, "existing").unwrap();
           std::env::set_current_dir(dir.path()).unwrap();
           let result = run(false, false);
           assert!(result.is_err());
           let msg = result.unwrap_err().to_string();
           assert!(msg.contains("already exists"));
       }
   }
   ```

   Expected: FAIL

2. Implement `init::run()`:
   - Check for existing `taskit.toml`, refuse without `--force`
   - Call `plan_from_discovery()` or build default plan
   - If interactive, call `refine_plan()`
   - Call `render_toml()` + `render_cruxfile()`
   - Write files

   Add `Init` subcommand to bin crate:

   ```rust
   /// Generate taskit.toml and ci.crux from workspace discovery
   Init {
       /// Overwrite existing taskit.toml
       #[arg(long)]
       force: bool,
       /// Interactive mode: prompt per section
       #[arg(long, short)]
       interactive: bool,
   },
   ```

3. Verify:

   ```
   cargo build -p taskit
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit): add Init subcommand with --force and --interactive"`

---

### Phase 4: taskit-crux

### Task 16: Create taskit-crux crate with EmbeddedCruxRunner

**Crate**: `taskit-crux`
**File(s)**: `crates/taskit-crux/Cargo.toml`, `crates/taskit-crux/src/lib.rs`
**Run**: `cargo test -p taskit-crux`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use taskit_core::pipeline_runner::PipelineRunner;
       use std::path::Path;

       #[test]
       fn embedded_runner_implements_trait() {
           let runner = EmbeddedCruxRunner {
               cruxfile_path: PathBuf::from("ci.crux"),
           };
           let _: &dyn PipelineRunner = &runner;
       }

       #[test]
       fn embedded_runner_missing_file_returns_err() {
           let runner = EmbeddedCruxRunner {
               cruxfile_path: PathBuf::from("/nonexistent/ci.crux"),
           };
           let result = runner.run_pipeline(
               Path::new("/nonexistent/ci.crux"),
               false,
           );
           assert!(result.is_err());
       }
   }
   ```

   Expected: FAIL

2. Implement:

   Create `crates/taskit-crux/Cargo.toml`:

   ```toml
   [package]
   name = "taskit-crux"
   version = "0.4.0"
   edition.workspace = true
   license.workspace = true
   description = "Embedded crux-script pipeline runner for taskit"

   [dependencies]
   taskit-core = { path = "../taskit-core" }
   anyhow = "1"
   crux-script = { path = "/Users/joe/dev/crux/crates/crux-script" }
   tokio = { version = "1", features = ["rt", "macros"] }
   serde_json = "1"
   ```

   Implement `EmbeddedCruxRunner`:
   - `run_pipeline()` creates a `tokio::runtime::Runtime`
   - Calls `block_on` to run `crux_script::Runner::run_target()`
   - Converts `Crux<Value>` trace steps into `StepResult` vec
   - Maps success/failure into `PipelineOutcome`

3. Verify:

   ```
   cargo test -p taskit-crux
   cargo clippy -p taskit-crux -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit-crux): EmbeddedCruxRunner with crux-script + tokio"`

### Task 17: Wire crux feature flag in bin crate

**Crate**: `taskit` (bin)
**File(s)**: `crates/taskit/src/main.rs`, `crates/taskit/Cargo.toml`
**Run**: `cargo build -p taskit --features crux`

1. Write failing test (manual): `cargo build -p taskit --features crux`
   should fail because the feature wiring doesn't exist yet.

2. Implement:

   In bin `Cargo.toml`, the `[features]` section already has
   `crux = ["dep:taskit-crux"]`. Update `main.rs` CI dispatch:

   ```rust
   // When cruxfile is set and crux feature is enabled
   #[cfg(feature = "crux")]
   if let Some(cruxfile) = ci_config.and_then(|c| c.cruxfile.as_ref()) {
       let runner = taskit_crux::EmbeddedCruxRunner {
           cruxfile_path: PathBuf::from(cruxfile),
       };
       let outcome = runner.run_pipeline(
           Path::new(cruxfile), fail_fast
       )?;
       return taskit_engine::output::write_output(output, &outcome)
           .map_err(|e| anyhow::anyhow!("{e}"));
   }
   ```

3. Verify:

   ```
   cargo build -p taskit                    -> works without crux
   cargo build -p taskit --features crux    -> works with crux
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   ```

4. Run: `git branch --show-current`
   Commit: `git commit -m "feat(taskit): wire crux feature flag for embedded Cruxfile execution"`

### Task 18: Version bump and final verification

**Crate**: all
**File(s)**: all `Cargo.toml` files
**Run**: `cargo test --workspace`

1. Bump all crate versions to `0.4.0`.
2. Run full verification:

   ```
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   cargo build -p taskit
   taskit --help
   taskit ci    (from taskit repo -- should pass)
   ```

3. Run: `git branch --show-current`
   Commit: `git commit -m "chore(release): taskit workspace v0.4.0"`
