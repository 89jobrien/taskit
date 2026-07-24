use std::sync::{Arc, Mutex};

use crate::message::Message;
use crate::sink::MessageSink;

// TODO(audit): only constructed in this crate's own tests — not adopted in
// any production output path yet.
/// Collects messages into a Vec for testing or buffered output.
#[derive(Clone)]
pub struct BufferSink {
    messages: Arc<Mutex<Vec<Message>>>,
}

impl BufferSink {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn messages(&self) -> Vec<Message> {
        self.messages.lock().unwrap().clone()
    }

    pub fn len(&self) -> usize {
        self.messages.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for BufferSink {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageSink for BufferSink {
    fn emit(&self, msg: &Message) {
        self.messages.lock().unwrap().push(msg.clone());
    }

    fn flush(&self) {
        // Nothing to flush for a buffer
    }
}
