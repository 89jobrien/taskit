use std::fmt;
use std::time::Duration;

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

/// Severity level for a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

// TODO(test): unit tests for DiagnosticRecord and DiagnosticLevel
/// A single diagnostic finding from a tool (clippy warning, test failure, etc.).
#[derive(Debug, Clone)]
pub struct DiagnosticRecord {
    /// Tool-specific rule id (e.g. "clippy::needless_return", "TE001").
    pub rule_id: String,
    /// Human-readable message.
    pub message: String,
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Source file path (relative to workspace root).
    pub file: Option<String>,
    /// 1-based line number.
    pub line: Option<usize>,
    /// 1-based column number.
    pub column: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub status: StepStatus,
    pub duration: Duration,
    pub error: Option<String>,
    pub gate: bool,
    /// Per-finding diagnostics captured from the tool's structured output.
    pub diagnostics: Vec<DiagnosticRecord>,
}

// TODO(test): test PipelineOutcome with passed=false case
#[derive(Debug)]
pub struct PipelineOutcome {
    pub results: Vec<StepResult>,
    pub total: Duration,
    pub passed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn step_status_display() {
        assert_eq!(format!("{}", StepStatus::Pass), "PASS");
        assert_eq!(format!("{}", StepStatus::Fail), "FAIL");
        assert_eq!(format!("{}", StepStatus::Skipped), "SKIP");
    }

    #[test]
    fn pipeline_outcome_default_is_passed() {
        let outcome = PipelineOutcome {
            results: vec![],
            total: Duration::ZERO,
            passed: true,
        };
        assert!(outcome.passed);
    }

    // --- property tests ---

    fn arb_step_status() -> impl Strategy<Value = StepStatus> {
        prop_oneof![
            Just(StepStatus::Pass),
            Just(StepStatus::Fail),
            Just(StepStatus::Skipped),
        ]
    }

    /// Build a minimal StepResult with the given name, status, and duration (nanos).
    fn make_step(name: String, status: StepStatus, nanos: u64) -> StepResult {
        StepResult {
            name,
            status,
            duration: Duration::from_nanos(nanos),
            error: None,
            gate: false,
            diagnostics: vec![],
        }
    }

    proptest! {
        /// Any StepStatus Display output must be non-empty.
        #[test]
        fn prop_step_status_display_non_empty(status in arb_step_status()) {
            let s = format!("{}", status);
            prop_assert!(!s.is_empty());
        }

        /// A PipelineOutcome constructed with N all-Pass steps and passed=true
        /// preserves len(results)==N and every status is Pass.
        #[test]
        fn prop_all_pass_outcome_consistent(n in 1usize..=5) {
            let results: Vec<StepResult> = (0..n)
                .map(|i| make_step(format!("step-{i}"), StepStatus::Pass, 0))
                .collect();
            let outcome = PipelineOutcome {
                total: Duration::ZERO,
                passed: true,
                results,
            };
            prop_assert_eq!(outcome.results.len(), n);
            prop_assert!(outcome.results.iter().all(|r| r.status == StepStatus::Pass));
        }

        /// The outcome total duration must be >= any individual step duration.
        #[test]
        fn prop_total_gte_max_step(
            step_nanos in proptest::collection::vec(0u64..=1_000_000_000u64, 1..=5),
            extra_nanos in 0u64..=1_000_000_000u64,
        ) {
            let results: Vec<StepResult> = step_nanos
                .iter()
                .enumerate()
                .map(|(i, &nanos)| make_step(format!("step-{i}"), StepStatus::Pass, nanos))
                .collect();
            let max_step = step_nanos.iter().copied().max().unwrap_or(0);
            let total = Duration::from_nanos(max_step + extra_nanos);
            let outcome = PipelineOutcome { results, total, passed: true };
            let max_individual = outcome
                .results
                .iter()
                .map(|r| r.duration)
                .max()
                .unwrap_or(Duration::ZERO);
            prop_assert!(outcome.total >= max_individual);
        }

        /// A non-empty name passed to StepResult is preserved exactly.
        #[test]
        fn prop_step_result_name_preserved(name in "[a-zA-Z0-9_\\-]{1,64}") {
            let step = make_step(name.clone(), StepStatus::Pass, 0);
            prop_assert_eq!(&step.name, &name);
        }
    }
}
