use godly_workspaces_core as workspaces_core;

use crate::split_pane::LayoutNode;

/// Workspace metadata used by the native shell.
pub type WorkspaceInfo = workspaces_core::WorkspaceInfo<LayoutNode>;

/// Collection of workspaces with active workspace tracking.
///
/// Ordering and active-selection behavior are delegated to
/// `godly-workspaces-core` for deterministic unit testing.
pub struct WorkspaceCollection {
    inner: workspaces_core::WorkspaceCollection<LayoutNode>,
}

impl WorkspaceCollection {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self {
            inner: workspaces_core::WorkspaceCollection::new(),
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
        let folder_path = std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        let focused = initial_terminal_id.clone();
        let workspace = WorkspaceInfo {
            id,
            name,
            folder_path,
            worktree_mode: false,
            layout: LayoutNode::Leaf {
                terminal_id: initial_terminal_id,
            },
            focused_terminal: focused,
        };
        self.inner.add(workspace)
    }

    /// Removes the workspace with the given id.
    pub fn remove(&mut self, id: &str) {
        let _ = self.inner.remove(id);
    }

    /// Returns a reference to the active workspace, if any.
    pub fn active(&self) -> Option<&WorkspaceInfo> {
        self.inner.active()
    }

    /// Returns a mutable reference to the active workspace, if any.
    pub fn active_mut(&mut self) -> Option<&mut WorkspaceInfo> {
        self.inner.active_mut()
    }

    /// Finds a workspace by id.
    pub fn get(&self, id: &str) -> Option<&WorkspaceInfo> {
        self.inner.get(id)
    }

    /// Finds a workspace by id, mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkspaceInfo> {
        self.inner.get_mut(id)
    }

    /// Sets the active workspace by id. No-op if id not found.
    pub fn set_active(&mut self, id: &str) {
        let _ = self.inner.set_active(id);
    }

    /// Renames a workspace by id. Returns whether the workspace was found.
    pub fn rename(&mut self, id: &str, name: String) -> bool {
        self.inner.rename(id, name)
    }

    /// Toggles worktree mode by id. Returns whether the workspace was found.
    pub fn set_worktree_mode(&mut self, id: &str, enabled: bool) -> bool {
        self.inner.set_worktree_mode(id, enabled)
    }

    /// Move a workspace one slot up in display order.
    pub fn move_up(&mut self, id: &str) -> bool {
        self.inner.move_up(id)
    }

    /// Move a workspace one slot down in display order.
    pub fn move_down(&mut self, id: &str) -> bool {
        self.inner.move_down(id)
    }

    /// Returns the number of workspaces in the collection.
    pub fn count(&self) -> usize {
        self.inner.count()
    }

    /// Iterates over all workspaces.
    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceInfo> {
        self.inner.iter()
    }

    /// Returns the workspaces as a slice.
    pub fn as_slice(&self) -> &[WorkspaceInfo] {
        self.inner.as_slice()
    }

    /// Returns the active workspace's id, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.inner.active_id()
    }

    /// Switch to the next workspace (wraps around).
    pub fn next(&mut self) {
        self.inner.next();
    }

    /// Switch to the previous workspace (wraps around).
    pub fn previous(&mut self) {
        self.inner.previous();
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

        col.remove("w1");
        assert_eq!(col.active_id(), Some("w2"));
        assert_eq!(col.count(), 2);

        col.remove("w2");
        assert_eq!(col.active_id(), Some("w3"));
        assert_eq!(col.count(), 1);

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

        let ws = col.get("w1").expect("workspace must exist");
        assert_eq!(ws.layout.leaf_count(), 1);
        assert!(ws.layout.find_leaf("t1"));
        assert_eq!(ws.layout.all_leaf_ids(), vec!["t1"]);
    }

    #[test]
    fn test_focused_terminal_set_on_add() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "term-abc".into());

        let ws = col.get("w1").expect("workspace must exist");
        assert_eq!(ws.focused_terminal, "term-abc");
    }

    #[test]
    fn test_active_and_active_mut() {
        let mut col = WorkspaceCollection::new();
        assert!(col.active().is_none());
        assert!(col.active_mut().is_none());

        col.add("w1".into(), "Workspace 1".into(), "t1".into());
        assert_eq!(col.active().expect("active workspace").id, "w1");
        assert_eq!(col.active().expect("active workspace").name, "Workspace 1");

        col.active_mut().expect("active workspace").name = "Renamed".into();
        assert_eq!(col.active().expect("active workspace").name, "Renamed");
    }

    #[test]
    fn test_get_and_get_mut() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Alpha".into(), "t1".into());
        col.add("w2".into(), "Beta".into(), "t2".into());

        assert_eq!(col.get("w1").expect("workspace").name, "Alpha");
        assert_eq!(col.get("w2").expect("workspace").name, "Beta");
        assert!(col.get("nonexistent").is_none());

        col.get_mut("w1").expect("workspace").name = "Alpha Prime".into();
        assert_eq!(col.get("w1").expect("workspace").name, "Alpha Prime");
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

    #[test]
    fn test_rename_workspace() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());

        assert!(col.rename("w1", "Renamed Workspace".into()));
        assert_eq!(col.get("w1").expect("workspace").name, "Renamed Workspace");
        assert!(!col.rename("missing", "Nope".into()));
    }

    #[test]
    fn test_set_worktree_mode() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "Workspace 1".into(), "t1".into());

        assert!(!col.get("w1").expect("workspace").worktree_mode);
        assert!(col.set_worktree_mode("w1", true));
        assert!(col.get("w1").expect("workspace").worktree_mode);
        assert!(!col.set_worktree_mode("missing", true));
    }

    #[test]
    fn test_move_up_down() {
        let mut col = WorkspaceCollection::new();
        col.add("w1".into(), "A".into(), "t1".into());
        col.add("w2".into(), "B".into(), "t2".into());
        col.add("w3".into(), "C".into(), "t3".into());

        assert!(!col.move_up("w1"));
        assert!(col.move_up("w3"));
        let ids: Vec<&str> = col.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w1", "w3", "w2"]);

        assert!(col.move_down("w1"));
        let ids: Vec<&str> = col.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w3", "w1", "w2"]);

        assert!(!col.move_down("w2"));
        assert!(!col.move_down("missing"));
    }
}
