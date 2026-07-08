use std::time::Duration;
use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};

/// Build a `PipelineOutcome` with a single step.
pub fn single_step_outcome(
    name: &str,
    passed: bool,
    duration: Duration,
    error: Option<String>,
) -> PipelineOutcome {
    let status = if passed {
        StepStatus::Pass
    } else {
        StepStatus::Fail
    };
    PipelineOutcome {
        results: vec![StepResult {
            name: name.to_string(),
            status,
            duration,
            error,
            gate: false,
            diagnostics: vec![],
            context: Default::default(),
        }],
        total: duration,
        passed,
        context: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passing_outcome() {
        let o = single_step_outcome("fmt", true, Duration::from_secs(1), None);
        assert!(o.passed);
        assert_eq!(o.results.len(), 1);
        assert_eq!(o.results[0].status, StepStatus::Pass);
        assert_eq!(o.results[0].name, "fmt");
        assert!(o.results[0].error.is_none());
    }

    #[test]
    fn failing_outcome() {
        let o = single_step_outcome("test", false, Duration::from_secs(2), Some("bad".into()));
        assert!(!o.passed);
        assert_eq!(o.results[0].status, StepStatus::Fail);
        assert_eq!(o.results[0].error.as_deref(), Some("bad"));
    }

    #[test]
    fn duration_propagated() {
        let d = Duration::from_millis(500);
        let o = single_step_outcome("x", true, d, None);
        assert_eq!(o.total, d);
        assert_eq!(o.results[0].duration, d);
    }
}
