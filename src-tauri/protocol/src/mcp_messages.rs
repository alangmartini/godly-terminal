use serde::{Deserialize, Serialize};

use crate::types::ShellType;

fn default_erase_count() -> usize {
    1
}

fn default_idle_ms() -> u64 {
    2000
}

fn default_timeout_ms() -> u64 {
    30000
}

fn default_split_direction() -> String {
    "horizontal".to_string()
}

fn default_split_ratio() -> f64 {
    0.5
}

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
    ToggleWorktreeMode { workspace_id: String },
    ToggleClaudeCodeMode { workspace_id: String },
    GetWorkspaceModes { workspace_id: String },

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

    // Grid state (godly-vt parsed terminal)
    ReadGrid { terminal_id: String },

    // Wait/polling tools
    WaitForIdle {
        terminal_id: String,
        idle_ms: u64,
        timeout_ms: u64,
    },
    WaitForText {
        terminal_id: String,
        text: String,
        timeout_ms: u64,
    },

    // Quick Claude (fire-and-forget idea capture)
    QuickClaude {
        workspace_id: String,
        prompt: String,
        #[serde(default)]
        branch_name: Option<String>,
        #[serde(default)]
        skip_fetch: Option<bool>,
        #[serde(default)]
        no_worktree: Option<bool>,
    },

    // Advanced terminal I/O
    SendKeys {
        terminal_id: String,
        keys: Vec<String>,
    },
    EraseContent {
        terminal_id: String,
        #[serde(default = "default_erase_count")]
        count: usize,
    },
    ExecuteCommand {
        terminal_id: String,
        command: String,
        #[serde(default = "default_idle_ms")]
        idle_ms: u64,
        #[serde(default = "default_timeout_ms")]
        timeout_ms: u64,
    },

    // Split view control (legacy — prefer layout tree commands below)
    CreateSplit {
        workspace_id: String,
        left_terminal_id: String,
        right_terminal_id: String,
        #[serde(default = "default_split_direction")]
        direction: String,
        #[serde(default = "default_split_ratio")]
        ratio: f64,
    },
    ClearSplit {
        workspace_id: String,
    },
    GetSplitState {
        workspace_id: String,
    },

    // Layout tree commands (recursive split pane model)
    SplitTerminal {
        workspace_id: String,
        target_terminal_id: String,
        new_terminal_id: String,
        #[serde(default = "default_split_direction")]
        direction: String,
        #[serde(default = "default_split_ratio")]
        ratio: f64,
    },
    SelfSplit {
        session_id: String,
        #[serde(default = "default_split_direction")]
        direction: String,
        #[serde(default = "default_split_ratio")]
        ratio: f64,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        command: Option<String>,
    },
    UnsplitTerminal {
        workspace_id: String,
        terminal_id: String,
    },
    GetLayoutTree {
        workspace_id: String,
    },
    SwapPanes {
        workspace_id: String,
        terminal_id_a: String,
        terminal_id_b: String,
    },
    ZoomPane {
        workspace_id: String,
        terminal_id: Option<String>,
    },

    // JS bridge (execute JavaScript in WebView, return result)
    ExecuteJs {
        script: String,
    },

    // Screenshot capture
    CaptureScreenshot {
        #[serde(default)]
        terminal_id: Option<String>,
    },

    // Terminal info export (for cross-session discovery)
    ExportTerminalInfo {
        #[serde(default)]
        terminal_id: Option<String>,
    },

    // Tab navigation
    NextTab {
        #[serde(default)]
        workspace_id: Option<String>,
    },
    PreviousTab {
        #[serde(default)]
        workspace_id: Option<String>,
    },
    GoToTab {
        #[serde(default)]
        workspace_id: Option<String>,
        index: u32,
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

    // Notification settings (via execute_js bridge)
    GetNotificationConfig,
    SetNotificationSound {
        preset: String,
    },
    AddMutePattern {
        pattern: String,
    },
    RemoveMutePattern {
        pattern: String,
    },
    ListMutePatterns,
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
    WorkspaceModes {
        worktree_mode: bool,
        claude_code_mode: bool,
    },
    WaitResult {
        completed: bool,
        last_output_ago_ms: u64,
    },
    GridSnapshot {
        rows: Vec<String>,
        cursor_row: u16,
        cursor_col: u16,
        cols: u16,
        num_rows: u16,
        alternate_screen: bool,
    },
    CommandOutput {
        output: String,
        completed: bool,
        last_output_ago_ms: u64,
        running: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_expected: Option<bool>,
    },
    SplitState {
        workspace_id: String,
        left_terminal_id: String,
        right_terminal_id: String,
        direction: String,
        ratio: f64,
    },
    NoSplit,
    SplitCreated {
        original_terminal_id: String,
        new_terminal_id: String,
        workspace_id: String,
        direction: String,
        ratio: f64,
    },
    LayoutTree {
        tree: Option<crate::layout_tree::LayoutNode>,
    },
    JsResult {
        result: Option<String>,
        error: Option<String>,
    },
    Screenshot {
        path: String,
    },
    NotificationConfig {
        enabled: bool,
        sound_preset: String,
        volume: f64,
    },
    MutePatterns {
        patterns: Vec<String>,
    },
}
