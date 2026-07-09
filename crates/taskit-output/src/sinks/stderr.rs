use crate::message::{Message, StepEvent};
use crate::sink::MessageSink;

/// Human-readable stderr output sink (default).
pub struct StderrSink;

impl MessageSink for StderrSink {
    fn emit(&self, msg: &Message) {
        match msg {
            Message::StepProgress { step, event } => match event {
                StepEvent::Started => eprintln!("  + {step}"),
                StepEvent::Passed { duration } => {
                    eprintln!("  + {step} ({:.1}s)", duration.as_secs_f64());
                }
                StepEvent::Failed { duration, error } => {
                    eprintln!("  x {step} ({:.1}s): {error}", duration.as_secs_f64());
                }
                StepEvent::Skipped => eprintln!("  - {step} (skipped)"),
            },
            Message::Progress(msg) => eprintln!("  {msg}"),
            Message::Skip(msg) => eprintln!("  (skip) {msg}"),
            Message::DryRun(cmd) => eprintln!("dry-run: {cmd}"),
            Message::Success(msg) => eprintln!("  {msg}"),
            Message::Error(msg) => eprintln!("  ERROR: {msg}"),
            Message::Diagnostic(d) => {
                if let Some(file) = &d.file {
                    let line = d.line.map(|l| format!(":{l}")).unwrap_or_default();
                    eprintln!("  [{:?}] {}{line}: {}", d.level, file, d.message);
                } else {
                    eprintln!("  [{:?}] {}: {}", d.level, d.rule_id, d.message);
                }
            }
        }
    }

    fn flush(&self) {
        // stderr is unbuffered, nothing to flush
    }
}
