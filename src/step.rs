use std::fmt;
use std::time::{Duration, Instant};

use crate::progress::Spinner;

const COL_NAME: usize = 30;
const COL_STATUS: usize = 10;
const SEPARATOR_WIDTH: usize = 55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pass,
    Fail,
    Skipped,
}

impl fmt::Display for StepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepStatus::Pass => write!(f, "PASS"),
            StepStatus::Fail => write!(f, "FAIL"),
            StepStatus::Skipped => write!(f, "SKIP"),
        }
    }
}

#[derive(Debug)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration: Duration,
}

struct PipelineStep<'a> {
    name: String,
    is_gate: bool,
    f: Box<dyn FnOnce() -> anyhow::Result<()> + 'a>,
}

pub struct Pipeline<'a> {
    steps: Vec<PipelineStep<'a>>,
    fail_fast: bool,
}

impl<'a> Pipeline<'a> {
    pub fn new(fail_fast: bool) -> Self {
        Self {
            steps: Vec::new(),
            fail_fast,
        }
    }

    /// Normal step. Skipped if a gate above failed, or if fail_fast and any prior step failed.
    pub fn step(mut self, name: &str, f: impl FnOnce() -> anyhow::Result<()> + 'a) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: false,
            f: Box::new(f),
        });
        self
    }

    /// Hard gate. If this fails, all subsequent steps are skipped regardless of fail_fast.
    pub fn gate(mut self, name: &str, f: impl FnOnce() -> anyhow::Result<()> + 'a) -> Self {
        self.steps.push(PipelineStep {
            name: name.to_string(),
            is_gate: true,
            f: Box::new(f),
        });
        self
    }

    /// Execute the pipeline. Prints summary table. Returns Err if any step failed.
    pub fn run(self) -> anyhow::Result<()> {
        let mut results: Vec<StepResult> = Vec::new();
        let mut gate_failed = false;
        let mut any_failed = false;

        for ps in self.steps {
            let should_skip = gate_failed || (self.fail_fast && any_failed);
            if should_skip {
                eprintln!("  - {} (skipped)", ps.name);
                results.push(StepResult {
                    name: ps.name,
                    status: StepStatus::Skipped,
                    duration: Duration::ZERO,
                });
                continue;
            }

            let sp = Spinner::new(&ps.name);
            let start = Instant::now();
            let outcome = (ps.f)();
            let duration = start.elapsed();
            let status = match &outcome {
                Ok(_) => {
                    sp.finish_ok();
                    StepStatus::Pass
                }
                Err(e) => {
                    sp.finish_err();
                    eprintln!("  error: {e}");
                    any_failed = true;
                    if ps.is_gate {
                        gate_failed = true;
                    }
                    StepStatus::Fail
                }
            };
            results.push(StepResult {
                name: ps.name,
                status,
                duration,
            });
        }

        print_summary(&results);

        if any_failed {
            anyhow::bail!("CI checks failed");
        }
        Ok(())
    }
}

pub fn print_summary(steps: &[StepResult]) {
    eprintln!();
    eprintln!("{:<COL_NAME$} {:<COL_STATUS$} Duration", "Step", "Status");
    eprintln!("{}", "-".repeat(SEPARATOR_WIDTH));
    let mut total = Duration::ZERO;
    for s in steps {
        eprintln!(
            "{:<COL_NAME$} {:<COL_STATUS$} {:.1}s",
            s.name,
            s.status,
            s.duration.as_secs_f64()
        );
        total += s.duration;
    }
    eprintln!("{}", "-".repeat(SEPARATOR_WIDTH));
    eprintln!(
        "{:<COL_NAME$} {:<COL_STATUS$} {:.1}s",
        "Total",
        "",
        total.as_secs_f64()
    );
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
        let result = Pipeline::new(false)
            .step("a", || Ok(()))
            .step("b", || Ok(()))
            .run();
        assert!(result.is_ok());
    }

    #[test]
    fn pipeline_fail_fast_skips_remaining() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_c = Rc::new(Cell::new(false));
        let ran_c2 = ran_c.clone();
        let result = Pipeline::new(true)
            .step("a", || Ok(()))
            .step("b", || anyhow::bail!("b failed"))
            .step("c", move || {
                ran_c2.set(true);
                Ok(())
            })
            .run();
        assert!(result.is_err());
        assert!(!ran_c.get(), "c should have been skipped");
    }

    #[test]
    fn pipeline_gate_skips_all_on_failure() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_b = Rc::new(Cell::new(false));
        let ran_b2 = ran_b.clone();
        let result = Pipeline::new(false)
            .gate("preflight", || anyhow::bail!("tools missing"))
            .step("b", move || {
                ran_b2.set(true);
                Ok(())
            })
            .run();
        assert!(result.is_err());
        assert!(!ran_b.get(), "b should be skipped after gate failure");
    }

    #[test]
    fn pipeline_gate_pass_continues() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_b = Rc::new(Cell::new(false));
        let ran_b2 = ran_b.clone();
        let result = Pipeline::new(false)
            .gate("preflight", || Ok(()))
            .step("b", move || {
                ran_b2.set(true);
                Ok(())
            })
            .run();
        assert!(result.is_ok());
        assert!(ran_b.get());
    }

    #[test]
    fn pipeline_fail_fast_false_runs_all_steps() {
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_c = Rc::new(Cell::new(false));
        let ran_c2 = ran_c.clone();
        let result = Pipeline::new(false)
            .step("a", || Ok(()))
            .step("b", || anyhow::bail!("b failed"))
            .step("c", move || {
                ran_c2.set(true);
                Ok(())
            })
            .run();
        assert!(result.is_err());
        assert!(ran_c.get(), "c should have run when fail_fast=false");
    }

    #[test]
    fn pipeline_with_no_steps_passes() {
        assert!(Pipeline::new(false).run().is_ok());
        assert!(Pipeline::new(true).run().is_ok());
    }

    #[test]
    fn pipeline_non_gate_failure_does_not_block_non_fail_fast() {
        // A failed non-gate step should NOT prevent subsequent steps when fail_fast=false.
        use std::cell::Cell;
        use std::rc::Rc;
        let ran_b = Rc::new(Cell::new(false));
        let ran_b2 = ran_b.clone();
        let result = Pipeline::new(false)
            .step("a", || anyhow::bail!("a failed"))
            .step("b", move || {
                ran_b2.set(true);
                Ok(())
            })
            .run();
        assert!(result.is_err());
        assert!(ran_b.get());
    }

    #[test]
    fn pipeline_multiple_failures_all_recorded_fail_fast_false() {
        // All steps run; pipeline returns Err because some failed.
        let result = Pipeline::new(false)
            .step("a", || anyhow::bail!("a"))
            .step("b", || anyhow::bail!("b"))
            .step("c", || Ok(()))
            .run();
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_error_message_indicates_ci_checks_failed() {
        let result = Pipeline::new(false)
            .step("a", || anyhow::bail!("something broke"))
            .run();
        assert!(result.unwrap_err().to_string().contains("CI checks failed"));
    }

    #[test]
    fn step_status_skipped_display() {
        assert_eq!(format!("{}", StepStatus::Skipped), "SKIP");
    }
}
