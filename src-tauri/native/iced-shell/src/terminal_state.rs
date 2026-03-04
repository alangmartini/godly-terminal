use std::collections::HashMap;

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
    /// Workspace this terminal belongs to (None = default workspace).
    pub workspace_id: Option<String>,
    /// User-assigned custom name (overrides title/process_name in tab label).
    pub custom_name: Option<String>,
}

impl TerminalInfo {
    /// Returns the display label for this terminal's tab.
    ///
    /// Priority: custom_name > title > process_name > "Terminal"
    pub fn tab_label(&self) -> &str {
        if let Some(ref name) = self.custom_name {
            if !name.is_empty() {
                return name;
            }
        }
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
///
/// Uses a `HashMap` for O(1) lookup by id, with a separate `order_list`
/// to maintain insertion order for iteration and tab display.
pub struct TerminalCollection {
    terminals: HashMap<String, TerminalInfo>,
    /// Maintains insertion order of terminal ids for ordered iteration.
    order_list: Vec<String>,
    active_id: Option<String>,
    next_order: u32,
}

impl TerminalCollection {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            order_list: Vec::new(),
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

        self.order_list.push(id.clone());
        self.terminals.insert(
            id.clone(),
            TerminalInfo {
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
                workspace_id: None,
                custom_name: None,
            },
        );

        if is_first {
            self.active_id = Some(id.clone());
        }

        self.terminals.get_mut(&id).unwrap()
    }

    /// Removes the terminal with the given id.
    ///
    /// If the removed terminal was active, the next terminal at the same index
    /// (or the previous one if at the end) becomes active.
    pub fn remove(&mut self, id: &str) {
        let Some(idx) = self.order_list.iter().position(|s| s == id) else {
            return;
        };

        let was_active = self.active_id.as_deref() == Some(id);
        self.order_list.remove(idx);
        self.terminals.remove(id);

        if was_active {
            if self.order_list.is_empty() {
                self.active_id = None;
            } else {
                // Prefer same index (next terminal), or fall back to previous.
                let new_idx = if idx < self.order_list.len() {
                    idx
                } else {
                    self.order_list.len() - 1
                };
                self.active_id = Some(self.order_list[new_idx].clone());
            }
        }
    }

    /// Returns a reference to the active terminal, if any.
    pub fn active(&self) -> Option<&TerminalInfo> {
        let id = self.active_id.as_deref()?;
        self.terminals.get(id)
    }

    /// Returns a mutable reference to the active terminal, if any.
    pub fn active_mut(&mut self) -> Option<&mut TerminalInfo> {
        let id = self.active_id.as_deref()?.to_owned();
        self.terminals.get_mut(&id)
    }

    /// Finds a terminal by id.
    pub fn get(&self, id: &str) -> Option<&TerminalInfo> {
        self.terminals.get(id)
    }

    /// Finds a terminal by id, mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut TerminalInfo> {
        self.terminals.get_mut(id)
    }

    /// Sets the active terminal by id. No-op if id not found.
    pub fn set_active(&mut self, id: &str) {
        if self.terminals.contains_key(id) {
            self.active_id = Some(id.to_owned());
        }
    }

    /// Returns the number of terminals in the collection.
    pub fn count(&self) -> usize {
        self.terminals.len()
    }

