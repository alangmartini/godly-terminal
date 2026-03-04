use crate::split_pane::LayoutNode;

/// Information about a single workspace.
pub struct WorkspaceInfo {
    /// Unique workspace ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Layout tree of terminal panes in this workspace.
    pub layout: LayoutNode,
    /// ID of the focused terminal pane within this workspace.
    pub focused_terminal: String,
}

/// Collection of workspaces with active workspace tracking.
pub struct WorkspaceCollection {
    workspaces: Vec<WorkspaceInfo>,
    active_id: Option<String>,
}

impl WorkspaceCollection {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            active_id: None,
        }
    }

    /// Adds a new workspace with the given id, name, and an initial terminal.
    ///
    /// Creates a single-leaf layout containing `initial_terminal_id`.
    /// Sets as active if this is the first workspace.
    /// Returns a mutable reference to the newly created `WorkspaceInfo`.
    pub fn add(
        &mut self,
        id: String,
        name: String,
        initial_terminal_id: String,
    ) -> &mut WorkspaceInfo {
        let is_first = self.workspaces.is_empty();

        let focused = initial_terminal_id.clone();
        self.workspaces.push(WorkspaceInfo {
            id: id.clone(),
            name,
            layout: LayoutNode::Leaf {
                terminal_id: initial_terminal_id,
            },
            focused_terminal: focused,
        });

        if is_first {
            self.active_id = Some(id);
        }

        // Return mutable reference to the last element (the one we just pushed).
        self.workspaces.last_mut().unwrap()
    }

    /// Removes the workspace with the given id.
    ///
    /// If the removed workspace was active, the next workspace at the same index
    /// (or the previous one if at the end) becomes active.
    pub fn remove(&mut self, id: &str) {
        let Some(idx) = self.workspaces.iter().position(|w| w.id == id) else {
            return;
        };

        let was_active = self.active_id.as_deref() == Some(id);
        self.workspaces.remove(idx);

        if was_active {
            if self.workspaces.is_empty() {
                self.active_id = None;
            } else {
                // Prefer same index (next workspace), or fall back to previous.
                let new_idx = if idx < self.workspaces.len() {
                    idx
                } else {
                    self.workspaces.len() - 1
                };
                self.active_id = Some(self.workspaces[new_idx].id.clone());
            }
        }
    }

    /// Returns a reference to the active workspace, if any.
    pub fn active(&self) -> Option<&WorkspaceInfo> {
        let id = self.active_id.as_deref()?;
        self.workspaces.iter().find(|w| w.id == id)
    }

    /// Returns a mutable reference to the active workspace, if any.
    pub fn active_mut(&mut self) -> Option<&mut WorkspaceInfo> {
        let id = self.active_id.as_deref()?.to_owned();
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    /// Finds a workspace by id.
    pub fn get(&self, id: &str) -> Option<&WorkspaceInfo> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    /// Finds a workspace by id, mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkspaceInfo> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    /// Sets the active workspace by id. No-op if id not found.
    pub fn set_active(&mut self, id: &str) {
        if self.workspaces.iter().any(|w| w.id == id) {
            self.active_id = Some(id.to_owned());
        }
    }

    /// Returns the number of workspaces in the collection.
    pub fn count(&self) -> usize {
        self.workspaces.len()
    }

    /// Iterates over all workspaces.
    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceInfo> {
        self.workspaces.iter()
    }

    /// Returns the workspaces as a slice.
    pub fn as_slice(&self) -> &[WorkspaceInfo] {
        &self.workspaces
    }

    /// Returns the active workspace's id, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    /// Switch to the next workspace (wraps around).
    pub fn next(&mut self) {
        if self.workspaces.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let next_idx = (idx + 1) % self.workspaces.len();
            self.active_id = Some(self.workspaces[next_idx].id.clone());
        }
    }

    /// Switch to the previous workspace (wraps around).
    pub fn previous(&mut self) {
        if self.workspaces.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let prev_idx = if idx == 0 {
                self.workspaces.len() - 1
            } else {
                idx - 1
            };
            self.active_id = Some(self.workspaces[prev_idx].id.clone());
        }
    }

    /// Returns the index of the active workspace.
    fn active_index(&self) -> Option<usize> {
        let id = self.active_id.as_deref()?;
        self.workspaces.iter().position(|w| w.id == id)
    }
}

