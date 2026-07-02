use crate::message::Message;
use crate::sink::MessageSink;

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
