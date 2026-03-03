use godly_protocol::{LayoutNode, SplitDirection};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};

#[allow(deprecated)]
use super::models::{SessionMetadata, SplitView, Terminal, WindowState, Workspace};

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
    /// Window geometry and monitor for cross-session restoration
    pub window_state: RwLock<Option<WindowState>>,
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
            window_state: RwLock::new(None),
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

    pub fn update_workspace_name(&self, id: &str, name: String) {
        let mut workspaces = self.workspaces.write();
        if let Some(workspace) = workspaces.get_mut(id) {
            workspace.name = name;
        }
    }

    pub fn update_workspace_worktree_mode(&self, id: &str, worktree_mode: bool) {
        let mut workspaces = self.workspaces.write();
        if let Some(workspace) = workspaces.get_mut(id) {
            workspace.worktree_mode = worktree_mode;
        }
    }

    pub fn update_workspace_ai_tool_mode(&self, id: &str, ai_tool_mode: super::models::AiToolMode) {
        let mut workspaces = self.workspaces.write();
        if let Some(workspace) = workspaces.get_mut(id) {
            workspace.ai_tool_mode = ai_tool_mode;
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
        // Validate both terminal IDs exist
        let terminals = self.terminals.read();
        if !terminals.contains_key(&split_view.left_terminal_id)
            || !terminals.contains_key(&split_view.right_terminal_id)
        {
            eprintln!(
                "[app_state] set_split_view: skipping — one or both terminal IDs don't exist (left={}, right={})",
                split_view.left_terminal_id, split_view.right_terminal_id
            );
            return;
        }
        drop(terminals);
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

    /// Store a layout tree, pruning any leaf IDs not present in `self.terminals`.
    /// Discards the tree entirely if fewer than 2 leaves survive pruning.
    pub fn set_layout_tree_validated(&self, workspace_id: &str, tree: LayoutNode) {
        let terminals = self.terminals.read();
        let live_ids: HashSet<String> = terminals.keys().cloned().collect();
        drop(terminals);

        match tree.prune_stale_terminal_ids(&live_ids) {
            Some(pruned) if pruned.count_leaves() >= 2 => {
                let mut trees = self.layout_trees.write();
                trees.insert(workspace_id.to_string(), pruned);
            }
            _ => {
                eprintln!(
                    "[app_state] set_layout_tree_validated: tree for workspace {} pruned to <2 leaves, discarding",
                    workspace_id
                );
                let mut trees = self.layout_trees.write();
                trees.remove(workspace_id);
            }
        }
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

        let make_fresh_tree = || LayoutNode::Split {
            direction,
            ratio,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: target_terminal_id.to_string(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: new_terminal_id.to_string(),
            }),
        };

        if let Some(tree) = trees.get_mut(workspace_id) {
            // Insert into existing tree: find the target leaf and replace it with a split
            if !Self::insert_split(tree, target_terminal_id, new_terminal_id, direction, ratio) {
                // Target not in tree (stale tree from previous session) — replace with fresh 2-leaf tree
                eprintln!(
                    "[app_state] split_terminal_in_tree: terminal {} not found in stale tree for workspace {}, replacing tree",
                    target_terminal_id, workspace_id
                );
                *tree = make_fresh_tree();
            }
        } else {
            // No tree exists -- create a 2-leaf tree
            trees.insert(workspace_id.to_string(), make_fresh_tree());
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

    /// Set/clear zoomed pane for a workspace. Validates terminal exists when setting.
    pub fn set_zoomed_pane(&self, workspace_id: &str, terminal_id: Option<String>) {
        if let Some(ref tid) = terminal_id {
            if !self.terminals.read().contains_key(tid) {
                eprintln!(
                    "[app_state] set_zoomed_pane: terminal {} doesn't exist, ignoring",
                    tid
                );
                return;
            }
        }
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
            LayoutNode::Grid { .. } => None,
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
            LayoutNode::Grid { .. } => {}
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

    /// Prune all state structures that reference terminal IDs not in `live_ids`.
    /// Call after load_layout once terminal restoration is complete.
    pub fn prune_stale_ids(&self, live_ids: &HashSet<String>) {
        // Prune layout trees
        let mut trees = self.layout_trees.write();
        let ws_ids: Vec<String> = trees.keys().cloned().collect();
        for ws_id in ws_ids {
            if let Some(tree) = trees.get(&ws_id) {
                match tree.prune_stale_terminal_ids(live_ids) {
                    Some(pruned) => {
                        // If pruned to a single leaf, remove the tree (no split needed)
                        if pruned.count_leaves() <= 1 {
                            trees.remove(&ws_id);
                        } else {
                            trees.insert(ws_id, pruned);
                        }
                    }
                    None => {
                        trees.remove(&ws_id);
                    }
                }
            }
        }
        drop(trees);

        // Prune legacy split views
        let mut views = self.split_views.write();
        views.retain(|_, sv| {
            live_ids.contains(&sv.left_terminal_id) && live_ids.contains(&sv.right_terminal_id)
        });
        drop(views);

        // Prune zoomed panes
        let mut zoomed = self.zoomed_panes.write();
        zoomed.retain(|_, tid| live_ids.contains(tid));
        drop(zoomed);

        // Prune active terminal ID
        let mut active = self.active_terminal_id.write();
        if let Some(ref tid) = *active {
            if !live_ids.contains(tid) {
                *active = None;
            }
        }
        drop(active);

        // Prune workspace tab_orders
        let mut workspaces = self.workspaces.write();
        for ws in workspaces.values_mut() {
            ws.tab_order.retain(|tid| live_ids.contains(tid));
        }
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
    use crate::state::models::{AiToolMode, ShellType};

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
            ai_tool_mode: AiToolMode::None,
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
            ai_tool_mode: AiToolMode::None,
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
            ai_tool_mode: AiToolMode::None,
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
                ai_tool_mode: AiToolMode::None,
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
                ai_tool_mode: AiToolMode::None,
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

    // ---------------------------------------------------------------
    // prune_stale_ids
    // ---------------------------------------------------------------

    fn make_state_with_terminals(ids: &[&str]) -> AppState {
        let state = AppState::new();
        for id in ids {
            state.add_terminal(Terminal {
                id: id.to_string(),
                workspace_id: "ws-1".to_string(),
                name: "Terminal".to_string(),
                process_name: "powershell".to_string(),
            });
        }
        state
    }

    #[test]
    fn prune_removes_dead_layout_tree() {
        let state = make_state_with_terminals(&["t1"]);
        // Layout tree references t1 and t2, but only t1 is live
        state.set_layout_tree(
            "ws-1",
            LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t1".to_string(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".to_string(),
                }),
            },
        );

        let live: HashSet<String> = ["t1"].iter().map(|s| s.to_string()).collect();
        state.prune_stale_ids(&live);

        // Tree had 2 leaves, now only 1 is live → tree should be removed (single leaf = no split)
        assert!(state.get_layout_tree("ws-1").is_none());
    }

    #[test]
    fn prune_keeps_valid_layout_tree() {
        let state = make_state_with_terminals(&["t1", "t2"]);
        state.set_layout_tree(
            "ws-1",
            LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t1".to_string(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".to_string(),
                }),
            },
        );

        let live: HashSet<String> = ["t1", "t2"].iter().map(|s| s.to_string()).collect();
        state.prune_stale_ids(&live);

        let tree = state.get_layout_tree("ws-1");
        assert!(tree.is_some());
        assert_eq!(tree.unwrap().count_leaves(), 2);
    }

    #[test]
    fn prune_removes_dead_zoomed_pane() {
        let state = make_state_with_terminals(&["t1"]);
        state.zoomed_panes
            .write()
            .insert("ws-1".to_string(), "t2".to_string());

        let live: HashSet<String> = ["t1"].iter().map(|s| s.to_string()).collect();
        state.prune_stale_ids(&live);

        assert!(state.get_zoomed_pane("ws-1").is_none());
    }

    #[test]
    fn prune_clears_dead_active_terminal() {
        let state = make_state_with_terminals(&["t1"]);
        state.set_active_terminal_id(Some("t_dead".to_string()));

        let live: HashSet<String> = ["t1"].iter().map(|s| s.to_string()).collect();
        state.prune_stale_ids(&live);

        assert_eq!(state.get_active_terminal_id(), None);
    }

    #[test]
    fn prune_filters_tab_order() {
        let state = AppState::new();
        state.add_workspace(Workspace {
            id: "ws-1".to_string(),
            name: "Test".to_string(),
            folder_path: "C:\\".to_string(),
            tab_order: vec!["t1".to_string(), "t_dead".to_string(), "t2".to_string()],
            shell_type: ShellType::Windows,
            worktree_mode: false,
            ai_tool_mode: AiToolMode::None,
        });
        state.add_terminal(Terminal {
            id: "t1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "T".to_string(),
            process_name: "p".to_string(),
        });
        state.add_terminal(Terminal {
            id: "t2".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "T".to_string(),
            process_name: "p".to_string(),
        });

        let live: HashSet<String> = ["t1", "t2"].iter().map(|s| s.to_string()).collect();
        state.prune_stale_ids(&live);

        let ws = state.get_workspace("ws-1").unwrap();
        assert_eq!(ws.tab_order, vec!["t1", "t2"]);
    }

    // ---------------------------------------------------------------
    // split_terminal_in_tree — stale tree fallback (Issue #444)
    // ---------------------------------------------------------------

    #[test]
    fn split_in_stale_tree_replaces_with_fresh_tree() {
        let state = make_state_with_terminals(&["t_new", "t_target"]);
        // Stale tree with old IDs that don't include t_target
        state.set_layout_tree(
            "ws-1",
            LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "old_t1".to_string(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "old_t2".to_string(),
                }),
            },
        );

        // Bug: this used to return Err because t_target isn't in the stale tree
        let result = state.split_terminal_in_tree(
            "ws-1",
            "t_target",
            "t_new",
            SplitDirection::Horizontal,
            0.5,
        );
        assert!(result.is_ok());

        // Should have replaced the stale tree with a fresh 2-leaf tree
        let tree = state.get_layout_tree("ws-1").unwrap();
        assert_eq!(tree.count_leaves(), 2);
        assert!(tree.find_terminal("t_target"));
        assert!(tree.find_terminal("t_new"));
    }

    #[test]
    fn split_in_valid_tree_still_works() {
        let state = make_state_with_terminals(&["t1", "t2"]);
        state.set_layout_tree(
            "ws-1",
            LayoutNode::Leaf {
                terminal_id: "t1".to_string(),
            },
        );

        let result = state.split_terminal_in_tree(
            "ws-1",
            "t1",
            "t2",
            SplitDirection::Vertical,
            0.5,
        );
        assert!(result.is_ok());

        let tree = state.get_layout_tree("ws-1").unwrap();
        assert_eq!(tree.count_leaves(), 2);
        assert!(tree.find_terminal("t1"));
        assert!(tree.find_terminal("t2"));
    }
}
