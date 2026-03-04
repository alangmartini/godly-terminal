/// Generic workspace metadata.
///
/// `L` is the layout payload owned by the frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo<L> {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub worktree_mode: bool,
    pub layout: L,
    pub focused_terminal: String,
}

/// Deterministic state machine for workspace ordering and active selection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceCollection<L> {
    workspaces: Vec<WorkspaceInfo<L>>,
    active_id: Option<String>,
}

impl<L> WorkspaceCollection<L> {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            active_id: None,
        }
    }

    /// Add a new workspace.
    ///
    /// Returns a mutable reference to the inserted workspace.
    pub fn add(&mut self, workspace: WorkspaceInfo<L>) -> &mut WorkspaceInfo<L> {
        let is_first = self.workspaces.is_empty();
        let id = workspace.id.clone();
        self.workspaces.push(workspace);
        if is_first {
            self.active_id = Some(id);
        }
        self.workspaces
            .last_mut()
            .expect("workspace list has at least one element after push")
    }

    /// Remove a workspace by id.
    ///
    /// Returns `true` if the workspace existed and was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(idx) = self.workspaces.iter().position(|w| w.id == id) else {
            return false;
        };

        let was_active = self.active_id.as_deref() == Some(id);
        self.workspaces.remove(idx);

        if was_active {
            if self.workspaces.is_empty() {
                self.active_id = None;
            } else {
                let new_idx = if idx < self.workspaces.len() {
                    idx
                } else {
                    self.workspaces.len() - 1
                };
                self.active_id = Some(self.workspaces[new_idx].id.clone());
            }
        }

        true
    }

    /// Number of workspaces.
    pub fn count(&self) -> usize {
        self.workspaces.len()
    }

    /// Active workspace ID.
    pub fn active_id(&self) -> Option<&str> {
        self.active_id.as_deref()
    }

    /// Active workspace reference.
    pub fn active(&self) -> Option<&WorkspaceInfo<L>> {
        let id = self.active_id.as_deref()?;
        self.workspaces.iter().find(|w| w.id == id)
    }

    /// Active workspace mutable reference.
    pub fn active_mut(&mut self) -> Option<&mut WorkspaceInfo<L>> {
        let id = self.active_id.as_deref()?.to_owned();
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    /// Get workspace by id.
    pub fn get(&self, id: &str) -> Option<&WorkspaceInfo<L>> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    /// Get workspace by id mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkspaceInfo<L>> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    /// Set active workspace by id.
    ///
    /// Returns `true` when id exists and active changed.
    pub fn set_active(&mut self, id: &str) -> bool {
        if self.workspaces.iter().any(|w| w.id == id) {
            self.active_id = Some(id.to_owned());
            true
        } else {
            false
        }
    }

    /// Rename workspace by id.
    pub fn rename(&mut self, id: &str, name: String) -> bool {
        let Some(workspace) = self.workspaces.iter_mut().find(|w| w.id == id) else {
            return false;
        };
        workspace.name = name;
        true
    }

    /// Enable/disable worktree mode by id.
    pub fn set_worktree_mode(&mut self, id: &str, enabled: bool) -> bool {
        let Some(workspace) = self.workspaces.iter_mut().find(|w| w.id == id) else {
            return false;
        };
        workspace.worktree_mode = enabled;
        true
    }

    /// Move workspace one slot up in display order.
    pub fn move_up(&mut self, id: &str) -> bool {
        let Some(idx) = self.workspaces.iter().position(|w| w.id == id) else {
            return false;
        };
        if idx == 0 {
            return false;
        }
        self.workspaces.swap(idx - 1, idx);
        true
    }

    /// Move workspace one slot down in display order.
    pub fn move_down(&mut self, id: &str) -> bool {
        let Some(idx) = self.workspaces.iter().position(|w| w.id == id) else {
            return false;
        };
        if idx + 1 >= self.workspaces.len() {
            return false;
        }
        self.workspaces.swap(idx, idx + 1);
        true
    }

    /// Iterate workspaces in display order.
    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceInfo<L>> {
        self.workspaces.iter()
    }

    /// Workspaces as a slice in display order.
    pub fn as_slice(&self) -> &[WorkspaceInfo<L>] {
        &self.workspaces
    }

    /// Activate next workspace (wrap-around).
    pub fn next(&mut self) {
        if self.workspaces.len() <= 1 {
            return;
        }
        if let Some(idx) = self.active_index() {
            let next_idx = (idx + 1) % self.workspaces.len();
            self.active_id = Some(self.workspaces[next_idx].id.clone());
        }
    }

    /// Activate previous workspace (wrap-around).
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

    fn active_index(&self) -> Option<usize> {
        let id = self.active_id.as_deref()?;
        self.workspaces.iter().position(|w| w.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceCollection, WorkspaceInfo};

    fn ws(id: &str, name: &str, focused_terminal: &str) -> WorkspaceInfo<&'static str> {
        WorkspaceInfo {
            id: id.into(),
            name: name.into(),
            folder_path: ".".into(),
            worktree_mode: false,
            layout: "layout",
            focused_terminal: focused_terminal.into(),
        }
    }

    #[test]
    fn add_first_sets_active() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "Workspace 1", "t1"));
        assert_eq!(col.active_id(), Some("w1"));
        assert_eq!(col.count(), 1);
    }

    #[test]
    fn remove_active_prefers_same_index() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "A", "t1"));
        col.add(ws("w2", "B", "t2"));
        col.add(ws("w3", "C", "t3"));

        assert!(col.remove("w1"));
        assert_eq!(col.active_id(), Some("w2"));
        assert!(col.remove("w2"));
        assert_eq!(col.active_id(), Some("w3"));
    }

    #[test]
    fn remove_last_active_prefers_previous() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "A", "t1"));
        col.add(ws("w2", "B", "t2"));
        col.add(ws("w3", "C", "t3"));
        col.set_active("w3");

        assert!(col.remove("w3"));
        assert_eq!(col.active_id(), Some("w2"));
    }

    #[test]
    fn set_active_unknown_is_noop() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "A", "t1"));
        assert!(!col.set_active("missing"));
        assert_eq!(col.active_id(), Some("w1"));
    }

    #[test]
    fn next_previous_wrap() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "A", "t1"));
        col.add(ws("w2", "B", "t2"));
        col.add(ws("w3", "C", "t3"));

        col.next();
        assert_eq!(col.active_id(), Some("w2"));
        col.next();
        assert_eq!(col.active_id(), Some("w3"));
        col.next();
        assert_eq!(col.active_id(), Some("w1"));

        col.previous();
        assert_eq!(col.active_id(), Some("w3"));
    }

    #[test]
    fn rename_and_worktree_mode() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "A", "t1"));

        assert!(col.rename("w1", "Renamed".into()));
        assert_eq!(col.get("w1").expect("workspace").name, "Renamed");
        assert!(col.set_worktree_mode("w1", true));
        assert!(col.get("w1").expect("workspace").worktree_mode);
    }

    #[test]
    fn move_up_down_changes_order() {
        let mut col = WorkspaceCollection::new();
        col.add(ws("w1", "A", "t1"));
        col.add(ws("w2", "B", "t2"));
        col.add(ws("w3", "C", "t3"));

        assert!(col.move_up("w3"));
        let ids: Vec<&str> = col.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w1", "w3", "w2"]);

        assert!(col.move_down("w1"));
        let ids: Vec<&str> = col.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w3", "w1", "w2"]);
    }
}
