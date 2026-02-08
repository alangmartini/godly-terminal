use serde::{Deserialize, Serialize};

use crate::types::ShellType;

/// Requests sent from godly-mcp binary to the Tauri app via MCP pipe
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpRequest {
    Ping,

    // Terminal queries
    ListTerminals,
    GetTerminal { terminal_id: String },
    GetCurrentSession { session_id: String },

    // Terminal mutations
    CreateTerminal {
        workspace_id: String,
        #[serde(default)]
        shell_type: Option<ShellType>,
        #[serde(default)]
        cwd: Option<String>,
    },
    CloseTerminal { terminal_id: String },
    RenameTerminal { terminal_id: String, name: String },
    FocusTerminal { terminal_id: String },

    // Workspace queries/mutations
    ListWorkspaces,
    CreateWorkspace { name: String, folder_path: String },
    SwitchWorkspace { workspace_id: String },
    MoveTerminalToWorkspace {
        terminal_id: String,
        workspace_id: String,
    },

    // Terminal I/O
    WriteToTerminal { terminal_id: String, data: String },
}

/// Terminal info returned by MCP queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTerminalInfo {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub process_name: String,
}

/// Workspace info returned by MCP queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpWorkspaceInfo {
    pub id: String,
    pub name: String,
    pub folder_path: String,
}

/// Responses sent from the Tauri app to godly-mcp binary via MCP pipe
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpResponse {
    Ok,
    Error { message: String },
    Pong,
    TerminalList { terminals: Vec<McpTerminalInfo> },
    TerminalInfo { terminal: McpTerminalInfo },
    WorkspaceList { workspaces: Vec<McpWorkspaceInfo> },
    Created { id: String },
}
