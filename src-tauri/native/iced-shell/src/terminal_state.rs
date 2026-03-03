use godly_protocol::types::RichGridData;

/// Information about a single terminal session.
pub struct TerminalInfo {
    pub id: String,
    pub title: String,
    pub process_name: String,
    pub order: u32,
    pub grid: Option<RichGridData>,
    pub dirty: bool,
    pub fetching: bool,
    pub rows: u16,
    pub cols: u16,
    pub exited: bool,
    pub exit_code: Option<i64>,
    /// Current scrollback offset (0 = live view, >0 = scrolled into history).
    pub scrollback_offset: usize,
    /// Total number of scrollback rows available.
    pub total_scrollback: usize,
}

impl TerminalInfo {
    /// Returns the display label for this terminal's tab.
    ///
    /// Priority: title > process_name > "Terminal"
    pub fn tab_label(&self) -> &str {
        if !self.title.is_empty() {
            &self.title
        } else if !self.process_name.is_empty() {
            &self.process_name
        } else {
            "Terminal"
        }
    }
}

/// Collection of terminal sessions with active tab tracking.
pub struct TerminalCollection {
    terminals: Vec<TerminalInfo>,
    active_id: Option<String>,
    next_order: u32,
}

impl TerminalCollection {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self {
            terminals: Vec::new(),
            active_id: None,
            next_order: 0,
        }
    }

    /// Adds a new terminal with the given id and grid dimensions.
    ///
    /// Auto-increments the order counter. Sets as active if this is the first terminal.
    /// Returns a mutable reference to the newly created `TerminalInfo`.
    pub fn add(&mut self, id: String, rows: u16, cols: u16) -> &mut TerminalInfo {
        let order = self.next_order;
        self.next_order += 1;

        let is_first = self.terminals.is_empty();

        self.terminals.push(TerminalInfo {
            id: id.clone(),
            title: String::new(),
            process_name: String::new(),
            order,
            grid: None,
            dirty: false,
            fetching: false,
            rows,
            cols,
            exited: false,
            exit_code: None,
            scrollback_offset: 0,
            total_scrollback: 0,
        });

        if is_first {
            self.active_id = Some(id.clone());
        }

        // Return mutable reference to the last element (the one we just pushed).
        self.terminals.last_mut().unwrap()
    }

    /// Removes the terminal with the given id.
    ///
    /// If the removed terminal was active, the next terminal at the same index
    /// (or the previous one if at the end) becomes active.
    pub fn remove(&mut self, id: &str) {
        let Some(idx) = self.terminals.iter().position(|t| t.id == id) else {
            return;
        };

        let was_active = self.active_id.as_deref() == Some(id);
        self.terminals.remove(idx);

        if was_active {
            if self.terminals.is_empty() {
                self.active_id = None;
            } else {
                // Prefer same index (next terminal), or fall back to previous.
                let new_idx = if idx < self.terminals.len() {
                    idx
                } else {
                    self.terminals.len() - 1
                };
                self.active_id = Some(self.terminals[new_idx].id.clone());
            }
        }
    }

    /// Returns a reference to the active terminal, if any.
    pub fn active(&self) -> Option<&TerminalInfo> {
        let id = self.active_id.as_deref()?;
        self.terminals.iter().find(|t| t.id == id)
    }

    /// Returns a mutable reference to the active terminal, if any.
    pub fn active_mut(&mut self) -> Option<&mut TerminalInfo> {
        let id = self.active_id.as_deref()?.to_owned();
        self.terminals.iter_mut().find(|t| t.id == id)
    }

    /// Finds a terminal by id.
    pub fn get(&self, id: &str) -> Option<&TerminalInfo> {
        self.terminals.iter().find(|t| t.id == id)
    }

    /// Finds a terminal by id, mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut TerminalInfo> {
        self.terminals.iter_mut().find(|t| t.id == id)
    }

    /// Sets the active terminal by id. No-op if id not found.
    pub fn set_active(&mut self, id: &str) {
        if self.terminals.iter().any(|t| t.id == id) {
            self.active_id = Some(id.to_owned());
        }
    }

    /// Returns the number of terminals in the collection.
    pub fn count(&self) -> usize {
        self.terminals.len()
    }

    /// Iterates over all terminals.
    pub fn iter(&self) -> impl Iterator<Item = &TerminalInfo> {
        self.terminals.iter()
    }

    /// Returns all terminals as a slice.
    pub fn as_slice(&self) -> &[TerminalInfo] {
        &self.terminals
    }

    /// Returns the active terminal's id, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    /// Switch to the next terminal (wraps around).
    pub fn next(&mut self) {
        if self.terminals.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let next_idx = (idx + 1) % self.terminals.len();
            self.active_id = Some(self.terminals[next_idx].id.clone());
        }
    }

    /// Switch to the previous terminal (wraps around).
    pub fn previous(&mut self) {
        if self.terminals.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let prev_idx = if idx == 0 {
                self.terminals.len() - 1
            } else {
                idx - 1
            };
            self.active_id = Some(self.terminals[prev_idx].id.clone());
        }
    }

    /// Returns the index of the active terminal.
    fn active_index(&self) -> Option<usize> {
        let id = self.active_id.as_deref()?;
        self.terminals.iter().position(|t| t.id == id)
    }
}

