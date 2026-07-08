use taskit_types::error::TaskitError;
use xshell::Shell;

use crate::ctx::Ctx;
use crate::health::{self, HealthBaseline};
use crate::step::{Pipeline, PipelineOutcome};

#[derive(Debug, Clone, Default)]
pub struct Thresholds {
    pub max_clippy_warnings: usize,
    pub max_clippy_errors: usize,
    pub max_test_failures: usize,
    pub max_todo_fixme: Option<usize>,
}

/// Build and run the inspect pipeline, returning a structured outcome.
pub fn run_pipeline(sh: &Shell, thresholds: &Thresholds) -> Result<PipelineOutcome, TaskitError> {
    let baseline = health::collect(sh)?;
    Ok(build_pipeline(&baseline, thresholds))
}

/// Build a pipeline from the collected baseline and thresholds.
///
/// Each threshold check becomes a pipeline step so the output formatters
/// can render it consistently with `taskit ci`.
fn build_pipeline(baseline: &HealthBaseline, thresholds: &Thresholds) -> PipelineOutcome {
    let max_failures = thresholds.max_test_failures;
    let failed = baseline.tests.failed;

    let max_errors = thresholds.max_clippy_errors;
    let errors = baseline.clippy.errors;

    let max_warnings = thresholds.max_clippy_warnings;
    let warnings = baseline.clippy.warnings;

    let consistent = baseline.versions_consistent;

    let mut pipeline = Pipeline::new(false)
        .step(&format!("test failures <= {max_failures}"), move || {
            threshold_check("test failures", failed, max_failures)
        })
        .step(&format!("clippy errors <= {max_errors}"), move || {
            threshold_check("clippy errors", errors, max_errors)
        })
        .step(&format!("clippy warnings <= {max_warnings}"), move || {
            threshold_check("clippy warnings", warnings, max_warnings)
        })
        .step("version consistency", move || {
            if consistent {
                Ok(())
            } else {
                Err(TaskitError::other("workspace versions are inconsistent"))
            }
        });

    if let Some(max_todo) = thresholds.max_todo_fixme {
        let todo_count = baseline.todo_fixme;
        pipeline = pipeline.step(&format!("TODO/FIXME <= {max_todo}"), move || {
            threshold_check("TODO/FIXME", todo_count, max_todo)
        });
    }

    pipeline.run()
}

fn threshold_check(name: &str, value: usize, limit: usize) -> Result<(), TaskitError> {
    if value <= limit {
        Ok(())
    } else {
        Err(TaskitError::other(format!(
            "{name}: {value} exceeds limit {limit}"
        )))
    }
}

pub fn run(ctx: &Ctx, max_warnings: usize, max_todo: Option<usize>) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let thresholds = Thresholds {
        max_clippy_warnings: max_warnings,
        max_todo_fixme: max_todo,
        ..Default::default()
    };
    let outcome = run_pipeline(sh, &thresholds)?;
    Ok(taskit_output::write_output(ctx.output, &outcome)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{ClippyCounts, HealthBaseline, TestCounts};
    use crate::step::StepStatus;
    use taskit_types::output_format::OutputFormat;

    fn baseline(
        failed: usize,
        warnings: usize,
        errors: usize,
        todo: usize,
        consistent: bool,
    ) -> HealthBaseline {
        HealthBaseline {
            date: "2026-01-01".into(),
            tests: TestCounts {
                total: 100,
                passed: 100 - failed,
                failed,
                skipped: 0,
            },
            clippy: ClippyCounts { warnings, errors },
            todo_fixme: todo,
            crates: 4,
            versions_consistent: consistent,
            version: "0.4.0".into(),
        }
    }

    #[test]
    fn all_clean_passes() {
        let b = baseline(0, 0, 0, 3, true);
        let outcome = build_pipeline(&b, &Thresholds::default());
        assert!(outcome.passed);
        assert_eq!(outcome.results.len(), 4);
    }

    #[test]
    fn test_failures_fail() {
        let b = baseline(1, 0, 0, 0, true);
        let outcome = build_pipeline(&b, &Thresholds::default());
        assert!(!outcome.passed);
        let step = &outcome.results[0];
        assert_eq!(step.status, StepStatus::Fail);
    }

    #[test]
    fn clippy_errors_fail() {
        let b = baseline(0, 0, 1, 0, true);
        let outcome = build_pipeline(&b, &Thresholds::default());
        assert!(!outcome.passed);
        let step = &outcome.results[1];
        assert_eq!(step.status, StepStatus::Fail);
    }

    #[test]
    fn clippy_warnings_within_threshold_pass() {
        let b = baseline(0, 3, 0, 0, true);
        let t = Thresholds {
            max_clippy_warnings: 5,
            ..Default::default()
        };
        let outcome = build_pipeline(&b, &t);
        assert!(outcome.passed);
    }

    #[test]
    fn clippy_warnings_over_threshold_fail() {
        let b = baseline(0, 6, 0, 0, true);
        let t = Thresholds {
            max_clippy_warnings: 5,
            ..Default::default()
        };
        let outcome = build_pipeline(&b, &t);
        assert!(!outcome.passed);
        let step = &outcome.results[2];
        assert_eq!(step.status, StepStatus::Fail);
    }

    #[test]
    fn version_inconsistency_fails() {
        let b = baseline(0, 0, 0, 0, false);
        let outcome = build_pipeline(&b, &Thresholds::default());
        assert!(!outcome.passed);
        let step = &outcome.results[3];
        assert_eq!(step.status, StepStatus::Fail);
    }

    #[test]
    fn todo_not_checked_without_threshold() {
        let b = baseline(0, 0, 0, 100, true);
        let outcome = build_pipeline(&b, &Thresholds::default());
        assert!(outcome.passed);
        assert_eq!(outcome.results.len(), 4);
    }

    #[test]
    fn todo_checked_when_threshold_set() {
        let b = baseline(0, 0, 0, 10, true);
        let t = Thresholds {
            max_todo_fixme: Some(5),
            ..Default::default()
        };
        let outcome = build_pipeline(&b, &t);
        assert!(!outcome.passed);
        assert_eq!(outcome.results.len(), 5);
        let step = &outcome.results[4];
        assert_eq!(step.status, StepStatus::Fail);
    }

    #[test]
    fn todo_within_threshold_passes() {
        let b = baseline(0, 0, 0, 3, true);
        let t = Thresholds {
            max_todo_fixme: Some(5),
            ..Default::default()
        };
        let outcome = build_pipeline(&b, &t);
        assert!(outcome.passed);
    }

    #[test]
    fn outcome_renders_with_all_formatters() {
        let b = baseline(1, 0, 0, 0, true);
        let outcome = build_pipeline(&b, &Thresholds::default());
        for fmt in [
            OutputFormat::Human,
            OutputFormat::Json,
            OutputFormat::Github,
            OutputFormat::Junit,
            OutputFormat::Diagnostic,
        ] {
            let formatter = taskit_output::formatter_for(fmt);
            let rendered = formatter.render(&outcome);
            assert!(!rendered.is_empty(), "empty output for {fmt:?}");
        }
    }
}
