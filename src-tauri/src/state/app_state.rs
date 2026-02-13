use parking_lot::RwLock;
use std::collections::HashMap;

use super::models::{SessionMetadata, SplitView, Terminal, Workspace};

pub struct AppState {
    pub workspaces: RwLock<HashMap<String, Workspace>>,
    pub terminals: RwLock<HashMap<String, Terminal>>,
    /// Session metadata for persistence (shell_type, cwd) - replaces direct PTY session access
    pub session_metadata: RwLock<HashMap<String, SessionMetadata>>,
    pub active_workspace_id: RwLock<Option<String>>,
    /// Per-terminal notification overrides (terminal_id → enabled)
    pub notification_overrides_terminal: RwLock<HashMap<String, bool>>,
    /// Per-workspace notification overrides (workspace_id → enabled)
    pub notification_overrides_workspace: RwLock<HashMap<String, bool>>,
    /// Split views per workspace (workspace_id → SplitView)
    pub split_views: RwLock<HashMap<String, SplitView>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspaces: RwLock::new(HashMap::new()),
            terminals: RwLock::new(HashMap::new()),
            session_metadata: RwLock::new(HashMap::new()),
            active_workspace_id: RwLock::new(None),
            notification_overrides_terminal: RwLock::new(HashMap::new()),
            notification_overrides_workspace: RwLock::new(HashMap::new()),
            split_views: RwLock::new(HashMap::new()),
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
}

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
