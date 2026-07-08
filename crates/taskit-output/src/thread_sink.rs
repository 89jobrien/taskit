use std::sync::OnceLock;

use crate::sink::MessageSink;
use crate::sinks::stderr::StderrSink;

static SINK: OnceLock<Box<dyn MessageSink>> = OnceLock::new();

/// Set the active message sink. Must be called before any emit.
/// Subsequent calls are ignored (first-write-wins).
pub fn set_sink(sink: Box<dyn MessageSink>) {
    let _ = SINK.set(sink);
}

/// Get the active message sink. Returns `StderrSink` if none was set.
pub fn sink() -> &'static dyn MessageSink {
    SINK.get()
        .map_or(&StderrSink as &dyn MessageSink, |s| s.as_ref())
}
