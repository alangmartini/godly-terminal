use parking_lot::RwLock;
use std::collections::HashMap;

use super::models::{Terminal, Workspace};
use crate::pty::manager::PtySession;

pub struct AppState {
    pub workspaces: RwLock<HashMap<String, Workspace>>,
    pub terminals: RwLock<HashMap<String, Terminal>>,
    pub pty_sessions: RwLock<HashMap<String, PtySession>>,
    pub active_workspace_id: RwLock<Option<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspaces: RwLock::new(HashMap::new()),
            terminals: RwLock::new(HashMap::new()),
            pty_sessions: RwLock::new(HashMap::new()),
            active_workspace_id: RwLock::new(None),
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

    pub fn get_workspace_terminals(&self, workspace_id: &str) -> Vec<Terminal> {
        let terminals = self.terminals.read();
        terminals
            .values()
            .filter(|t| t.workspace_id == workspace_id)
            .cloned()
            .collect()
    }

    pub fn add_pty_session(&self, id: String, session: PtySession) {
        let mut sessions = self.pty_sessions.write();
        sessions.insert(id, session);
    }

    pub fn remove_pty_session(&self, id: &str) -> Option<PtySession> {
        let mut sessions = self.pty_sessions.write();
        sessions.remove(id)
    }

    #[allow(dead_code)]
    pub fn get_pty_session(&self, id: &str) -> Option<PtySession> {
        let sessions = self.pty_sessions.read();
        sessions.get(id).cloned()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
