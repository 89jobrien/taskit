use crate::message::Message;

/// Port: receives structured messages during pipeline execution.
pub trait MessageSink: Send + Sync {
    /// Emit one message.
    fn emit(&self, msg: &Message);
    /// Flush buffered output, if any.
    fn flush(&self);
}
