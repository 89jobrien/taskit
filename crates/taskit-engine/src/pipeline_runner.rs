use std::path::{Path, PathBuf};
use std::time::Instant;

use taskit_core::pipeline_runner::PipelineRunner;
use taskit_types::error::TaskitError;
use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};

use crate::ctx::Ctx;

/// Adapter: runs the built-in pipeline using taskit's native step engine.
///
/// Reads pipeline configuration (`[ci]`, `[coverage]`, `[protocol]`) from the
/// injected [`Ctx`]. `offline` is a per-run flag (not part of the config).
pub struct BuiltinRunner<'a> {
    pub(crate) ctx: &'a Ctx,
    pub(crate) offline: bool,
}

impl<'a> BuiltinRunner<'a> {
    pub fn new(ctx: &'a Ctx, offline: bool) -> Self {
        Self { ctx, offline }
    }
}

impl PipelineRunner for BuiltinRunner<'_> {
    fn run_pipeline(
        &self,
        _config_path: &Path,
        fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError> {
        let outcome = match self.ctx.ci() {
            Some(cfg) if !cfg.steps.is_empty() => {
                crate::ci::run_from_config_internal(self.ctx, cfg, fail_fast, self.offline)
            }
            Some(_) => {
                // Explicit [ci] with empty steps = run nothing
                crate::step::Pipeline::new(fail_fast).run()
            }
            None => crate::ci::run_default_internal(self.ctx, fail_fast, self.offline),
        };
        Ok(outcome)
    }
}

/// Adapter: runs a Cruxfile via subprocess (`crux run <path>`).
pub struct SubprocessCruxRunner {
    cruxfile_path: PathBuf,
}

impl SubprocessCruxRunner {
    pub fn new(cruxfile_path: PathBuf) -> Self {
        Self { cruxfile_path }
    }
}

impl PipelineRunner for SubprocessCruxRunner {
    fn run_pipeline(
        &self,
        _config_path: &Path,
        _fail_fast: bool,
    ) -> Result<PipelineOutcome, TaskitError> {
        if !self.cruxfile_path.exists() {
            return Err(TaskitError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("cruxfile not found: {}", self.cruxfile_path.display()),
            )));
        }

        let start = Instant::now();
        let output = std::process::Command::new("crux")
            .arg("run")
            .arg(&self.cruxfile_path)
            .output()
            .map_err(|e| {
                TaskitError::Io(std::io::Error::other(format!("failed to run crux: {e}")))
            })?;

        let duration = start.elapsed();
        let passed = output.status.success();
        let error = if passed {
            None
        } else {
            Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
        };

        Ok(PipelineOutcome {
            results: vec![StepResult {
                name: "crux-pipeline".into(),
                status: if passed {
                    StepStatus::Pass
                } else {
                    StepStatus::Fail
                },
                duration,
                error,
                gate: false,
                diagnostics: vec![],
            }],
            total: duration,
            passed,
        })
    }
}

// ── Conformance ─────────────────────────────────────────────────────────────

/// Invariant 1: nonexistent config path must return Err.
#[cfg(test)]
pub(crate) fn assert_nonexistent_path_returns_err(runner: &dyn PipelineRunner) {
    let result = runner.run_pipeline(Path::new("/nonexistent/taskit.toml"), false);
    assert!(
        result.is_err(),
        "run_pipeline with nonexistent path must return Err"
    );
}

/// Invariant 2: a successful outcome must have `passed == true`, non-empty
/// `results`, and every step status must be `Pass`.
#[cfg(test)]
pub(crate) fn assert_success_outcome_invariants(outcome: &PipelineOutcome) {
    assert!(
        outcome.passed,
        "successful outcome must have passed == true"
    );
    assert!(
        !outcome.results.is_empty(),
        "successful outcome must contain at least one StepResult"
    );
    for r in &outcome.results {
        assert_eq!(
            r.status,
            StepStatus::Pass,
            "all steps in a successful outcome must have status Pass (got {:?} for '{}')",
            r.status,
            r.name,
        );
    }
}

/// Invariant 3: a failed outcome must have `passed == false` and at least one
/// step with status `Fail`.
#[cfg(test)]
pub(crate) fn assert_failure_outcome_invariants(outcome: &PipelineOutcome) {
    assert!(!outcome.passed, "failed outcome must have passed == false");
    let has_fail = outcome.results.iter().any(|r| r.status == StepStatus::Fail);
    assert!(
        has_fail,
        "failed outcome must contain at least one step with status Fail"
    );
}

/// Invariant 4: `total` duration must be >= the sum of individual step
/// durations (tolerates minor timer skew with a small epsilon).
#[cfg(test)]
pub(crate) fn assert_duration_invariants(outcome: &PipelineOutcome) {
    use std::time::Duration;
    let step_sum: Duration = outcome.results.iter().map(|r| r.duration).sum();
    // Allow up to 1 ms of measurement noise
    let epsilon = Duration::from_millis(1);
    assert!(
        outcome.total + epsilon >= step_sum,
        "total duration ({:?}) must be >= sum of step durations ({:?})",
        outcome.total,
        step_sum,
    );
}

/// Invariant 5: every StepResult name must be non-empty.
#[cfg(test)]
pub(crate) fn assert_step_names_nonempty(outcome: &PipelineOutcome) {
    for (i, r) in outcome.results.iter().enumerate() {
        assert!(
            !r.name.is_empty(),
            "StepResult[{i}] has an empty name — all step names must be non-empty"
        );
    }
}

