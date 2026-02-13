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
        #[serde(default)]
        worktree_name: Option<String>,
        #[serde(default)]
        worktree: Option<bool>,
        #[serde(default)]
        command: Option<String>,
    },
    CloseTerminal { terminal_id: String },
    RenameTerminal { terminal_id: String, name: String },
    FocusTerminal { terminal_id: String },

    // Workspace queries/mutations
    ListWorkspaces,
    CreateWorkspace { name: String, folder_path: String },
    DeleteWorkspace { workspace_id: String },
    SwitchWorkspace { workspace_id: String },
    GetActiveWorkspace,
    GetActiveTerminal,
    MoveTerminalToWorkspace {
        terminal_id: String,
        workspace_id: String,
    },
    RemoveWorktree { worktree_path: String },

    // Terminal I/O
    WriteToTerminal { terminal_id: String, data: String },
    ReadTerminal {
        terminal_id: String,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        lines: Option<usize>,
        #[serde(default)]
        strip_ansi: Option<bool>,
    },
    ResizeTerminal {
        terminal_id: String,
        rows: u16,
        cols: u16,
    },

    // Notifications
    Notify {
        terminal_id: String,
        #[serde(default)]
        message: Option<String>,
    },
    SetNotificationEnabled {
        #[serde(default)]
        terminal_id: Option<String>,
        #[serde(default)]
        workspace_id: Option<String>,
        enabled: bool,
    },
    GetNotificationStatus {
        #[serde(default)]
        terminal_id: Option<String>,
        #[serde(default)]
        workspace_id: Option<String>,
    },
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
    Created {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktree_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktree_branch: Option<String>,
    },
    NotificationStatus { enabled: bool, source: String },
    TerminalOutput { content: String },
    ActiveWorkspace {
        workspace: Option<McpWorkspaceInfo>,
    },
    ActiveTerminal {
        terminal: Option<McpTerminalInfo>,
    },
}
