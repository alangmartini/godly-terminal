use godly_protocol::{LayoutNode, SplitDirection};
use parking_lot::RwLock;
use std::collections::HashMap;

#[allow(deprecated)]
use super::models::{SessionMetadata, SplitView, Terminal, Workspace};

#[allow(deprecated)]
pub struct AppState {
    pub workspaces: RwLock<HashMap<String, Workspace>>,
    pub terminals: RwLock<HashMap<String, Terminal>>,
    /// Session metadata for persistence (shell_type, cwd) - replaces direct PTY session access
    pub session_metadata: RwLock<HashMap<String, SessionMetadata>>,
    pub active_workspace_id: RwLock<Option<String>>,
    pub active_terminal_id: RwLock<Option<String>>,
    /// Per-terminal notification overrides (terminal_id → enabled)
    pub notification_overrides_terminal: RwLock<HashMap<String, bool>>,
    /// Per-workspace notification overrides (workspace_id → enabled)
    pub notification_overrides_workspace: RwLock<HashMap<String, bool>>,
    /// Split views per workspace (workspace_id → SplitView)
    #[deprecated(note = "Use layout_trees instead")]
    pub split_views: RwLock<HashMap<String, SplitView>>,
    /// Recursive layout trees per workspace (workspace_id → LayoutNode)
    pub layout_trees: RwLock<HashMap<String, LayoutNode>>,
    /// Zoomed pane per workspace (workspace_id → terminal_id)
    pub zoomed_panes: RwLock<HashMap<String, String>>,
    /// Workspace ID for MCP-created terminals (Agent workspace in separate window)
    pub mcp_workspace_id: RwLock<Option<String>>,
}

#[allow(deprecated)]
impl AppState {
    pub fn new() -> Self {
        Self {
            workspaces: RwLock::new(HashMap::new()),
            terminals: RwLock::new(HashMap::new()),
            session_metadata: RwLock::new(HashMap::new()),
            active_workspace_id: RwLock::new(None),
            active_terminal_id: RwLock::new(None),
            notification_overrides_terminal: RwLock::new(HashMap::new()),
            notification_overrides_workspace: RwLock::new(HashMap::new()),
            split_views: RwLock::new(HashMap::new()),
            layout_trees: RwLock::new(HashMap::new()),
            zoomed_panes: RwLock::new(HashMap::new()),
            mcp_workspace_id: RwLock::new(None),
        }
    }

    pub fn add_workspace(&self, workspace: Workspace) {
        let mut workspaces = self.workspaces.write();
        workspaces.insert(workspace.id.clone(), workspace);
    }

    pub fn remove_workspace(&self, id: &str) {
        let mut workspaces = self.workspaces.write();
        workspaces.remove(id);
    }

    pub fn get_workspace(&self, id: &str) -> Option<Workspace> {
        let workspaces = self.workspaces.read();
        workspaces.get(id).cloned()
    }

    pub fn get_all_workspaces(&self) -> Vec<Workspace> {
        let workspaces = self.workspaces.read();
        workspaces.values().cloned().collect()
    }

    pub fn add_terminal(&self, terminal: Terminal) {
        let mut terminals = self.terminals.write();
        terminals.insert(terminal.id.clone(), terminal);
    }

    pub fn remove_terminal(&self, id: &str) {
        let mut terminals = self.terminals.write();
        terminals.remove(id);
    }

    #[allow(dead_code)]
    pub fn get_terminal(&self, id: &str) -> Option<Terminal> {
        let terminals = self.terminals.read();
        terminals.get(id).cloned()
    }

    pub fn update_terminal_name(&self, id: &str, name: String) {
        let mut terminals = self.terminals.write();
        if let Some(terminal) = terminals.get_mut(id) {
            terminal.name = name;
        }
    }

    pub fn update_terminal_workspace(&self, id: &str, workspace_id: String) {
        let mut terminals = self.terminals.write();
        if let Some(terminal) = terminals.get_mut(id) {
            terminal.workspace_id = workspace_id;
        }
    }

    pub fn update_terminal_process(&self, id: &str, process_name: String) {
        let mut terminals = self.terminals.write();
        if let Some(terminal) = terminals.get_mut(id) {
            terminal.process_name = process_name;
        }
    }

    pub fn update_workspace_worktree_mode(&self, id: &str, worktree_mode: bool) {
        let mut workspaces = self.workspaces.write();
        if let Some(workspace) = workspaces.get_mut(id) {
            workspace.worktree_mode = worktree_mode;
        }
    }

    pub fn update_workspace_claude_code_mode(&self, id: &str, claude_code_mode: bool) {
        let mut workspaces = self.workspaces.write();
        if let Some(workspace) = workspaces.get_mut(id) {
            workspace.claude_code_mode = claude_code_mode;
        }
    }

