use std::time::Duration;
use taskit_types::step::DiagnosticRecord;

/// Structured message emitted during pipeline execution.
#[derive(Debug, Clone)]
pub enum Message {
    /// Step lifecycle event.
    StepProgress {
        /// Step name.
        step: String,
        /// Lifecycle event payload.
        event: StepEvent,
    },
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
    /// Step has started.
    Started,
    /// Step passed.
    Passed {
        /// Runtime duration.
        duration: Duration,
    },
    /// Step failed.
    Failed {
        /// Runtime duration.
        duration: Duration,
        /// Human-readable failure message.
        error: String,
    },
    /// Step was skipped.
    Skipped,
}