impl Default for WorkspaceCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_first_becomes_active() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        assert_eq!(col.active_id(), Some("w1"));
        assert_eq!(col.count(), 1);
    }

    #[test]
    fn test_add_second_does_not_change_active() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.add("w2".into(), "Workspace 2".into(), "t2".into());
        assert_eq!(col.active_id(), Some("w1"));
        assert_eq!(col.count(), 2);
    }

    #[test]
    fn test_remove_active_picks_next() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.add("w2".into(), "Workspace 2".into(), "t2".into());
        col.add("w3".into(), "Workspace 3".into(), "t3".into());

        // Active is w1 (first added). Remove it -> w2 should become active (same index 0).
        col.remove("w1");
        assert_eq!(col.active_id(), Some("w2"));
        assert_eq!(col.count(), 2);

        // Remove w2 (active) -> w3 should become active.
        col.remove("w2");
        assert_eq!(col.active_id(), Some("w3"));
        assert_eq!(col.count(), 1);

        // Remove last workspace -> no active.
        col.remove("w3");
        assert_eq!(col.active_id(), None);
        assert_eq!(col.count(), 0);
    }

    #[test]
    fn test_remove_last_active_picks_previous() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.add("w2".into(), "Workspace 2".into(), "t2".into());
        col.add("w3".into(), "Workspace 3".into(), "t3".into());

        // Make w3 active, then remove it -> should pick w2 (previous).
        col.set_active("w3");
        assert_eq!(col.active_id(), Some("w3"));
        col.remove("w3");
        assert_eq!(col.active_id(), Some("w2"));
    }

    #[test]
    fn test_remove_non_active_preserves_active() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.add("w2".into(), "Workspace 2".into(), "t2".into());
        col.add("w3".into(), "Workspace 3".into(), "t3".into());

        // Active is w1. Remove w2 -> active should still be w1.
        col.remove("w2");
        assert_eq!(col.active_id(), Some("w1"));
        assert_eq!(col.count(), 2);
    }

    #[test]
    fn test_set_active_nonexistent_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.set_active("nonexistent");
        assert_eq!(col.active_id(), Some("w1"));
    }

    #[test]
    fn test_next_wraps_around() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.add("w2".into(), "Workspace 2".into(), "t2".into());
        col.add("w3".into(), "Workspace 3".into(), "t3".into());

        assert_eq!(col.active_id(), Some("w1"));

        col.next();
        assert_eq!(col.active_id(), Some("w2"));

        col.next();
        assert_eq!(col.active_id(), Some("w3"));

        // Wraps around to w1.
        col.next();
        assert_eq!(col.active_id(), Some("w1"));
    }

    #[test]
    fn test_previous_wraps_around() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        col.add("w2".into(), "Workspace 2".into(), "t2".into());
        col.add("w3".into(), "Workspace 3".into(), "t3".into());

        assert_eq!(col.active_id(), Some("w1"));

        // Wraps around to w3.
        col.previous();
        assert_eq!(col.active_id(), Some("w3"));

        col.previous();
        assert_eq!(col.active_id(), Some("w2"));

        col.previous();
        assert_eq!(col.active_id(), Some("w1"));
    }

    #[test]
    fn test_next_single_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());

        col.next();
        assert_eq!(col.active_id(), Some("w1"));
    }

    #[test]
    fn test_previous_single_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());

        col.previous();
        assert_eq!(col.active_id(), Some("w1"));
    }

    #[test]
    fn test_initial_layout_is_single_leaf() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());

        let ws = col.get("w1").unwrap();
        assert_eq!(ws.layout.leaf_count(), 1);
        assert!(ws.layout.find_leaf("t1"));
        assert_eq!(ws.layout.all_leaf_ids(), vec!["t1"]);
    }

    #[test]
    fn test_focused_terminal_set_on_add() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "term-abc".into());

        let ws = col.get("w1").unwrap();
        assert_eq!(ws.focused_terminal, "term-abc");
    }

    #[test]
    fn test_active_and_active_mut() {
        let mut col = WorkspaceCollection::new();
        assert!(col.active().is_none());
        assert!(col.active_mut().is_none());

        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        assert_eq!(col.active().unwrap().id, "w1");
        assert_eq!(col.active().unwrap().name, "Workspace 1");

        col.active_mut().unwrap().name = "Renamed".into();
        assert_eq!(col.active().unwrap().name, "Renamed");
    }

    #[test]
    fn test_get_and_get_mut() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Alpha".into(), "t1".into());
        col.add("w2".into(), "Beta".into(), "t2".into());

        assert_eq!(col.get("w1").unwrap().name, "Alpha");
        assert_eq!(col.get("w2").unwrap().name, "Beta");
        assert!(col.get("nonexistent").is_none());

        col.get_mut("w1").unwrap().name = "Alpha Prime".into();
        assert_eq!(col.get("w1").unwrap().name, "Alpha Prime");
        assert!(col.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_iter() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "A".into(), "t1".into());
        col.add("w2".into(), "B".into(), "t2".into());
        col.add("w3".into(), "C".into(), "t3".into());

        let ids: Vec<&str> = col.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w1", "w2", "w3"]);
    }

    #[test]
    fn test_default_is_empty() {
        let col = WorkspaceCollection::default();
        assert_eq!(col.count(), 0);
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_next_empty_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.next();
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_previous_empty_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.previous();
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_next_previous_round_trip() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "A".into(), "t1".into());
        col.add("w2".into(), "B".into(), "t2".into());
        col.add("w3".into(), "C".into(), "t3".into());

        col.set_active("w2");
        assert_eq!(col.active_id(), Some("w2"));

        col.next();
        assert_eq!(col.active_id(), Some("w3"));

        col.previous();
        assert_eq!(col.active_id(), Some("w2"));
    }

    #[test]
    fn test_remove_nonexistent_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "A".into(), "t1".into());
        col.remove("nonexistent");
        assert_eq!(col.count(), 1);
        assert_eq!(col.active_id(), Some("w1"));
    }
}