    pub fn get_workspace_terminals(&self, workspace_id: &str) -> Vec<Terminal> {
        let terminals = self.terminals.read();
        terminals
            .values()
            .filter(|t| t.workspace_id == workspace_id)
            .cloned()
            .collect()
    }

    pub fn add_session_metadata(&self, id: String, metadata: SessionMetadata) {
        let mut meta = self.session_metadata.write();
        meta.insert(id, metadata);
    }

    pub fn remove_session_metadata(&self, id: &str) {
        let mut meta = self.session_metadata.write();
        meta.remove(id);
    }

    pub fn set_split_view(&self, workspace_id: &str, split_view: SplitView) {
        let mut views = self.split_views.write();
        views.insert(workspace_id.to_string(), split_view);
    }

    pub fn clear_split_view(&self, workspace_id: &str) {
        let mut views = self.split_views.write();
        views.remove(workspace_id);
    }

    pub fn get_all_split_views(&self) -> HashMap<String, SplitView> {
        self.split_views.read().clone()
    }

    pub fn set_layout_tree(&self, workspace_id: &str, tree: LayoutNode) {
        let mut trees = self.layout_trees.write();
        trees.insert(workspace_id.to_string(), tree);
    }

    pub fn get_layout_tree(&self, workspace_id: &str) -> Option<LayoutNode> {
        let trees = self.layout_trees.read();
        trees.get(workspace_id).cloned()
    }

    pub fn clear_layout_tree(&self, workspace_id: &str) {
        let mut trees = self.layout_trees.write();
        trees.remove(workspace_id);
    }

    pub fn get_all_layout_trees(&self) -> HashMap<String, LayoutNode> {
        self.layout_trees.read().clone()
    }

    /// Split a target terminal in the layout tree, inserting a new terminal next to it.
    /// If no layout tree exists for the workspace, creates a 2-leaf tree.
    /// Returns Ok(()) on success or Err with a message.
    pub fn split_terminal_in_tree(
        &self,
        workspace_id: &str,
        target_terminal_id: &str,
        new_terminal_id: &str,
        direction: SplitDirection,
        ratio: f64,
    ) -> Result<(), String> {
        let mut trees = self.layout_trees.write();
        let ratio = ratio.clamp(0.15, 0.85);

        if let Some(tree) = trees.get_mut(workspace_id) {
            // Insert into existing tree: find the target leaf and replace it with a split
            if !Self::insert_split(tree, target_terminal_id, new_terminal_id, direction, ratio) {
                return Err(format!(
                    "Terminal {} not found in layout tree for workspace {}",
                    target_terminal_id, workspace_id
                ));
            }
        } else {
            // No tree exists -- create a 2-leaf tree
            let tree = LayoutNode::Split {
                direction,
                ratio,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: target_terminal_id.to_string(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: new_terminal_id.to_string(),
                }),
            };
            trees.insert(workspace_id.to_string(), tree);
        }

