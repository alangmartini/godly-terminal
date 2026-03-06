/// Deterministic state machine for ordered tabs.
///
/// This crate intentionally contains no I/O and no UI types, so it can be
/// tested quickly and reused across frontends.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TabState {
    order: Vec<String>,
    active_id: Option<String>,
}

impl TabState {
    /// Create an empty tab state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of tabs.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Returns true when there are no tabs.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Returns true when the provided tab ID is present.
    pub fn contains(&self, id: &str) -> bool {
        self.order.iter().any(|existing| existing == id)
    }

    /// Active tab id, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    /// Ordered tab IDs.
    pub fn ids(&self) -> &[String] {
        &self.order
    }

    /// Return the index of the tab id in the current order.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.order.iter().position(|existing| existing == id)
    }

    /// Open a new tab.
    ///
    /// Returns `true` if the tab was inserted, `false` if the id already
    /// existed and state was unchanged.
    pub fn open(&mut self, id: impl Into<String>) -> bool {
        let id = id.into();
        if self.contains(&id) {
            return false;
        }

        let first = self.order.is_empty();
        self.order.push(id.clone());

        if first {
            self.active_id = Some(id);
        }

        true
    }

    /// Close a tab.
    ///
    /// Returns `true` if the tab existed and was removed.
    pub fn close(&mut self, id: &str) -> bool {
        let Some(idx) = self.order.iter().position(|existing| existing == id) else {
            return false;
        };

        let was_active = self.active_id.as_deref() == Some(id);
        self.order.remove(idx);

        if was_active {
            if self.order.is_empty() {
                self.active_id = None;
            } else {
                let new_idx = if idx < self.order.len() {
                    idx
                } else {
                    self.order.len() - 1
                };
                self.active_id = Some(self.order[new_idx].clone());
            }
        }

        true
    }

    /// Make a tab active.
    ///
    /// Returns `true` when the tab exists and became active.
    pub fn activate(&mut self, id: &str) -> bool {
        if !self.contains(id) {
            return false;
        }
        self.active_id = Some(id.to_owned());
        true
    }

    /// Move active tab forward (wrap-around).
    pub fn next(&mut self) {
        if self.order.len() <= 1 {
            return;
        }

        if let Some(idx) = self.active_index() {
            let next_idx = (idx + 1) % self.order.len();
            self.active_id = Some(self.order[next_idx].clone());
        }
    }

    /// Move active tab backward (wrap-around).
    pub fn previous(&mut self) {
        if self.order.len() <= 1 {
            return;
        }

        if let Some(idx) = self.active_index() {
            let prev_idx = if idx == 0 {
                self.order.len() - 1
            } else {
                idx - 1
            };
            self.active_id = Some(self.order[prev_idx].clone());
        }
    }

    /// Reorder tabs by index.
    ///
    /// Returns `true` if both indices are valid and a move was applied.
    pub fn reorder(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.order.len() || to_index >= self.order.len() {
            return false;
        }
        if from_index == to_index {
            return true;
        }

        let item = self.order.remove(from_index);
        self.order.insert(to_index, item);
        true
    }

    fn active_index(&self) -> Option<usize> {
        let id = self.active_id.as_deref()?;
        self.index_of(id)
    }
}

#[cfg(test)]
mod tests {
    use super::TabState;

    #[test]
    fn open_first_sets_active() {
        let mut tabs = TabState::new();
        assert!(tabs.open("t1"));
        assert_eq!(tabs.active_id(), Some("t1"));
        assert_eq!(tabs.ids(), &["t1"]);
    }

    #[test]
    fn open_duplicate_is_ignored() {
        let mut tabs = TabState::new();
        assert!(tabs.open("t1"));
        assert!(!tabs.open("t1"));
        assert_eq!(tabs.ids(), &["t1"]);
        assert_eq!(tabs.active_id(), Some("t1"));
    }

    #[test]
    fn close_active_prefers_same_index() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        assert!(tabs.close("t1"));
        assert_eq!(tabs.active_id(), Some("t2"));

        assert!(tabs.close("t2"));
        assert_eq!(tabs.active_id(), Some("t3"));
    }

    #[test]
    fn close_last_active_moves_to_previous() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");
        tabs.activate("t3");

        assert!(tabs.close("t3"));
        assert_eq!(tabs.active_id(), Some("t2"));
    }

    #[test]
    fn close_non_active_does_not_change_active() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        assert!(tabs.close("t2"));
        assert_eq!(tabs.active_id(), Some("t1"));
    }

    #[test]
    fn activate_unknown_is_noop() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        assert!(!tabs.activate("missing"));
        assert_eq!(tabs.active_id(), Some("t1"));
    }

    #[test]
    fn next_wraps() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        tabs.next();
        assert_eq!(tabs.active_id(), Some("t2"));
        tabs.next();
        assert_eq!(tabs.active_id(), Some("t3"));
        tabs.next();
        assert_eq!(tabs.active_id(), Some("t1"));
    }

    #[test]
    fn previous_wraps() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        tabs.previous();
        assert_eq!(tabs.active_id(), Some("t3"));
        tabs.previous();
        assert_eq!(tabs.active_id(), Some("t2"));
        tabs.previous();
        assert_eq!(tabs.active_id(), Some("t1"));
    }

    #[test]
    fn reorder_moves_item() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        assert!(tabs.reorder(0, 2));
        assert_eq!(tabs.ids(), &["t2", "t3", "t1"]);
        assert_eq!(tabs.active_id(), Some("t1"));
    }

    #[test]
    fn reorder_out_of_bounds_rejected() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");

        assert!(!tabs.reorder(2, 0));
        assert!(!tabs.reorder(0, 2));
        assert_eq!(tabs.ids(), &["t1", "t2"]);
    }

    #[test]
    fn index_of_returns_position() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        assert_eq!(tabs.index_of("t1"), Some(0));
        assert_eq!(tabs.index_of("t2"), Some(1));
        assert_eq!(tabs.index_of("t3"), Some(2));
        assert_eq!(tabs.index_of("missing"), None);
    }

    #[test]
    fn navigation_wraps_after_reorder() {
        let mut tabs = TabState::new();
        tabs.open("t1");
        tabs.open("t2");
        tabs.open("t3");

        assert!(tabs.reorder(2, 0));
        assert_eq!(tabs.ids(), &["t3", "t1", "t2"]);

        // Active remains t1 after reorder.
        assert_eq!(tabs.active_id(), Some("t1"));

        tabs.activate("t2");
        tabs.next();
        assert_eq!(tabs.active_id(), Some("t3"));

        tabs.previous();
        assert_eq!(tabs.active_id(), Some("t2"));
    }
}
