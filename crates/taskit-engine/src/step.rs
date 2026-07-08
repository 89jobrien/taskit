use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::progress::fmt_elapsed;
use taskit_types::error::TaskitError;
use taskit_types::step::{DiagnosticRecord, PipelineRunContext, StepDiagnosticContext};

// Re-export core types for convenience.
pub use taskit_types::step::{PipelineOutcome, StepResult, StepStatus};

/// Shared buffer for collecting diagnostics from within a step closure.
///
/// Pass a clone into your step closure and push diagnostics into it.
/// After the step runs, the pipeline extracts the diagnostics and attaches
/// them to the `StepResult`.
pub type DiagnosticSink = Rc<RefCell<Vec<DiagnosticRecord>>>;
/// Shared buffer for collecting diagnostic context from a step wrapper.
pub type StepContextSink = Rc<RefCell<StepDiagnosticContext>>;

struct PipelineStep<'a> {
    name: String,
    is_gate: bool,
    f: Box<dyn FnOnce() -> Result<(), TaskitError> + 'a>,
    diagnostics: Option<DiagnosticSink>,
    context: Option<StepContextSink>,
}

pub struct Pipeline<'a> {
    steps: Vec<PipelineStep<'a>>,
    fail_fast: bool,
    context: Option<PipelineRunContext>,
}

impl<'a> Pipeline<'a> {
    pub fn new(fail_fast: bool) -> Self {
        Self {
            steps: Vec::new(),
            fail_fast,
            context: None,
        }
    }

    pub fn with_context(mut self, context: PipelineRunContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Normal step. Skipped if a gate above failed, or if fail_fast and any prior step failed.
    pub fn step(mut self, name: &str, f: impl FnOnce() -> Result<(), TaskitError> + 'a) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: false,
            f: Box::new(f),
            diagnostics: None,
            context: None,
        });
        self
    }

    pub fn step_with_context_sink(
        mut self,
        name: &str,
        sink: StepContextSink,
        f: impl FnOnce() -> Result<(), TaskitError> + 'a,
    ) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: false,
            f: Box::new(f),
            diagnostics: None,
            context: Some(sink),
        });
        self
    }

    pub fn gate_with_context_sink(
        mut self,
        name: &str,
        sink: StepContextSink,
        f: impl FnOnce() -> Result<(), TaskitError> + 'a,
    ) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: true,
            f: Box::new(f),
            diagnostics: None,
            context: Some(sink),
        });
        self
    }

    pub(crate) fn step_with_diagnostics_and_context_sink(
        mut self,
        name: &str,
        diagnostics: DiagnosticSink,
        context: StepContextSink,
        f: impl FnOnce() -> Result<(), TaskitError> + 'a,
    ) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: false,
            f: Box::new(f),
            diagnostics: Some(diagnostics),
            context: Some(context),
        });
        self
    }

    /// Hard gate. If this fails, all subsequent steps are skipped regardless of fail_fast.
    pub fn gate(mut self, name: &str, f: impl FnOnce() -> Result<(), TaskitError> + 'a) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: true,
            f: Box::new(f),
            diagnostics: None,
            context: None,
        });
        self
    }

    /// Step with a diagnostic sink for capturing per-finding data (SARIF output).
    ///
    /// The closure should push `DiagnosticRecord`s into the sink. After the step
    /// runs, the pipeline drains the sink into `StepResult.diagnostics`.
    pub fn step_with_diagnostics(
        mut self,
        name: &str,
        sink: DiagnosticSink,
        f: impl FnOnce() -> Result<(), TaskitError> + 'a,
    ) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: false,
            f: Box::new(f),
            diagnostics: Some(sink),
            context: None,
        });
        self
    }

    /// Execute the pipeline. Returns structured outcome.
    pub fn run(self) -> PipelineOutcome {
        let mut results: Vec<StepResult> = Vec::new();
        let mut gate_failed = false;
        let mut any_failed = false;
        let pipeline_start = Instant::now();

        for ps in self.steps {
            let should_skip = gate_failed || (self.fail_fast && any_failed);
            if should_skip {
                let step_context = drain_step_context(ps.context);
                taskit_output::taskit_skip!("{} (skipped)", ps.name);
                results.push(StepResult {
                    name: ps.name,
                    status: StepStatus::Skipped,
                    duration: Duration::ZERO,
                    error: None,
                    gate: ps.is_gate,
                    diagnostics: vec![],
                    context: step_context,
                });
                continue;
            }

            let start = Instant::now();
            let outcome = (ps.f)();
            let duration = start.elapsed();
            let elapsed = fmt_elapsed(duration);
            let (status, error) = match &outcome {
                Ok(_) => {
                    taskit_output::taskit_ok!("✓ {} [{elapsed}]", ps.name);
                    (StepStatus::Pass, None)
                }
                Err(e) => {
                    let msg = e.to_string();
                    taskit_output::taskit_err!("✗ {} [{elapsed}]: {msg}", ps.name);
                    any_failed = true;
                    if ps.is_gate {
                        gate_failed = true;
                    }
                    (StepStatus::Fail, Some(msg))
                }
            };
            let step_diagnostics = ps
                .diagnostics
                .map(|sink| sink.borrow_mut().drain(..).collect())
                .unwrap_or_default();
            let step_context = drain_step_context(ps.context);
            results.push(StepResult {
                name: ps.name,
                status,
                duration,
                error,
                gate: ps.is_gate,
                diagnostics: step_diagnostics,
                context: step_context,
            });
        }

        PipelineOutcome {
            total: pipeline_start.elapsed(),
            passed: !any_failed,
            results,
            context: self.context,
        }
    }
}

