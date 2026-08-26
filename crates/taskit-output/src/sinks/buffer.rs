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
    /// Create an empty in-memory buffer sink.
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return a snapshot of buffered messages.
    pub fn messages(&self) -> Vec<Message> {
        self.with_messages(|messages| messages.clone())
    }

    /// Number of buffered messages.
    pub fn len(&self) -> usize {
        self.with_messages(|messages| messages.len())
    }

    /// Whether no messages have been emitted.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn with_messages<T>(&self, f: impl FnOnce(&mut Vec<Message>) -> T) -> T {
        match self.messages.lock() {
            Ok(mut messages) => f(&mut messages),
            Err(poisoned) => {
                let mut messages = poisoned.into_inner();
                f(&mut messages)
            }
        }
    }
}

impl Default for BufferSink {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageSink for BufferSink {
    fn emit(&self, msg: &Message) {
        self.with_messages(|messages| messages.push(msg.clone()));
    }

    fn flush(&self) {
        // Nothing to flush for a buffer
    }
}
