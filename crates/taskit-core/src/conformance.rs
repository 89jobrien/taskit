//! Conformance helpers for [`PipelineRunner`] implementations.
//!
//! These are public when the `test-support` feature is enabled, allowing
//! downstream crates to verify their adapter satisfies the trait contract.

use std::path::Path;
use std::time::Duration;

use taskit_types::step::{PipelineOutcome, StepStatus};

use crate::pipeline_runner::PipelineRunner;

/// Invariant 1: nonexistent config path must return Err.
pub fn assert_nonexistent_path_returns_err(runner: &dyn PipelineRunner) {
    let result = runner.run_pipeline(Path::new("/nonexistent/taskit.toml"), false);
    assert!(
        result.is_err(),
        "run_pipeline with nonexistent path must return Err"
    );
}

/// Invariant 2: a successful outcome must have `passed == true`, non-empty
/// `results`, and every step status must be `Pass`.
pub fn assert_success_outcome_invariants(outcome: &PipelineOutcome) {
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
pub fn assert_failure_outcome_invariants(outcome: &PipelineOutcome) {
    assert!(!outcome.passed, "failed outcome must have passed == false");
    let has_fail = outcome.results.iter().any(|r| r.status == StepStatus::Fail);
    assert!(
        has_fail,
        "failed outcome must contain at least one step with status Fail"
    );
}

/// Invariant 4: `total` duration must be >= the sum of individual step
/// durations (tolerates minor timer skew with a small epsilon).
pub fn assert_duration_invariants(outcome: &PipelineOutcome) {
    let step_sum: Duration = outcome.results.iter().map(|r| r.duration).sum();
    let epsilon = Duration::from_millis(1);
    assert!(
        outcome.total + epsilon >= step_sum,
        "total duration ({:?}) must be >= sum of step durations ({:?})",
        outcome.total,
        step_sum,
    );
}

/// Invariant 5: every StepResult name must be non-empty.
pub fn assert_step_names_nonempty(outcome: &PipelineOutcome) {
    for (i, r) in outcome.results.iter().enumerate() {
        assert!(
            !r.name.is_empty(),
            "StepResult[{i}] has an empty name -- all step names must be non-empty"
        );
    }
}

/// Assert the full trait contract for a runner whose nonexistent path returns Err.
pub fn assert_pipeline_runner_contract(runner: &dyn PipelineRunner) {
    assert_nonexistent_path_returns_err(runner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step_builder::StepBuilder;

    #[test]
    fn conformance_success_outcome_invariants() {
        let outcome = PipelineOutcome {
            results: vec![StepBuilder::new("fmt").duration_ms(10).build()],
            total: Duration::from_millis(10),
            passed: true,
            context: None,
        };
        assert_success_outcome_invariants(&outcome);
        assert_duration_invariants(&outcome);
        assert_step_names_nonempty(&outcome);
    }

    #[test]
    fn conformance_failure_outcome_invariants() {
        let outcome = PipelineOutcome {
            results: vec![
                StepBuilder::new("lint")
                    .fail()
                    .duration_ms(5)
                    .error("clippy found warnings")
                    .build(),
            ],
            total: Duration::from_millis(5),
            passed: false,
            context: None,
        };
        assert_failure_outcome_invariants(&outcome);
        assert_duration_invariants(&outcome);
        assert_step_names_nonempty(&outcome);
    }

    #[test]
    #[should_panic(expected = "empty name")]
    fn conformance_empty_step_name_panics() {
        let outcome = PipelineOutcome {
            results: vec![StepBuilder::new("").build()],
            total: Duration::ZERO,
            passed: true,
            context: None,
        };
        assert_step_names_nonempty(&outcome);
    }

    #[test]
    #[should_panic(expected = "status Pass")]
    fn conformance_passed_true_with_fail_step_panics() {
        let outcome = PipelineOutcome {
            results: vec![StepBuilder::new("lint").fail().build()],
            total: Duration::ZERO,
            passed: true,
            context: None,
        };
        assert_success_outcome_invariants(&outcome);
    }

    #[test]
    #[should_panic(expected = "at least one step with status Fail")]
    fn conformance_passed_false_without_fail_step_panics() {
        let outcome = PipelineOutcome {
            results: vec![StepBuilder::new("fmt").build()],
            total: Duration::ZERO,
            passed: false,
            context: None,
        };
        assert_failure_outcome_invariants(&outcome);
    }
}
