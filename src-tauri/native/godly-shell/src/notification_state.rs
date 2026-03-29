//! Notification tracking per terminal.

use std::collections::HashMap;

/// Tracks notification state per terminal.
#[derive(Debug, Default)]
pub struct NotificationTracker {
    unread: HashMap<String, u32>,
    bell: HashMap<String, bool>,
    last_mcp_message: HashMap<String, String>,
}

impl NotificationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_output(&mut self, terminal_id: &str) {
        *self.unread.entry(terminal_id.to_string()).or_insert(0) += 1;
    }

    pub fn record_bell(&mut self, terminal_id: &str) {
        self.bell.insert(terminal_id.to_string(), true);
    }

    pub fn mark_read(&mut self, terminal_id: &str) {
        self.unread.remove(terminal_id);
        self.bell.remove(terminal_id);
        self.last_mcp_message.remove(terminal_id);
    }

    pub fn unread_count(&self, terminal_id: &str) -> u32 {
        self.unread.get(terminal_id).copied().unwrap_or(0)
    }

    pub fn has_bell(&self, terminal_id: &str) -> bool {
        self.bell.get(terminal_id).copied().unwrap_or(false)
    }

    pub fn clear(&mut self, terminal_id: &str) {
        self.unread.remove(terminal_id);
        self.bell.remove(terminal_id);
        self.last_mcp_message.remove(terminal_id);
    }

    pub fn is_mcp_duplicate(&mut self, terminal_id: &str, message: &str) -> bool {
        if let Some(last) = self.last_mcp_message.get(terminal_id) {
            if last == message {
                return true;
            }
        }
        self.last_mcp_message.insert(terminal_id.to_string(), message.to_string());
        false
    }
}