fn drain_step_context(context: Option<StepContextSink>) -> StepDiagnosticContext {
    context
        .map(|sink| std::mem::take(&mut *sink.borrow_mut()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_status_display() {
        assert_eq!(format!("{}", StepStatus::Pass), "PASS");
        assert_eq!(format!("{}", StepStatus::Fail), "FAIL");
        assert_eq!(format!("{}", StepStatus::Skipped), "SKIP");
    }

    #[test]
    fn pipeline_all_pass() {
        let outcome = Pipeline::new(false)
            .step("a", || Ok(()))
            .step("b", || Ok(()))
            .run();
        assert!(outcome.passed);
        assert_eq!(outcome.results.len(), 2);
    }

    #[test]
    fn pipeline_fail_fast_skips_remaining() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_c = Rc::new(Cell::new(false));
        let ran_c2 = ran_c.clone();
        let outcome = Pipeline::new(true)
            .step("a", || Ok(()))
            .step("b", || Err(TaskitError::other("b failed")))
            .step("c", move || {
                ran_c2.set(true);
                Ok(())
            })
            .run();
        assert!(!outcome.passed);
        assert!(!ran_c.get(), "c should have been skipped");
    }

    #[test]
    fn pipeline_gate_skips_all_on_failure() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_b = Rc::new(Cell::new(false));
        let ran_b2 = ran_b.clone();
        let outcome = Pipeline::new(false)
            .gate("preflight", || Err(TaskitError::other("tools missing")))
            .step("b", move || {
                ran_b2.set(true);
                Ok(())
            })
            .run();
        assert!(!outcome.passed);
        assert!(!ran_b.get(), "b should be skipped after gate failure");
    }

    #[test]
    fn pipeline_gate_pass_continues() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_b = Rc::new(Cell::new(false));
        let ran_b2 = ran_b.clone();
        let outcome = Pipeline::new(false)
            .gate("preflight", || Ok(()))
            .step("b", move || {
                ran_b2.set(true);
                Ok(())
            })
            .run();
        assert!(outcome.passed);
        assert!(ran_b.get());
    }

    #[test]
    fn pipeline_fail_fast_false_runs_all_steps() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_c = Rc::new(Cell::new(false));
        let ran_c2 = ran_c.clone();
        let outcome = Pipeline::new(false)
            .step("a", || Ok(()))
            .step("b", || Err(TaskitError::other("b failed")))
            .step("c", move || {
                ran_c2.set(true);
                Ok(())
            })
            .run();
        assert!(!outcome.passed);
        assert!(ran_c.get(), "c should have run when fail_fast=false");
    }

    #[test]
    fn pipeline_with_no_steps_passes() {
        assert!(Pipeline::new(false).run().passed);
        assert!(Pipeline::new(true).run().passed);
    }

    #[test]
    fn pipeline_non_gate_failure_does_not_block_non_fail_fast() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_b = Rc::new(Cell::new(false));
        let ran_b2 = ran_b.clone();
        let outcome = Pipeline::new(false)
            .step("a", || Err(TaskitError::other("a failed")))
            .step("b", move || {
                ran_b2.set(true);
                Ok(())
            })
            .run();
        assert!(!outcome.passed);
        assert!(ran_b.get());
    }

    #[test]
    fn pipeline_multiple_failures_all_recorded_fail_fast_false() {
        let outcome = Pipeline::new(false)
            .step("a", || Err(TaskitError::other("a")))
            .step("b", || Err(TaskitError::other("b")))
            .step("c", || Ok(()))
            .run();
        assert!(!outcome.passed);
        assert_eq!(outcome.results.len(), 3);
    }

    #[test]
    fn pipeline_run_returns_outcome_with_error_and_gate() {
        let outcome = Pipeline::new(false)
            .gate("g", || Err(TaskitError::other("gate failed")))
            .step("s", || Ok(()))
            .run();
        assert!(!outcome.passed);
        assert!(outcome.results[0].gate);
        assert!(outcome.results[0].error.is_some());
        assert_eq!(outcome.results[1].status, StepStatus::Skipped);
    }

    #[test]
    fn step_status_skipped_display() {
        assert_eq!(format!("{}", StepStatus::Skipped), "SKIP");
    }

    #[test]
    fn pipeline_attaches_run_context() {
        let context = PipelineRunContext {
            taskit_version: "0.7.0".into(),
            workspace_root: ".".into(),
            ..PipelineRunContext::default()
        };
        let outcome = Pipeline::new(false)
            .with_context(context.clone())
            .step("a", || Ok(()))
            .run();

        assert_eq!(outcome.context, Some(context));
    }

    #[test]
    fn pipeline_attaches_step_context_from_sink() {
        let sink = Rc::new(RefCell::new(StepDiagnosticContext {
            reproduction: Some("taskit lint".into()),
            ..StepDiagnosticContext::default()
        }));
        let outcome = Pipeline::new(false)
            .step_with_context_sink("lint", sink, || Ok(()))
            .run();

        assert_eq!(
            outcome.results[0].context.reproduction.as_deref(),
            Some("taskit lint")
        );
    }
}
