/// In-memory sink used primarily for tests.
pub mod buffer;
/// Sink that writes to stderr.
pub mod stderr;
/// Fan-out sink that writes to multiple child sinks.
pub mod tee;
