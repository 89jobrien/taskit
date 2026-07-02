use crate::message::Message;

/// Port: receives structured messages during pipeline execution.
pub trait MessageSink: Send + Sync {
    fn emit(&self, msg: &Message);
    fn flush(&self);
}
