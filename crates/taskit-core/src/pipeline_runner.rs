use taskit_types::error::TaskitError;
use taskit_types::step::PipelineOutcome;

/// Port: executes a CI pipeline and returns its outcome.
///
/// Each adapter carries its own configuration (workspace refs, cruxfile
/// path) via its constructor, so the port only takes what every adapter
/// must honor: the `fail_fast` policy.
///
/// Adapters: `BuiltinRunner` (taskit-engine), `SubprocessCruxRunner`
/// (taskit-engine), `EmbeddedCruxRunner` (taskit-crux).
pub trait PipelineRunner {
    fn run_pipeline(&self, fail_fast: bool) -> Result<PipelineOutcome, TaskitError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use taskit_types::step::{StepResult, StepStatus};

    struct FakeRunner {
        passed: bool,
    }

    impl PipelineRunner for FakeRunner {
        fn run_pipeline(&self, _fail_fast: bool) -> Result<PipelineOutcome, TaskitError> {
            Ok(PipelineOutcome {
                results: vec![StepResult {
                    name: "fake".into(),
                    status: if self.passed {
                        StepStatus::Pass
                    } else {
                        StepStatus::Fail
                    },
                    duration: Duration::from_millis(10),
                    error: None,
                    gate: false,
                    diagnostics: vec![],
                }],
                total: Duration::from_millis(10),
                passed: self.passed,
            })
        }
    }

    #[test]
    fn fake_runner_satisfies_trait() {
        let runner = FakeRunner { passed: true };
        let outcome = runner.run_pipeline(false).unwrap();
        assert!(outcome.passed);
        assert_eq!(outcome.results.len(), 1);
    }

    #[test]
    fn fake_runner_failure() {
        let runner = FakeRunner { passed: false };
        let outcome = runner.run_pipeline(true).unwrap();
        assert!(!outcome.passed);
    }
}
