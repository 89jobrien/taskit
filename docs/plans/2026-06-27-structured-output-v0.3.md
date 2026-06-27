# Plan: Structured Output (v0.3.0)

## Goal

Pipeline runs emit structured results in JSON, GitHub Actions, or JUnit XML
via `--output <format>`, making taskit CI-integration-ready and giving lib
consumers programmatic access to pipeline outcomes.

## Architecture

- Crate affected: `taskit` (single crate)
- New types: `PipelineOutcome`, `OutputFormat`, `OutputFormatter` trait,
  `HumanFormatter`, `JsonFormatter`, `GithubFormatter`, `JunitFormatter`
  in `src/output.rs`
- Changed: `Pipeline::run()` returns `PipelineOutcome` instead of
  `Result<()>`; `StepResult` gains `error` and `gate` fields
- Data flow: `Pipeline::run()` -> `PipelineOutcome` -> `OutputFormatter`
  -> stderr/stdout/file

## Tech Stack

- Rust edition 2024
- New dependency: `quick-xml = "0.37"` (JUnit XML generation)
- Existing: `serde`, `serde_json`, `clap`

## Tasks

### Task 1: Add `quick-xml` dependency

**Crate**: `taskit`
**File(s)**: `Cargo.toml`

1. Add to `[dependencies]`:

   ```toml
   quick-xml = { version = "0.37", features = ["serialize"] }
   ```

2. Verify:

   ```
   cargo check    -> compiles
   ```

3. Commit: `chore(taskit): add quick-xml dependency for JUnit output`

---

### Task 2: Enhance `StepResult` with `error` and `gate` fields

**Crate**: `taskit`
**File(s)**: `src/step.rs`
**Run**: `cargo test -p taskit -- step`

