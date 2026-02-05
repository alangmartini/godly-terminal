use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Terminal {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub process_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub tab_order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub workspaces: Vec<Workspace>,
    pub terminals: Vec<TerminalInfo>,
    pub active_workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            terminals: Vec::new(),
            active_workspace_id: None,
        }
    }
}