        Ok(())
    }

    /// Remove a terminal from the layout tree. The sibling takes its parent's place.
    /// Returns Ok(()) on success, Err if terminal not found.
    pub fn unsplit_terminal_in_tree(
        &self,
        workspace_id: &str,
        terminal_id: &str,
    ) -> Result<(), String> {
        let mut trees = self.layout_trees.write();

        let tree = trees.get_mut(workspace_id).ok_or_else(|| {
            format!("No layout tree for workspace {}", workspace_id)
        })?;

        // If the tree is just a single leaf, remove the whole tree
        if let LayoutNode::Leaf { terminal_id: tid } = tree {
            if tid == terminal_id {
                trees.remove(workspace_id);
                return Ok(());
            }
            return Err(format!(
                "Terminal {} not found in layout tree",
                terminal_id
            ));
        }

        // Try to remove the terminal and collapse
        match Self::remove_from_tree(tree, terminal_id) {
            Some(replacement) => {
                // If replacement is a single leaf, we could keep or remove the tree
                *tree = replacement;
                // If the result is a single leaf, remove the tree entirely
                // (single-pane mode needs no tree)
                if matches!(tree, LayoutNode::Leaf { .. }) {
                    trees.remove(workspace_id);
                }
                Ok(())
            }
            None => Err(format!(
                "Terminal {} not found in layout tree for workspace {}",
                terminal_id, workspace_id
            )),
        }
    }

    /// Swap two terminals' positions in the layout tree.
    pub fn swap_panes_in_tree(
        &self,
        workspace_id: &str,
        terminal_id_a: &str,
        terminal_id_b: &str,
    ) -> Result<(), String> {
        let mut trees = self.layout_trees.write();
        let tree = trees.get_mut(workspace_id).ok_or_else(|| {
            format!("No layout tree for workspace {}", workspace_id)
        })?;

        Self::swap_terminals(tree, terminal_id_a, terminal_id_b);
        Ok(())
    }

    /// Set/clear zoomed pane for a workspace.
    pub fn set_zoomed_pane(&self, workspace_id: &str, terminal_id: Option<String>) {
        let mut zoomed = self.zoomed_panes.write();
        match terminal_id {
            Some(tid) => { zoomed.insert(workspace_id.to_string(), tid); }
            None => { zoomed.remove(workspace_id); }
        }
    }

    pub fn get_zoomed_pane(&self, workspace_id: &str) -> Option<String> {
        self.zoomed_panes.read().get(workspace_id).cloned()
    }

    // --- Private tree helpers ---

    /// Recursively find a leaf with `target_id` and replace it with a split
    /// containing both the original and the new terminal.
    fn insert_split(
        node: &mut LayoutNode,
        target_id: &str,
        new_id: &str,
        direction: SplitDirection,
        ratio: f64,
    ) -> bool {
        match node {
            LayoutNode::Leaf { terminal_id } if terminal_id == target_id => {
                // Replace this leaf with a split
                let old_leaf = LayoutNode::Leaf {
                    terminal_id: target_id.to_string(),
                };
                let new_leaf = LayoutNode::Leaf {
                    terminal_id: new_id.to_string(),
                };
                *node = LayoutNode::Split {
                    direction,
                    ratio,
                    first: Box::new(old_leaf),
                    second: Box::new(new_leaf),
                };
                true
            }
            LayoutNode::Split { first, second, .. } => {
                Self::insert_split(first, target_id, new_id, direction, ratio)
                    || Self::insert_split(second, target_id, new_id, direction, ratio)
            }
            _ => false,
        }
    }

    /// Remove a terminal from the tree. Returns the replacement subtree (the sibling)
    /// or None if the terminal was not found.
    fn remove_from_tree(node: &mut LayoutNode, terminal_id: &str) -> Option<LayoutNode> {
        match node {
            LayoutNode::Leaf { .. } => None, // Can't remove from a leaf at this level
            LayoutNode::Split { first, second, .. } => {
                // Check if either child is the target leaf
                if let LayoutNode::Leaf { terminal_id: tid } = first.as_ref() {
                    if tid == terminal_id {
                        return Some(*second.clone());
                    }
                }
                if let LayoutNode::Leaf { terminal_id: tid } = second.as_ref() {
                    if tid == terminal_id {
                        return Some(*first.clone());
                    }
                }

                // Recurse into children
                if let Some(replacement) = Self::remove_from_tree(first, terminal_id) {
                    *first = Box::new(replacement);
                    return Some(node.clone());
                }
                if let Some(replacement) = Self::remove_from_tree(second, terminal_id) {
                    *second = Box::new(replacement);
                    return Some(node.clone());
                }

                None
            }
        }
    }

    /// Swap two terminals in the tree by swapping their IDs in-place.
    fn swap_terminals(node: &mut LayoutNode, id_a: &str, id_b: &str) {
        match node {
            LayoutNode::Leaf { terminal_id } => {
                if terminal_id == id_a {
                    *terminal_id = id_b.to_string();
                } else if terminal_id == id_b {
                    *terminal_id = id_a.to_string();
                }
            }
            LayoutNode::Split { first, second, .. } => {
                Self::swap_terminals(first, id_a, id_b);
                Self::swap_terminals(second, id_a, id_b);
            }
        }
    }

    /// Check if notifications are enabled for a given terminal/workspace.
    /// Priority: per-terminal override > per-workspace override > global default (true).
    /// Returns (enabled, source_description).
    pub fn is_notification_enabled(
        &self,
        terminal_id: Option<&str>,
        workspace_id: Option<&str>,
    ) -> (bool, &'static str) {
        if let Some(tid) = terminal_id {
            let overrides = self.notification_overrides_terminal.read();
            if let Some(&enabled) = overrides.get(tid) {
                return (enabled, "terminal");
            }
        }
        if let Some(wid) = workspace_id {
            let overrides = self.notification_overrides_workspace.read();
            if let Some(&enabled) = overrides.get(wid) {
                return (enabled, "workspace");
            }
        }
        (true, "global")
    }

    pub fn set_notification_enabled_terminal(&self, terminal_id: &str, enabled: bool) {
        let mut overrides = self.notification_overrides_terminal.write();
        overrides.insert(terminal_id.to_string(), enabled);
    }

    pub fn set_notification_enabled_workspace(&self, workspace_id: &str, enabled: bool) {
        let mut overrides = self.notification_overrides_workspace.write();
        overrides.insert(workspace_id.to_string(), enabled);
    }

    pub fn set_active_terminal_id(&self, id: Option<String>) {
        *self.active_terminal_id.write() = id;
    }

    pub fn get_active_terminal_id(&self) -> Option<String> {
        self.active_terminal_id.read().clone()
    }
}

