use std::time::Duration;
use taskit_types::step::DiagnosticRecord;

/// Structured message emitted during pipeline execution.
#[derive(Debug, Clone)]
pub enum Message {
    /// Step lifecycle event.
    StepProgress { step: String, event: StepEvent },
    /// General progress message.
    Progress(String),
    /// Something was skipped.
    Skip(String),
    /// Dry-run: would have executed this command.
    DryRun(String),
    /// Success message.
    Success(String),
    /// Error detail during execution.
    Error(String),
    /// Structured diagnostic finding.
    Diagnostic(DiagnosticRecord),
}

/// Lifecycle events for a pipeline step.
#[derive(Debug, Clone)]
pub enum StepEvent {
    Started,
    Passed { duration: Duration },
    Failed { duration: Duration, error: String },
    Skipped,
}
