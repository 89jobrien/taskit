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

#[derive(Debug)]
pub struct PipelineOutcome {
    pub results: Vec<StepResult>,
    pub total: Duration,
    pub passed: bool,
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
    fn pipeline_outcome_default_is_passed() {
        let outcome = PipelineOutcome {
            results: vec![],
            total: Duration::ZERO,
            passed: true,
        };
        assert!(outcome.passed);
    }
}