1. Write failing test:

   ```rust
   #[test]
   fn step_result_has_error_and_gate_fields() {
       let r = StepResult {
           name: "test".into(),
           status: StepStatus::Fail,
           duration: Duration::from_secs(1),
           error: Some("boom".into()),
           gate: true,
       };
       assert_eq!(r.error.as_deref(), Some("boom"));
       assert!(r.gate);
   }
   ```

   Expected: FAIL (fields don't exist)

2. Add fields to `StepResult` (src/step.rs line 28):

   ```rust
   #[derive(Debug)]
   pub struct StepResult {
       pub name: String,
       pub status: StepStatus,
       pub duration: Duration,
       pub error: Option<String>,
       pub gate: bool,
   }
   ```

3. Update `Pipeline::run()` to populate the new fields. In the step
   execution loop, change the `results.push(...)` calls:
   - For skipped steps: `error: None, gate: ps.is_gate`
   - For passed steps: `error: None, gate: ps.is_gate`
   - For failed steps: `error: Some(e.to_string()), gate: ps.is_gate`

4. Verify:

   ```
   cargo test -p taskit -- step   -> all pass
   cargo clippy -p taskit -- -D warnings -> zero warnings
   ```

5. Commit: `feat(taskit): add error and gate fields to StepResult`

---

### Task 3: Add `PipelineOutcome` and change `Pipeline::run()` return type

**Crate**: `taskit`
**File(s)**: `src/step.rs`
**Run**: `cargo test -p taskit -- step`

1. Write failing test:

   ```rust
   #[test]
   fn pipeline_run_returns_outcome() {
       let outcome = Pipeline::new(false)
           .step("a", || Ok(()))
           .step("b", || anyhow::bail!("fail"))
           .run();
       assert!(!outcome.passed);
       assert_eq!(outcome.results.len(), 2);
       assert_eq!(outcome.results[0].status, StepStatus::Pass);
       assert_eq!(outcome.results[1].status, StepStatus::Fail);
       assert!(outcome.results[1].error.is_some());
       assert!(outcome.total > Duration::ZERO || outcome.total == Duration::ZERO);
   }
   ```

   Expected: FAIL (`run()` returns `Result<()>`, not `PipelineOutcome`)

2. Add `PipelineOutcome`:

   ```rust
   #[derive(Debug)]
   pub struct PipelineOutcome {
       pub results: Vec<StepResult>,
       pub total: Duration,
       pub passed: bool,
   }
   ```

3. Change `Pipeline::run()` signature from `-> anyhow::Result<()>` to
   `-> PipelineOutcome`. Remove the `print_summary` call and the
   `bail!("CI checks failed")` -- the caller decides what to do.

   ```rust
   pub fn run(self) -> PipelineOutcome {
       let mut results: Vec<StepResult> = Vec::new();
       let mut gate_failed = false;
       let mut any_failed = false;
       let pipeline_start = Instant::now();

       for ps in self.steps {
           let should_skip = gate_failed || (self.fail_fast && any_failed);
           if should_skip {
               eprintln!("  - {} (skipped)", ps.name);
               results.push(StepResult {
                   name: ps.name,
                   status: StepStatus::Skipped,
                   duration: Duration::ZERO,
                   error: None,
                   gate: ps.is_gate,
               });
               continue;
           }

           let sp = Spinner::new(&ps.name);
           let start = Instant::now();
           let outcome = (ps.f)();
           let duration = start.elapsed();
           let (status, error) = match &outcome {
               Ok(_) => {
                   sp.finish_ok();
                   (StepStatus::Pass, None)
               }
               Err(e) => {
                   sp.finish_err();
                   let msg = e.to_string();
                   eprintln!("  error: {msg}");
                   any_failed = true;
                   if ps.is_gate {
                       gate_failed = true;
                   }
                   (StepStatus::Fail, Some(msg))
               }
           };
           results.push(StepResult {
               name: ps.name,
               status,
               duration,
               error,
               gate: ps.is_gate,
           });
       }

       PipelineOutcome {
           total: pipeline_start.elapsed(),
           passed: !any_failed,
           results,
       }
   }
   ```

4. Update ALL existing tests in `step.rs` that call `.run()` -- they
   currently expect `Result<()>`, now they get `PipelineOutcome`:
   - `pipeline_all_pass`: `assert!(result.passed)` instead of
     `assert!(result.is_ok())`
   - `pipeline_fail_fast_skips_remaining`: `assert!(!result.passed)`
   - `pipeline_gate_skips_all_on_failure`: `assert!(!result.passed)`
   - `pipeline_gate_pass_continues`: `assert!(result.passed)`
   - `pipeline_fail_fast_false_runs_all_steps`: `assert!(!result.passed)`
   - `pipeline_with_no_steps_passes`: `assert!(Pipeline::new(false).run().passed)`
   - `pipeline_non_gate_failure_does_not_block_non_fail_fast`:
     `assert!(!result.passed)`
   - `pipeline_multiple_failures_all_recorded_fail_fast_false`:
     `assert!(!result.passed)`
   - `pipeline_error_message_indicates_ci_checks_failed`: remove this test
     (no error message anymore -- outcome has `.passed` bool)

5. Verify:

   ```
   cargo test -p taskit -- step   -> all pass
   ```

6. Commit: `feat(taskit): Pipeline::run returns PipelineOutcome`

---

### Task 4: Update `ci.rs` callers to handle `PipelineOutcome`

**Crate**: `taskit`
**File(s)**: `src/ci.rs`
**Run**: `cargo test -p taskit -- ci`

1. Update `run_from_config` and `run_default` to return `Result<()>` by
   checking `outcome.passed`:

   Change both functions' `pipeline.run()` call from:

   ```rust
   pipeline.run()
   ```

   to:

   ```rust
   let outcome = pipeline.run();
   crate::step::print_summary(&outcome.results);
   if outcome.passed {
       Ok(())
   } else {
       anyhow::bail!("CI checks failed")
   }
   ```

2. Verify:

   ```
   cargo test -p taskit -- ci     -> all pass
   cargo clippy -p taskit -- -D warnings -> zero warnings
   ```

3. Commit: `refactor(taskit): update ci.rs for PipelineOutcome`

---

### Task 5: Update `quick.rs` caller to handle `PipelineOutcome`

**Crate**: `taskit`
**File(s)**: `src/quick.rs`
**Run**: `cargo test -p taskit -- quick`

1. Update `quick::run` similarly:

   ```rust
   pub fn run(sh: &Shell, ws: &WorkspaceConfig) -> Result<()> {
       with_silent(|| {
           let outcome = Pipeline::new(false)
               .step("fmt --check (affected)", || fmt::run(sh, ws, true, true))
               .step("lint (affected)", || lint::run(sh, ws, None, true, false))
               .step("compile-tests", || testing::compile::run(sh))
               .step("test (affected, offline)", || {
                   testing::run::run(sh, ws, None, true, false, true)
               })
               .run();
           crate::step::print_summary(&outcome.results);
           if outcome.passed {
               Ok(())
           } else {
               anyhow::bail!("quick checks failed")
           }
       })
   }
   ```

2. Verify:

   ```
   cargo test -p taskit -- quick  -> passes
   ```

3. Commit: `refactor(taskit): update quick.rs for PipelineOutcome`

---

### Task 6: Create `OutputFormatter` trait and `HumanFormatter`

**Crate**: `taskit`
**File(s)**: `src/output.rs` (new), `src/lib.rs`
**Run**: `cargo test -p taskit -- output`

1. Write failing test:

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::step::{StepResult, StepStatus};
       use std::time::Duration;

       fn sample_outcome() -> PipelineOutcome {
           PipelineOutcome {
               results: vec![
                   StepResult {
                       name: "fmt".into(),
                       status: StepStatus::Pass,
                       duration: Duration::from_millis(1200),
                       error: None,
                       gate: true,
                   },
                   StepResult {
                       name: "test".into(),
                       status: StepStatus::Fail,
                       duration: Duration::from_millis(14700),
                       error: Some("3 tests failed".into()),
                       gate: false,
                   },
               ],
               total: Duration::from_millis(15900),
               passed: false,
           }
       }

       #[test]
       fn human_formatter_contains_step_names() {
           let output = HumanFormatter.render(&sample_outcome());
           assert!(output.contains("fmt"));
           assert!(output.contains("test"));
           assert!(output.contains("PASS"));
           assert!(output.contains("FAIL"));
       }
   }
   ```

   Expected: FAIL (module does not exist)

2. Create `src/output.rs`:

   ```rust
   use std::time::Duration;

   use crate::step::{PipelineOutcome, StepResult, StepStatus};

   /// Output format for pipeline results.
   #[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
   pub enum OutputFormat {
       #[default]
       Human,
       Json,
       Github,
       Junit,
   }

   /// Port: formats pipeline results for different output targets.
   pub trait OutputFormatter {
       fn render(&self, outcome: &PipelineOutcome) -> String;
   }

   impl OutputFormat {
       pub fn formatter(self) -> Box<dyn OutputFormatter> {
           match self {
               OutputFormat::Human => Box::new(HumanFormatter),
               OutputFormat::Json => Box::new(JsonFormatter),
               OutputFormat::Github => Box::new(GithubFormatter),
               OutputFormat::Junit => Box::new(JunitFormatter),
           }
       }
   }

   // ── Human ────────────────────────────────────────────────────────

   const COL_NAME: usize = 30;
   const COL_STATUS: usize = 10;
   const SEPARATOR_WIDTH: usize = 55;

   pub struct HumanFormatter;

   impl OutputFormatter for HumanFormatter {
       fn render(&self, outcome: &PipelineOutcome) -> String {
           let mut out = String::new();
           out.push('\n');
           out.push_str(&format!(
               "{:<COL_NAME$} {:<COL_STATUS$} Duration\n",
               "Step", "Status"
           ));
           out.push_str(&"-".repeat(SEPARATOR_WIDTH));
           out.push('\n');
           for s in &outcome.results {
               out.push_str(&format!(
                   "{:<COL_NAME$} {:<COL_STATUS$} {:.1}s\n",
                   s.name,
                   s.status,
                   s.duration.as_secs_f64()
               ));
           }
           out.push_str(&"-".repeat(SEPARATOR_WIDTH));
           out.push('\n');
           out.push_str(&format!(
               "{:<COL_NAME$} {:<COL_STATUS$} {:.1}s\n",
               "Total",
               "",
               outcome.total.as_secs_f64()
           ));
           out
       }
   }

   // Placeholder structs — implemented in subsequent tasks
   pub struct JsonFormatter;
   pub struct GithubFormatter;
   pub struct JunitFormatter;
   ```

3. Add `pub mod output;` to `src/lib.rs`.

4. Add stub `impl OutputFormatter` for `JsonFormatter`,
   `GithubFormatter`, `JunitFormatter` that return empty strings
   (just to compile). They will be filled in Tasks 7-9.

5. Verify:

   ```
   cargo test -p taskit -- output  -> passes
   ```

6. Commit: `feat(taskit): add OutputFormatter trait and HumanFormatter`

---

### Task 7: Implement `JsonFormatter`

**Crate**: `taskit`
**File(s)**: `src/output.rs`
**Run**: `cargo test -p taskit -- output::tests::json`

1. Write failing test:

   ```rust
   #[test]
   fn json_formatter_valid_json() {
       let output = JsonFormatter.render(&sample_outcome());
       let parsed: serde_json::Value =
           serde_json::from_str(&output).expect("valid JSON");
       assert_eq!(parsed["version"], 1);
       assert_eq!(parsed["passed"], false);
       assert_eq!(parsed["steps"].as_array().unwrap().len(), 2);
       assert_eq!(parsed["steps"][0]["name"], "fmt");
       assert_eq!(parsed["steps"][0]["status"], "pass");
       assert_eq!(parsed["steps"][1]["status"], "fail");
       assert_eq!(parsed["steps"][1]["error"], "3 tests failed");
       assert!(parsed["total_duration_secs"].as_f64().unwrap() > 0.0);
   }

   #[test]
   fn json_formatter_null_error_for_passing_step() {
       let output = JsonFormatter.render(&sample_outcome());
       let parsed: serde_json::Value =
           serde_json::from_str(&output).unwrap();
       assert!(parsed["steps"][0]["error"].is_null());
   }
   ```

2. Implement `JsonFormatter`:

   ```rust
   use serde::Serialize;

   #[derive(Serialize)]
   struct JsonOutput {
       version: u8,
       steps: Vec<JsonStep>,
       total_duration_secs: f64,
       passed: bool,
   }

   #[derive(Serialize)]
   struct JsonStep {
       name: String,
       status: String,
       duration_secs: f64,
       error: Option<String>,
       gate: bool,
   }

   impl OutputFormatter for JsonFormatter {
       fn render(&self, outcome: &PipelineOutcome) -> String {
           let output = JsonOutput {
               version: 1,
               steps: outcome
                   .results
                   .iter()
                   .map(|s| JsonStep {
                       name: s.name.clone(),
                       status: match s.status {
                           StepStatus::Pass => "pass".into(),
                           StepStatus::Fail => "fail".into(),
                           StepStatus::Skipped => "skip".into(),
                       },
                       duration_secs: s.duration.as_secs_f64(),
                       error: s.error.clone(),
                       gate: s.gate,
                   })
                   .collect(),
               total_duration_secs: outcome.total.as_secs_f64(),
               passed: outcome.passed,
           };
           serde_json::to_string_pretty(&output)
               .expect("JSON serialization cannot fail")
       }
   }
   ```

3. Verify:

   ```
   cargo test -p taskit -- output::tests::json  -> passes
   ```

4. Commit: `feat(taskit): implement JsonFormatter`

---

### Task 8: Implement `GithubFormatter`

**Crate**: `taskit`
**File(s)**: `src/output.rs`
**Run**: `cargo test -p taskit -- output::tests::github`

1. Write failing test:

   ```rust
   #[test]
   fn github_formatter_emits_annotations() {
       let output = GithubFormatter.render(&sample_outcome());
       assert!(output.contains("::notice title=fmt::"));
       assert!(output.contains("::error title=test::"));
   }

   #[test]
   fn github_formatter_includes_summary_table() {
       let output = GithubFormatter.render(&sample_outcome());
       assert!(output.contains("| Step "));
       assert!(output.contains("| fmt "));
   }
   ```

2. Implement:

   ```rust
   impl OutputFormatter for GithubFormatter {
       fn render(&self, outcome: &PipelineOutcome) -> String {
           let mut out = String::new();
           for s in &outcome.results {
               match s.status {
                   StepStatus::Pass => {
                       out.push_str(&format!(
                           "::notice title={}::Step \"{}\" passed ({:.1}s)\n",
                           s.name,
                           s.name,
                           s.duration.as_secs_f64()
                       ));
                   }
                   StepStatus::Fail => {
                       let msg = s.error.as_deref().unwrap_or("failed");
                       out.push_str(&format!(
                           "::error title={}::Step \"{}\" failed ({:.1}s): {}\n",
                           s.name,
                           s.name,
                           s.duration.as_secs_f64(),
                           msg
                       ));
                   }
                   StepStatus::Skipped => {
                       out.push_str(&format!(
                           "::notice title={}::Step \"{}\" skipped\n",
                           s.name, s.name
                       ));
                   }
               }
           }
           // Markdown summary table
           out.push_str("\n| Step | Status | Duration |\n");
           out.push_str("|---|---|---|\n");
           for s in &outcome.results {
               out.push_str(&format!(
                   "| {} | {} | {:.1}s |\n",
                   s.name,
                   s.status,
                   s.duration.as_secs_f64()
               ));
           }
           out
       }
   }
   ```

3. Verify:

   ```
   cargo test -p taskit -- output::tests::github  -> passes
   ```

4. Commit: `feat(taskit): implement GithubFormatter`

---

### Task 9: Implement `JunitFormatter`

**Crate**: `taskit`
**File(s)**: `src/output.rs`
**Run**: `cargo test -p taskit -- output::tests::junit`

1. Write failing test:

   ```rust
   #[test]
   fn junit_formatter_valid_xml() {
       let output = JunitFormatter.render(&sample_outcome());
       assert!(output.contains("<testsuites>"));
       assert!(output.contains("</testsuites>"));
       assert!(output.contains("<testsuite"));
       assert!(output.contains("name=\"taskit\""));
       assert!(output.contains("tests=\"2\""));
       assert!(output.contains("failures=\"1\""));
       assert!(output.contains("<testcase name=\"fmt\""));
       assert!(output.contains("<testcase name=\"test\""));
       assert!(output.contains("<failure"));
       assert!(output.contains("3 tests failed"));
   }

   #[test]
   fn junit_formatter_passing_pipeline_has_zero_failures() {
       let outcome = PipelineOutcome {
           results: vec![StepResult {
               name: "fmt".into(),
               status: StepStatus::Pass,
               duration: Duration::from_secs(1),
               error: None,
               gate: false,
           }],
           total: Duration::from_secs(1),
           passed: true,
       };
       let output = JunitFormatter.render(&outcome);
       assert!(output.contains("failures=\"0\""));
       assert!(!output.contains("<failure"));
   }
   ```

2. Implement (hand-written XML -- simpler than quick-xml serialization
   for this small structure):

   ```rust
   impl OutputFormatter for JunitFormatter {
       fn render(&self, outcome: &PipelineOutcome) -> String {
           let failures = outcome
               .results
               .iter()
               .filter(|s| s.status == StepStatus::Fail)
               .count();
           let tests = outcome.results.len();

           let mut out = String::new();
           out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
           out.push_str("<testsuites>\n");
           out.push_str(&format!(
               "  <testsuite name=\"taskit\" tests=\"{}\" \
                failures=\"{}\" time=\"{:.1}\">\n",
               tests,
               failures,
               outcome.total.as_secs_f64()
           ));
           for s in &outcome.results {
               match s.status {
                   StepStatus::Fail => {
                       let msg = s.error.as_deref().unwrap_or("failed");
                       let msg = xml_escape(msg);
                       out.push_str(&format!(
                           "    <testcase name=\"{}\" time=\"{:.1}\">\n",
                           xml_escape(&s.name),
                           s.duration.as_secs_f64()
                       ));
                       out.push_str(&format!(
                           "      <failure message=\"{msg}\"/>\n"
                       ));
                       out.push_str("    </testcase>\n");
                   }
                   StepStatus::Skipped => {
                       out.push_str(&format!(
                           "    <testcase name=\"{}\" time=\"0.0\">\n\
                            \x20     <skipped/>\n\
                            \x20   </testcase>\n",
                           xml_escape(&s.name)
                       ));
                   }
                   StepStatus::Pass => {
                       out.push_str(&format!(
                           "    <testcase name=\"{}\" time=\"{:.1}\"/>\n",
                           xml_escape(&s.name),
                           s.duration.as_secs_f64()
                       ));
                   }
               }
           }
           out.push_str("  </testsuite>\n");
           out.push_str("</testsuites>\n");
           out
       }
   }

   fn xml_escape(s: &str) -> String {
       s.replace('&', "&amp;")
           .replace('<', "&lt;")
           .replace('>', "&gt;")
           .replace('"', "&quot;")
   }
   ```

   Note: Since we're generating simple XML by hand, the `quick-xml`
   dependency is not needed. Remove it from `Cargo.toml` in this task.

3. Verify:

   ```
   cargo test -p taskit -- output::tests::junit  -> passes
   ```

4. Commit: `feat(taskit): implement JunitFormatter`

---

### Task 10: Wire `--output` flag into `main.rs`

**Crate**: `taskit`
**File(s)**: `src/main.rs`
**Run**: `cargo test -p taskit`

1. Add the global `--output` arg to `Cli`:

   ```rust
   use taskit::output::OutputFormat;

   #[derive(Parser)]
   #[command(name = "taskit", about = "Config-driven cargo xtask runner")]
   struct Cli {
       #[arg(long, global = true)]
       dry_run: bool,
       /// Output format: human (default), json, github, junit
       #[arg(long, global = true, default_value = "human")]
       output: OutputFormat,
       #[command(subcommand)]
       cmd: Cmd,
   }
   ```

2. For subcommands that use `Pipeline` (`Ci`, `Quick`), change the
   dispatch to pass `OutputFormat` through. For `Ci`:

   ```rust
   Cmd::Ci { fail_fast, include_network } => ci::run(
       &sh, ws, proto, config.ci.as_ref(), config.coverage.as_ref(),
       fail_fast, include_network, cli.output,
   ),
   ```

   Update `ci::run` signature to accept `OutputFormat` and use it
   instead of the hardcoded `print_summary` + bail pattern:

   ```rust
   pub fn run(
       sh: &Shell,
       ws: &WorkspaceConfig,
       proto: Option<&ProtocolConfig>,
       ci: Option<&CiConfig>,
       cov: Option<&CoverageConfig>,
       fail_fast: bool,
       include_network: bool,
       output_format: OutputFormat,
   ) -> Result<()> {
       let offline = !include_network;
       let outcome = match ci {
           Some(cfg) if !cfg.steps.is_empty() => {
               run_from_config(sh, ws, proto, cov, cfg, fail_fast, offline)
           }
           _ => run_default(sh, ws, proto, cov, fail_fast, offline),
       };
       let formatter = output_format.formatter();
       let rendered = formatter.render(&outcome);
       match output_format {
           OutputFormat::Json => print!("{rendered}"),
           OutputFormat::Junit => {
               let path = "target/taskit-results.xml";
               std::fs::write(path, &rendered)?;
               eprintln!("JUnit results written to {path}");
           }
           _ => eprint!("{rendered}"),
       }
       if let OutputFormat::Github = output_format {
           if let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") {
               use std::io::Write;
               let mut f = std::fs::OpenOptions::new()
                   .append(true)
                   .create(true)
                   .open(summary_path)?;
               // Write the markdown table portion (skip annotations)
               let table_start = rendered.find("\n| Step ");
               if let Some(idx) = table_start {
                   write!(f, "{}", &rendered[idx..])?;
               }
           }
       }
       if outcome.passed {
           Ok(())
       } else {
           anyhow::bail!("CI checks failed")
       }
   }
   ```

   Change `run_from_config` and `run_default` to return
   `PipelineOutcome` instead of `Result<()>`.

3. For `Quick`, keep it simple -- `quick::run` continues to return
   `Result<()>` and uses `HumanFormatter` internally (quick is local
   feedback, not CI).

4. Verify:

   ```
   cargo test -p taskit              -> all pass
   cargo clippy -p taskit -- -D warnings -> zero warnings
   ```

5. Commit: `feat(taskit): wire --output flag into CLI and ci pipeline`

---

### Task 11: Move `print_summary` from `step.rs` to `output.rs`

**Crate**: `taskit`
**File(s)**: `src/step.rs`, `src/output.rs`
**Run**: `cargo test -p taskit`

1. The old `print_summary` in `step.rs` is now superseded by
   `HumanFormatter::render`. Keep `print_summary` as a thin wrapper
   that calls `HumanFormatter` and prints to stderr, for callers that
   don't care about output format:

   ```rust
   pub fn print_summary(steps: &[StepResult]) {
       let outcome = PipelineOutcome {
           results: steps.to_vec(),
           total: steps.iter().map(|s| s.duration).sum(),
           passed: steps.iter().all(|s| s.status != StepStatus::Fail),
       };
       let rendered = crate::output::HumanFormatter.render(&outcome);
       eprint!("{rendered}");
   }
   ```

   This requires `StepResult` to derive `Clone`.

2. Verify:

   ```
   cargo test -p taskit  -> all pass
   ```

3. Commit: `refactor(taskit): delegate print_summary to HumanFormatter`

---

### Task 12: Version bump to 0.3.0

**Crate**: `taskit`
**File(s)**: `Cargo.toml`

1. Update version:

   ```toml
   version = "0.3.0"
   ```

2. Verify:

   ```
   cargo check                             -> compiles
   cargo test -p taskit                     -> all pass
   cargo clippy -p taskit -- -D warnings    -> zero warnings
   ```

3. Commit: `chore(release): taskit v0.3.0`
