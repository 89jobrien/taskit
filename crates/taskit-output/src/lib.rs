//! Output formatting and sink abstractions for taskit CLI messages.

/// Output formatters for pipeline outcomes.
pub mod formatter;
mod macros;
/// Message/event types emitted by commands.
pub mod message;
/// Sink trait abstraction.
pub mod sink;
/// Concrete sink implementations.
pub mod sinks;
/// Thread-local sink management utilities.
pub mod thread_sink;

/// Re-exported formatter types and helpers.
pub use formatter::{
    DiagnosticFormatter, GithubFormatter, HumanFormatter, JsonFormatter, JunitFormatter,
    OutputFormatter, SarifFormatter, formatter_for, write_output,
};
/// Re-exported message/event types.
pub use message::{Message, StepEvent};
/// Re-exported sink trait.
pub use sink::MessageSink;
/// Re-exported buffer sink.
pub use sinks::buffer::BufferSink;
/// Re-exported stderr sink.
pub use sinks::stderr::StderrSink;
/// Re-exported tee sink.
pub use sinks::tee::TeeSink;
/// Re-exported thread-local sink accessors.
pub use thread_sink::{set_sink, sink};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stderr_sink_smoke() {
        let sink = StderrSink;
        sink.emit(&Message::Progress("testing".into()));
        sink.emit(&Message::Skip("skipped".into()));
        sink.emit(&Message::DryRun("cargo fmt".into()));
        sink.emit(&Message::Success("done".into()));
        sink.emit(&Message::Error("oops".into()));
        sink.emit(&Message::StepProgress {
            step: "lint".into(),
            event: StepEvent::Started,
        });
        sink.emit(&Message::StepProgress {
            step: "lint".into(),
            event: StepEvent::Passed {
                duration: Duration::from_secs(1),
            },
        });
        sink.emit(&Message::StepProgress {
            step: "test".into(),
            event: StepEvent::Failed {
                duration: Duration::from_secs(2),
                error: "bad".into(),
            },
        });
        sink.emit(&Message::StepProgress {
            step: "audit".into(),
            event: StepEvent::Skipped,
        });
        sink.flush();
    }

    #[test]
    fn buffer_sink_collects_messages() {
        let buf = BufferSink::new();
        buf.emit(&Message::Progress("a".into()));
        buf.emit(&Message::Progress("b".into()));
        buf.emit(&Message::Progress("c".into()));
        assert_eq!(buf.len(), 3);
        assert!(!buf.is_empty());
    }

    #[test]
    fn buffer_sink_empty_by_default() {
        let buf = BufferSink::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn tee_sink_fans_out() {
        let b1 = BufferSink::new();
        let b2 = BufferSink::new();
        let tee = TeeSink::new(vec![Box::new(b1.clone()), Box::new(b2.clone())]);
        tee.emit(&Message::Progress("hello".into()));
        tee.emit(&Message::Success("done".into()));
        tee.flush();
        assert_eq!(b1.len(), 2);
        assert_eq!(b2.len(), 2);
    }

    #[test]
    fn emit_after_flush_works() {
        let buf = BufferSink::new();
        buf.emit(&Message::Progress("before".into()));
        buf.flush();
        buf.emit(&Message::Progress("after".into()));
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn default_sink_is_stderr() {
        // Just verify it doesn't panic
        let s = sink();
        s.emit(&Message::Progress("default sink test".into()));
    }
}
