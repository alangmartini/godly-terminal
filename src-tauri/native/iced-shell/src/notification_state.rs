use std::collections::HashMap;

/// Tracks notification state per terminal.
///
/// Records unread output events and bell signals for each terminal.
/// State is cleared when a terminal gains focus (mark_read) or is closed (clear).
#[derive(Debug, Default)]
pub struct NotificationTracker {
    /// Count of unread output events per terminal (reset when terminal is focused).
    unread: HashMap<String, u32>,
    /// Whether a bell has fired since last focus (per terminal).
    bell: HashMap<String, bool>,
}

impl NotificationTracker {
    /// Creates a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an output event for a terminal (increment unread count).
    pub fn record_output(&mut self, terminal_id: &str) {
        *self.unread.entry(terminal_id.to_string()).or_insert(0) += 1;
    }

    /// Record a bell signal for a terminal.
    pub fn record_bell(&mut self, terminal_id: &str) {
        self.bell.insert(terminal_id.to_string(), true);
    }

    /// Mark a terminal as read (reset unread count and bell flag).
    /// Called when the terminal gains focus.
    pub fn mark_read(&mut self, terminal_id: &str) {
        self.unread.remove(terminal_id);
        self.bell.remove(terminal_id);
    }

    /// Returns the unread output count for a terminal.
    pub fn unread_count(&self, terminal_id: &str) -> u32 {
        self.unread.get(terminal_id).copied().unwrap_or(0)
    }

    /// Returns whether a bell has fired since last focus.
    pub fn has_bell(&self, terminal_id: &str) -> bool {
        self.bell.get(terminal_id).copied().unwrap_or(false)
    }

    /// Remove all notification state for a terminal (on close).
    pub fn clear(&mut self, terminal_id: &str) {
        self.unread.remove(terminal_id);
        self.bell.remove(terminal_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker_empty() {
        let tracker = NotificationTracker::new();
        assert_eq!(tracker.unread_count("t1"), 0);
        assert!(!tracker.has_bell("t1"));
    }

    #[test]
    fn test_record_output_increments() {
        let mut tracker = NotificationTracker::new();
        tracker.record_output("t1");
        assert_eq!(tracker.unread_count("t1"), 1);
        tracker.record_output("t1");
        assert_eq!(tracker.unread_count("t1"), 2);
        tracker.record_output("t1");
        assert_eq!(tracker.unread_count("t1"), 3);
    }

    #[test]
    fn test_record_bell() {
        let mut tracker = NotificationTracker::new();
        assert!(!tracker.has_bell("t1"));
        tracker.record_bell("t1");
        assert!(tracker.has_bell("t1"));
    }

    #[test]
    fn test_mark_read_resets() {
        let mut tracker = NotificationTracker::new();
        tracker.record_output("t1");
        tracker.record_output("t1");
        tracker.record_bell("t1");

        tracker.mark_read("t1");
        assert_eq!(tracker.unread_count("t1"), 0);
        assert!(!tracker.has_bell("t1"));
    }

    #[test]
    fn test_mark_read_unknown_is_noop() {
        let mut tracker = NotificationTracker::new();
        tracker.mark_read("nonexistent"); // Should not panic
        assert_eq!(tracker.unread_count("nonexistent"), 0);
    }

    #[test]
    fn test_clear_removes_all_state() {
        let mut tracker = NotificationTracker::new();
        tracker.record_output("t1");
        tracker.record_bell("t1");

        tracker.clear("t1");
        assert_eq!(tracker.unread_count("t1"), 0);
        assert!(!tracker.has_bell("t1"));
    }

    #[test]
    fn test_independent_terminals() {
        let mut tracker = NotificationTracker::new();
        tracker.record_output("t1");
        tracker.record_output("t2");
        tracker.record_output("t2");
        tracker.record_bell("t1");

        assert_eq!(tracker.unread_count("t1"), 1);
        assert_eq!(tracker.unread_count("t2"), 2);
        assert!(tracker.has_bell("t1"));
        assert!(!tracker.has_bell("t2"));

        tracker.mark_read("t1");
        assert_eq!(tracker.unread_count("t1"), 0);
        assert_eq!(tracker.unread_count("t2"), 2); // t2 unaffected
    }

    #[test]
    fn test_default_is_empty() {
        let tracker = NotificationTracker::default();
        assert_eq!(tracker.unread_count("any"), 0);
        assert!(!tracker.has_bell("any"));
    }
}