#[allow(deprecated)]
impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::models::ShellType;

    #[test]
    fn test_workspace_add_and_get() {
        let state = AppState::new();

        let workspace = Workspace {
            id: "ws-123".to_string(),
            name: "Test Workspace".to_string(),
            folder_path: "C:\\Test".to_string(),
            tab_order: vec![],
            shell_type: ShellType::Windows,
            worktree_mode: false,
            claude_code_mode: false,
        };

        state.add_workspace(workspace.clone());

        let retrieved = state.get_workspace("ws-123");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "ws-123");
    }

    #[test]
    fn test_workspace_restore_preserves_id() {
        let state = AppState::new();

        let original_id = "original-workspace-id-abc123";
        let workspace = Workspace {
            id: original_id.to_string(),
            name: "Restored Workspace".to_string(),
            folder_path: "C:\\Projects".to_string(),
            tab_order: vec!["term-1".to_string()],
            shell_type: ShellType::Windows,
            worktree_mode: false,
            claude_code_mode: false,
        };

        state.add_workspace(workspace);

        let retrieved = state.get_workspace(original_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, original_id);
    }

    #[test]
    fn test_terminal_workspace_relationship() {
        let state = AppState::new();

        state.add_workspace(Workspace {
            id: "ws-1".to_string(),
            name: "Workspace".to_string(),
            folder_path: "C:\\".to_string(),
            tab_order: vec![],
            shell_type: ShellType::Windows,
            worktree_mode: false,
            claude_code_mode: false,
        });

        state.add_terminal(Terminal {
            id: "term-1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "Terminal".to_string(),
            process_name: "powershell".to_string(),
        });

        let terminals = state.get_workspace_terminals("ws-1");
        assert_eq!(terminals.len(), 1);
        assert_eq!(terminals[0].id, "term-1");
    }

    #[test]
    fn test_restore_multiple_workspaces() {
        let state = AppState::new();

        let workspaces = vec![
            Workspace {
                id: "ws-1".to_string(),
                name: "Project A".to_string(),
                folder_path: "C:\\ProjectA".to_string(),
                tab_order: vec![],
                shell_type: ShellType::Windows,
                worktree_mode: false,
                claude_code_mode: false,
            },
            Workspace {
                id: "ws-2".to_string(),
                name: "Project B".to_string(),
                folder_path: "/home/user/projectb".to_string(),
                tab_order: vec![],
                shell_type: ShellType::Wsl {
                    distribution: Some("Ubuntu".to_string()),
                },
                worktree_mode: false,
                claude_code_mode: false,
            },
        ];

        for ws in workspaces {
            state.add_workspace(ws);
        }

        assert_eq!(state.get_all_workspaces().len(), 2);
        assert!(state.get_workspace("ws-1").is_some());
        assert!(state.get_workspace("ws-2").is_some());
    }

    #[test]
    fn test_active_workspace_id_restore() {
        let state = AppState::new();

        *state.active_workspace_id.write() = Some("ws-active".to_string());

        let active = state.active_workspace_id.read().clone();
        assert_eq!(active, Some("ws-active".to_string()));
    }

    #[test]
    fn test_terminal_with_preserved_id() {
        let state = AppState::new();

        let preserved_id = "preserved-terminal-id-xyz";
        state.add_terminal(Terminal {
            id: preserved_id.to_string(),
            workspace_id: "ws-1".to_string(),
            name: "Restored Terminal".to_string(),
            process_name: "powershell".to_string(),
        });

        let terminal = state.get_terminal(preserved_id);
        assert!(terminal.is_some());
        assert_eq!(terminal.unwrap().id, preserved_id);
    }

    #[test]
    fn test_session_metadata() {
        let state = AppState::new();

        state.add_session_metadata(
            "term-1".to_string(),
            SessionMetadata {
                shell_type: ShellType::Windows,
                cwd: Some("C:\\Users\\test".to_string()),
                worktree_path: None,
                worktree_branch: None,
            },
        );

        let meta = state.session_metadata.read();
        let m = meta.get("term-1").unwrap();
        assert_eq!(m.shell_type, ShellType::Windows);
        assert_eq!(m.cwd, Some("C:\\Users\\test".to_string()));
    }
}