    /// Iterates over all terminals in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &TerminalInfo> {
        self.order_list
            .iter()
            .filter_map(|id| self.terminals.get(id))
    }

    /// Returns all terminals in insertion order as a `Vec` of references.
    ///
    /// Use this for the tab bar and other ordered displays.
    pub fn ordered_terminals(&self) -> Vec<&TerminalInfo> {
        self.order_list
            .iter()
            .filter_map(|id| self.terminals.get(id))
            .collect()
    }

    /// Returns the active terminal's id, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    /// Switch to the next terminal (wraps around).
    pub fn next(&mut self) {
        if self.order_list.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let next_idx = (idx + 1) % self.order_list.len();
            self.active_id = Some(self.order_list[next_idx].clone());
        }
    }

    /// Switch to the previous terminal (wraps around).
    pub fn previous(&mut self) {
        if self.order_list.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let prev_idx = if idx == 0 {
                self.order_list.len() - 1
            } else {
                idx - 1
            };
            self.active_id = Some(self.order_list[prev_idx].clone());
        }
    }

    /// Returns the index of the active terminal in the order list.
    fn active_index(&self) -> Option<usize> {
        let id = self.active_id.as_deref()?;
        self.order_list.iter().position(|s| s == id)
    }

    /// Adds a terminal to a specific workspace.
    ///
    /// Like `add()`, but also sets `workspace_id`. Returns a mutable reference
    /// to the newly created `TerminalInfo`.
    pub fn add_to_workspace(
        &mut self,
        id: String,
        rows: u16,
        cols: u16,
        workspace_id: String,
    ) -> &mut TerminalInfo {
        let order = self.next_order;
        self.next_order += 1;
        let is_first = self.terminals.is_empty();

        self.order_list.push(id.clone());
        self.terminals.insert(
            id.clone(),
            TerminalInfo {
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
                workspace_id: Some(workspace_id),
                custom_name: None,
            },
        );

        if is_first {
            self.active_id = Some(id.clone());
        }

        self.terminals.get_mut(&id).unwrap()
    }

    /// Set or clear the custom name for a terminal.
    pub fn rename(&mut self, id: &str, name: Option<String>) {
        if let Some(term) = self.get_mut(id) {
            term.custom_name = name;
        }
    }

    /// Returns terminals belonging to a specific workspace.
    pub fn terminals_for_workspace(&self, workspace_id: &str) -> Vec<&TerminalInfo> {
        self.order_list
            .iter()
            .filter_map(|id| self.terminals.get(id))
            .filter(|t| t.workspace_id.as_deref() == Some(workspace_id))
            .collect()
    }

    /// Move a terminal to a workspace (or unassign with None).
    pub fn set_workspace(&mut self, terminal_id: &str, workspace_id: Option<String>) {
        if let Some(term) = self.get_mut(terminal_id) {
            term.workspace_id = workspace_id;
        }
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

    #[test]
    fn test_tab_label_custom_name_priority() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        info.process_name = "pwsh".into();
        info.title = "My Shell".into();
        info.custom_name = Some("Custom Name".into());
        assert_eq!(info.tab_label(), "Custom Name");
    }

    #[test]
    fn test_tab_label_empty_custom_name_falls_through() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        info.custom_name = Some(String::new());
        info.title = "Title".into();
        assert_eq!(info.tab_label(), "Title");
    }

    #[test]
    fn test_rename() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.rename("t1", Some("My Terminal".into()));
        assert_eq!(col.get("t1").unwrap().tab_label(), "My Terminal");

        col.rename("t1", None);
        assert_eq!(col.get("t1").unwrap().tab_label(), "Terminal");
    }

    #[test]
    fn test_workspace_filtering() {
        let mut col = TerminalCollection::new();
        col.add_to_workspace("t1".into(), 24, 80, "w1".into());
        col.add_to_workspace("t2".into(), 24, 80, "w1".into());
        col.add_to_workspace("t3".into(), 24, 80, "w2".into());
        col.add("t4".into(), 24, 80); // No workspace

        let w1_terms = col.terminals_for_workspace("w1");
        assert_eq!(w1_terms.len(), 2);

        let w2_terms = col.terminals_for_workspace("w2");
        assert_eq!(w2_terms.len(), 1);

        let w3_terms = col.terminals_for_workspace("w3");
        assert_eq!(w3_terms.len(), 0);
    }

    #[test]
    fn test_set_workspace() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        assert!(col.get("t1").unwrap().workspace_id.is_none());

        col.set_workspace("t1", Some("w1".into()));
        assert_eq!(col.get("t1").unwrap().workspace_id.as_deref(), Some("w1"));

        col.set_workspace("t1", None);
        assert!(col.get("t1").unwrap().workspace_id.is_none());
    }

    #[test]
    fn test_new_fields_default_to_none() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        assert!(info.workspace_id.is_none());
        assert!(info.custom_name.is_none());
    }

    #[test]
    fn test_ordered_terminals_preserves_insertion_order() {
        let mut col = TerminalCollection::new();
        col.add("t3".into(), 24, 80);
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);

        let ordered: Vec<&str> = col.ordered_terminals().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ordered, vec!["t3", "t1", "t2"]);
    }

    #[test]
    fn test_ordered_terminals_after_removal() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        col.remove("t2");

        let ordered: Vec<&str> = col.ordered_terminals().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ordered, vec!["t1", "t3"]);
    }
}