impl Default for TerminalCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_first_becomes_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.count(), 1);
    }

    #[test]
    fn test_add_second_does_not_change_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.count(), 2);
    }

    #[test]
    fn test_remove_active_picks_next() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        // Active is t1 (first added). Remove it -> t2 should become active (same index 0).
        col.remove("t1");
        assert_eq!(col.active_id(), Some("t2"));
        assert_eq!(col.count(), 2);

        // Remove t2 (active) -> t3 should become active.
        col.remove("t2");
        assert_eq!(col.active_id(), Some("t3"));
        assert_eq!(col.count(), 1);

        // Remove last terminal -> no active.
        col.remove("t3");
        assert_eq!(col.active_id(), None);
        assert_eq!(col.count(), 0);
    }

    #[test]
    fn test_remove_last_active_picks_previous() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        // Make t3 active, then remove it -> should pick t2 (previous).
        col.set_active("t3");
        assert_eq!(col.active_id(), Some("t3"));
        col.remove("t3");
        assert_eq!(col.active_id(), Some("t2"));
    }

    #[test]
    fn test_remove_non_active_preserves_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        // Active is t1. Remove t2 -> active should still be t1.
        col.remove("t2");
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.count(), 2);
    }

    #[test]
    fn test_tab_label_priority() {
        let mut col = TerminalCollection::new();

        // No title, no process_name -> "Terminal"
        let info = col.add("t1".into(), 24, 80);
        assert_eq!(info.tab_label(), "Terminal");

        // process_name set, no title -> process_name
        info.process_name = "pwsh".into();
        assert_eq!(info.tab_label(), "pwsh");

        // title set -> title takes priority
        info.title = "My Shell".into();
        assert_eq!(info.tab_label(), "My Shell");
    }

    #[test]
    fn test_get_and_get_mut() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 30, 100);

        // get
        let t1 = col.get("t1").unwrap();
        assert_eq!(t1.rows, 24);
        assert_eq!(t1.cols, 80);

        let t2 = col.get("t2").unwrap();
        assert_eq!(t2.rows, 30);
        assert_eq!(t2.cols, 100);

        assert!(col.get("nonexistent").is_none());

        // get_mut
        {
            let t1_mut = col.get_mut("t1").unwrap();
            t1_mut.dirty = true;
        }
        assert!(col.get("t1").unwrap().dirty);

        assert!(col.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_set_active_nonexistent_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.set_active("nonexistent");
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_iter() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        let ids: Vec<&str> = col.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn test_order_auto_increments() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        let orders: Vec<u32> = col.iter().map(|t| t.order).collect();
        assert_eq!(orders, vec![0, 1, 2]);
    }

    #[test]
    fn test_active_and_active_mut() {
        let mut col = TerminalCollection::new();
        assert!(col.active().is_none());
        assert!(col.active_mut().is_none());

        col.add("t1".into(), 24, 80);
        assert_eq!(col.active().unwrap().id, "t1");

        col.active_mut().unwrap().title = "Hello".into();
        assert_eq!(col.active().unwrap().title, "Hello");
    }

    #[test]
    fn test_next_wraps_around() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        assert_eq!(col.active_id(), Some("t1"));

        col.next();
        assert_eq!(col.active_id(), Some("t2"));

        col.next();
        assert_eq!(col.active_id(), Some("t3"));

        // Wraps around to t1.
        col.next();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_previous_wraps_around() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        assert_eq!(col.active_id(), Some("t1"));

        // Wraps around to t3.
        col.previous();
        assert_eq!(col.active_id(), Some("t3"));

        col.previous();
        assert_eq!(col.active_id(), Some("t2"));

        col.previous();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_next_single_terminal_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);

        col.next();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_previous_single_terminal_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);

        col.previous();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_next_empty_is_noop() {
        let mut col = TerminalCollection::new();
        col.next();
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_previous_empty_is_noop() {
        let mut col = TerminalCollection::new();
        col.previous();
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_next_previous_round_trip() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        col.set_active("t2");
        assert_eq!(col.active_id(), Some("t2"));

        col.next();
        assert_eq!(col.active_id(), Some("t3"));

        col.previous();
        assert_eq!(col.active_id(), Some("t2"));
    }

    #[test]
    fn test_scrollback_fields_initialized_to_zero() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        assert_eq!(info.scrollback_offset, 0);
        assert_eq!(info.total_scrollback, 0);
    }
}