/// Assert the full trait contract for a runner whose nonexistent path returns Err.
/// Callers that can also produce a valid outcome should call the individual
/// `assert_*` helpers directly.
#[cfg(test)]
pub(crate) fn assert_pipeline_runner_contract(runner: &dyn PipelineRunner) {
    assert_nonexistent_path_returns_err(runner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_types::config::{CiConfig, CiStep, Config};
    use taskit_types::output_format::OutputFormat;
    use xshell::Shell;

    /// Build a test context whose `[ci]` section contains a single self-check
    /// step (fast) to exercise the config-driven path without running full CI.
    fn ctx_with_ci(ci: CiConfig) -> Ctx {
        let config = Config {
            ci: Some(ci),
            ..Config::default()
        };
        Ctx::new(
            Shell::new().unwrap(),
            PathBuf::from("."),
            config,
            false,
            OutputFormat::Human,
        )
    }

    fn self_check_ci() -> CiConfig {
        CiConfig {
            steps: vec![CiStep {
                name: "self-check".into(),
                cmd: "self-check".into(),
                gate: false,
            }],
            cruxfile: None,
        }
    }

    // ── BuiltinRunner ───────────────────────────────────────────────────

    #[test]
    fn builtin_runner_implements_trait() {
        let ctx = ctx_with_ci(self_check_ci());
        let runner = BuiltinRunner::new(&ctx, false);
        let outcome = runner
            .run_pipeline(Path::new("taskit.toml"), false)
            .unwrap();
        assert_eq!(outcome.results.len(), 1);
    }

    #[test]
    fn builtin_runner_with_config_steps_returns_outcome() {
        let ctx = ctx_with_ci(self_check_ci());
        let runner = BuiltinRunner::new(&ctx, false);
        // Runs the config-driven path (not the full default pipeline)
        let outcome = runner
            .run_pipeline(Path::new("taskit.toml"), false)
            .unwrap();
        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].name, "self-check");
    }

    // ── SubprocessCruxRunner ────────────────────────────────────────────

    #[test]
    fn subprocess_runner_missing_cruxfile_returns_err() {
        let runner = SubprocessCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(Path::new("/nonexistent/ci.crux"), false);
        assert!(result.is_err());
    }

    #[test]
    fn subprocess_runner_implements_trait() {
        let runner = SubprocessCruxRunner::new(PathBuf::from("/nonexistent/ci.crux"));
        let result = runner.run_pipeline(Path::new("taskit.toml"), false);
        assert!(result.is_err(), "missing cruxfile should return Err");
    }

    // ── Conformance ─────────────────────────────────────────────────────

    #[test]
    fn subprocess_runner_conformance() {
        let runner = SubprocessCruxRunner::new(PathBuf::from("/nonexistent"));
        assert_pipeline_runner_contract(&runner);
    }

    /// Verify all outcome invariants against a hand-constructed passing outcome.
    #[test]
    fn conformance_success_outcome_invariants() {
        use std::time::Duration;
        let step = StepResult {
            name: "fmt".into(),
            status: StepStatus::Pass,
            duration: Duration::from_millis(10),
            error: None,
            gate: false,
            diagnostics: vec![],
        };
        let outcome = PipelineOutcome {
            results: vec![step],
            total: Duration::from_millis(10),
            passed: true,
        };
        assert_success_outcome_invariants(&outcome);
        assert_duration_invariants(&outcome);
        assert_step_names_nonempty(&outcome);
    }

    /// Verify all outcome invariants against a hand-constructed failing outcome.
    #[test]
    fn conformance_failure_outcome_invariants() {
        use std::time::Duration;
        let step = StepResult {
            name: "lint".into(),
            status: StepStatus::Fail,
            duration: Duration::from_millis(5),
            error: Some("clippy found warnings".into()),
            gate: false,
            diagnostics: vec![],
        };
        let outcome = PipelineOutcome {
            results: vec![step],
            total: Duration::from_millis(5),
            passed: false,
        };
        assert_failure_outcome_invariants(&outcome);
        assert_duration_invariants(&outcome);
        assert_step_names_nonempty(&outcome);
    }

    /// Empty step name must trigger the invariant check.
    #[test]
    #[should_panic(expected = "empty name")]
    fn conformance_empty_step_name_panics() {
        use std::time::Duration;
        let step = StepResult {
            name: String::new(),
            status: StepStatus::Pass,
            duration: Duration::ZERO,
            error: None,
            gate: false,
            diagnostics: vec![],
        };
        let outcome = PipelineOutcome {
            results: vec![step],
            total: Duration::ZERO,
            passed: true,
        };
        assert_step_names_nonempty(&outcome);
    }

    /// `passed == true` with a Fail step must trigger the invariant check.
    #[test]
    #[should_panic(expected = "status Pass")]
    fn conformance_passed_true_with_fail_step_panics() {
        use std::time::Duration;
        let step = StepResult {
            name: "lint".into(),
            status: StepStatus::Fail,
            duration: Duration::ZERO,
            error: None,
            gate: false,
            diagnostics: vec![],
        };
        let outcome = PipelineOutcome {
            results: vec![step],
            total: Duration::ZERO,
            passed: true,
        };
        assert_success_outcome_invariants(&outcome);
    }

    /// `passed == false` with no Fail step must trigger the invariant check.
    #[test]
    #[should_panic(expected = "at least one step with status Fail")]
    fn conformance_passed_false_without_fail_step_panics() {
        use std::time::Duration;
        let step = StepResult {
            name: "fmt".into(),
            status: StepStatus::Pass,
            duration: Duration::ZERO,
            error: None,
            gate: false,
            diagnostics: vec![],
        };
        let outcome = PipelineOutcome {
            results: vec![step],
            total: Duration::ZERO,
            passed: false,
        };
        assert_failure_outcome_invariants(&outcome);
    }
}
