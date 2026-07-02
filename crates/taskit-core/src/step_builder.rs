//! Builder for [`StepResult`] — eliminates 6-field struct literals in tests.

use std::time::Duration;

use taskit_types::step::{DiagnosticLevel, DiagnosticRecord, StepResult, StepStatus};

/// Construct a [`StepResult`] with sensible defaults.
///
/// Defaults: status = Pass, duration = 0, error = None, gate = false,
/// diagnostics = empty.
pub struct StepBuilder {
    name: String,
    status: StepStatus,
    duration: Duration,
    error: Option<String>,
    gate: bool,
    diagnostics: Vec<DiagnosticRecord>,
}

impl StepBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: StepStatus::Pass,
            duration: Duration::ZERO,
            error: None,
            gate: false,
            diagnostics: vec![],
        }
    }

    pub fn status(mut self, status: StepStatus) -> Self {
        self.status = status;
        self
    }

    pub fn fail(self) -> Self {
        self.status(StepStatus::Fail)
    }

    pub fn skip(self) -> Self {
        self.status(StepStatus::Skipped)
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn duration_ms(self, ms: u64) -> Self {
        self.duration(Duration::from_millis(ms))
    }

    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn gate(mut self) -> Self {
        self.gate = true;
        self
    }

    pub fn diagnostic(mut self, rule_id: impl Into<String>, message: impl Into<String>) -> Self {
        self.diagnostics.push(DiagnosticRecord {
            rule_id: rule_id.into(),
            message: message.into(),
            level: DiagnosticLevel::Warning,
            file: None,
            line: None,
            column: None,
        });
        self
    }

    pub fn build(self) -> StepResult {
        StepResult {
            name: self.name,
            status: self.status,
            duration: self.duration,
            error: self.error,
            gate: self.gate,
            diagnostics: self.diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_step_is_pass() {
        let step = StepBuilder::new("fmt").build();
        assert_eq!(step.name, "fmt");
        assert_eq!(step.status, StepStatus::Pass);
        assert_eq!(step.duration, Duration::ZERO);
        assert!(step.error.is_none());
        assert!(!step.gate);
        assert!(step.diagnostics.is_empty());
    }

    #[test]
    fn fail_sets_status() {
        let step = StepBuilder::new("lint").fail().build();
        assert_eq!(step.status, StepStatus::Fail);
    }

    #[test]
    fn skip_sets_status() {
        let step = StepBuilder::new("test").skip().build();
        assert_eq!(step.status, StepStatus::Skipped);
    }

    #[test]
    fn duration_ms_sets_duration() {
        let step = StepBuilder::new("test").duration_ms(42).build();
        assert_eq!(step.duration, Duration::from_millis(42));
    }

    #[test]
    fn error_sets_message() {
        let step = StepBuilder::new("lint")
            .fail()
            .error("clippy warning")
            .build();
        assert_eq!(step.error.as_deref(), Some("clippy warning"));
    }

    #[test]
    fn gate_sets_flag() {
        let step = StepBuilder::new("fmt").gate().build();
        assert!(step.gate);
    }

    #[test]
    fn diagnostic_appends() {
        let step = StepBuilder::new("lint")
            .diagnostic("W001", "warning 1")
            .diagnostic("W002", "warning 2")
            .build();
        assert_eq!(step.diagnostics.len(), 2);
    }

    #[test]
    fn chained_builder() {
        let step = StepBuilder::new("coverage")
            .fail()
            .duration_ms(100)
            .error("below threshold")
            .gate()
            .diagnostic("COV001", "72% < 80%")
            .build();
        assert_eq!(step.name, "coverage");
        assert_eq!(step.status, StepStatus::Fail);
        assert_eq!(step.duration, Duration::from_millis(100));
        assert_eq!(step.error.as_deref(), Some("below threshold"));
        assert!(step.gate);
        assert_eq!(step.diagnostics.len(), 1);
        assert_eq!(step.diagnostics[0].message, "72% < 80%");
    }
}
