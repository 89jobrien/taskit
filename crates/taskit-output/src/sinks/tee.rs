use crate::message::Message;
use crate::sink::MessageSink;

// TODO(audit): only constructed in this crate's own tests — not adopted in
// any production output path yet.
/// Fan-out sink: sends to multiple sinks simultaneously.
pub struct TeeSink {
    children: Vec<Box<dyn MessageSink>>,
}

impl TeeSink {
    pub fn new(children: Vec<Box<dyn MessageSink>>) -> Self {
        Self { children }
    }
}

impl MessageSink for TeeSink {
    fn emit(&self, msg: &Message) {
        for child in &self.children {
            child.emit(msg);
        }
    }

    fn flush(&self) {
        for child in &self.children {
            child.flush();
        }
    }
}
