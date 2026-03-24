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

fn default_resize_delta() -> f64 {
    0.05
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
    GetTerminal {
        terminal_id: String,
    },
    GetCurrentSession {
        session_id: String,
    },

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
        #[serde(default)]
        focus: Option<bool>,
    },
    CloseTerminal {
        terminal_id: String,
    },
    RenameTerminal {
        terminal_id: String,
        name: String,
    },
    FocusTerminal {
        terminal_id: String,
    },

    // Workspace queries/mutations
    ListWorkspaces,
    CreateWorkspace {
        name: String,
        folder_path: String,
    },
    DeleteWorkspace {
        workspace_id: String,
    },
    SwitchWorkspace {
        workspace_id: String,
    },
    RenameWorkspace {
        workspace_id: String,
        name: String,
    },
    ReorderWorkspaces {
        workspace_ids: Vec<String>,
    },
    GetWorkspaceDetails {
        workspace_id: String,
    },
    GetActiveWorkspace,
    GetActiveTerminal,
    MoveTerminalToWorkspace {
        terminal_id: String,
        workspace_id: String,
    },
    RemoveWorktree {
        worktree_path: String,
    },
    ToggleWorktreeMode {
        workspace_id: String,
    },
    ToggleClaudeCodeMode {
        workspace_id: String,
    },
    GetWorkspaceModes {
        workspace_id: String,
    },

    // Terminal I/O
    WriteToTerminal {
        terminal_id: String,
        data: String,
        #[serde(default)]
        focus: Option<bool>,
    },
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
    ReadGrid {
        terminal_id: String,
    },

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
        #[serde(default)]
        focus: Option<bool>,
    },
    EraseContent {
        terminal_id: String,
        #[serde(default = "default_erase_count")]
        count: usize,
        #[serde(default)]
        focus: Option<bool>,
    },
    ExecuteCommand {
        terminal_id: String,
        command: String,
        #[serde(default = "default_idle_ms")]
        idle_ms: u64,
        #[serde(default = "default_timeout_ms")]
        timeout_ms: u64,
        #[serde(default)]
        focus: Option<bool>,
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

    // OS integration
    OpenInExplorer {
        path: String,
    },

    // Split pane focus and resize (Pattern C — execute_js bridge)
    FocusPane {
        #[serde(default)]
        workspace_id: Option<String>,
        direction: String,
    },
    FocusOtherPane {
        #[serde(default)]
        workspace_id: Option<String>,
    },
    ResizePane {
        #[serde(default)]
        workspace_id: Option<String>,
        direction: String,
        #[serde(default = "default_resize_delta")]
        delta: f64,
    },
    SetSplitRatio {
        #[serde(default)]
        workspace_id: Option<String>,
        ratio: f64,
    },
    RotateSplit {
        #[serde(default)]
        workspace_id: Option<String>,
    },

    // Shell settings
    ListAvailableShells,
    GetDefaultShell,
    SetDefaultShell {
        shell_type: String,
        #[serde(default)]
        wsl_distribution: Option<String>,
        #[serde(default)]
        custom_program: Option<String>,
        #[serde(default)]
        custom_args: Option<Vec<String>>,
    },

    // JS bridge (execute JavaScript in WebView, return result)
    ExecuteJs {
        script: String,
    },

    // Scrollback control
    ScrollPageUp {
        #[serde(default)]
        terminal_id: Option<String>,
    },
    ScrollPageDown {
        #[serde(default)]
        terminal_id: Option<String>,
    },
    ScrollToTop {
        #[serde(default)]
        terminal_id: Option<String>,
    },
    ScrollToBottom {
        #[serde(default)]
        terminal_id: Option<String>,
    },
    GetScrollPosition {
        #[serde(default)]
        terminal_id: Option<String>,
    },

    // Font/zoom controls
    ZoomIn,
    ZoomOut,
    ZoomReset,
    GetFontSize,

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

    // App control
    OpenSettings {
        #[serde(default)]
        tab: Option<String>,
    },
    SaveLayout,
    GetAppInfo,

    // Tab management
    ReorderTabs {
        workspace_id: String,
        terminal_ids: Vec<String>,
    },
    GetTabOrder {
        workspace_id: String,
    },

    // Clipboard
    CopyToClipboard {
        text: String,
    },

    // Selection
    GetSelectedText {
        #[serde(default)]
        terminal_id: Option<String>,
    },

    // Theme management
    ListThemes,
    GetActiveTheme,
    SetTheme {
        theme_name: String,
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

    // Test harness lifecycle
    TestHarnessStatus,
    ResetStagingProfile,
    CollectArtifactBundle {
        #[serde(default)]
        run_id: Option<String>,
    },
    ExportStateDump,
    WaitForAppReady {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },

    // Content pane management
    OpenFilePane {
        file_path: String,
        #[serde(default)]
        target_terminal_id: Option<String>,
        #[serde(default = "default_split_direction")]
        direction: String,
        #[serde(default = "default_split_ratio")]
        ratio: f64,
    },
    ClosePane {
        pane_id: String,
    },
    ListPanes {
        #[serde(default)]
        workspace_id: Option<String>,
    },
    UpdateFilePane {
        pane_id: String,
        file_path: String,
    },

    // Semantic testing API
    UiQuery {
        target: String,
        #[serde(default)]
        args: Option<serde_json::Value>,
    },
    UiAct {
        target: String,
        action: String,
        #[serde(default)]
        args: Option<serde_json::Value>,
    },
    UiWait {
        condition: String,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        poll_interval_ms: Option<u64>,
        #[serde(default)]
        args: Option<serde_json::Value>,
    },
}

/// Terminal info returned by MCP queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTerminalInfo {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub process_name: String,
    #[serde(default)]
    pub exited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
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
    Error {
        message: String,
    },
    Pong,
    TerminalList {
        terminals: Vec<McpTerminalInfo>,
    },
    TerminalInfo {
        terminal: McpTerminalInfo,
    },
    WorkspaceList {
        workspaces: Vec<McpWorkspaceInfo>,
    },
    Created {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktree_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        worktree_branch: Option<String>,
    },
    NotificationStatus {
        enabled: bool,
        source: String,
    },
    TerminalOutput {
        content: String,
    },
    WorkspaceDetails {
        name: String,
        folder_path: String,
        worktree_mode: bool,
        claude_code_mode: bool,
        terminal_count: usize,
    },
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
    ScrollPosition {
        offset: u32,
        total_scrollback: u32,
        viewport_rows: u32,
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
    AppInfo {
        version: String,
        workspace_count: usize,
        terminal_count: usize,
        daemon_connected: bool,
    },
    TabOrder {
        terminal_ids: Vec<String>,
    },
    SelectedText {
        text: String,
    },
    ThemeList {
        themes: Vec<String>,
        active: String,
    },
    AvailableShells {
        shells: Vec<String>,
    },
    ShellInfo {
        shell_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wsl_distribution: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        custom_program: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        custom_args: Option<Vec<String>>,
    },
    FontSize {
        size: u32,
    },

    // Content pane responses
    PaneCreated {
        pane_id: String,
        file_type: String,
    },
    PaneList {
        panes: Vec<crate::layout_tree::PaneInfo>,
    },

    // Test harness responses
    TestHarnessStatus {
        ready: bool,
        frontend_type: String,
        harness_mode: bool,
        run_id: Option<String>,
        uptime_ms: u64,
    },
    StateDump {
        dump: serde_json::Value,
    },
    ArtifactBundle {
        run_id: String,
        artifact_dir: String,
        manifest: serde_json::Value,
    },
    QueryResult {
        ok: bool,
        target: String,
        data: Option<serde_json::Value>,
        error: Option<String>,
        timestamp_ms: u64,
    },
    ActionResult {
        ok: bool,
        target: String,
        action: String,
        error: Option<String>,
        timestamp_ms: u64,
    },
    WaitCompleted {
        ok: bool,
        condition: String,
        timed_out: bool,
        elapsed_ms: u64,
        error: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_terminal_info(exited: bool, exit_code: Option<i64>) -> McpTerminalInfo {
        McpTerminalInfo {
            id: "term-abc-123".to_string(),
            workspace_id: "ws-001".to_string(),
            name: "powershell".to_string(),
            process_name: "powershell".to_string(),
            exited,
            exit_code,
        }
    }

    #[test]
    fn mcp_terminal_info_running_roundtrip() {
        let info = make_terminal_info(false, None);
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"exited\":false"));
        assert!(
            !json.contains("exit_code"),
            "exit_code should be omitted when None, got: {}",
            json
        );
        let d: McpTerminalInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(d.id, "term-abc-123");
        assert!(!d.exited);
        assert_eq!(d.exit_code, None);
    }

    #[test]
    fn mcp_terminal_info_exited_with_code_zero() {
        let info = make_terminal_info(true, Some(0));
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"exited\":true"));
        assert!(json.contains("\"exit_code\":0"));
        let d: McpTerminalInfo = serde_json::from_str(&json).unwrap();
        assert!(d.exited);
        assert_eq!(d.exit_code, Some(0));
    }

    #[test]
    fn mcp_terminal_info_exited_with_nonzero_code() {
        let info = make_terminal_info(true, Some(1));
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"exit_code\":1"));
        let d: McpTerminalInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(d.exit_code, Some(1));
    }

    #[test]
    fn mcp_terminal_info_exited_with_negative_code() {
        let info = make_terminal_info(true, Some(-9));
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"exit_code\":-9"));
        let d: McpTerminalInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(d.exit_code, Some(-9));
    }

    #[test]
    fn mcp_terminal_info_exited_without_code() {
        let info = make_terminal_info(true, None);
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"exited\":true"));
        assert!(
            !json.contains("exit_code"),
            "exit_code should be omitted when None, got: {}",
            json
        );
        let d: McpTerminalInfo = serde_json::from_str(&json).unwrap();
        assert!(d.exited);
        assert_eq!(d.exit_code, None);
    }

    #[test]
    fn mcp_terminal_info_backward_compat_no_exited_field() {
        let json = r#"{
            "id": "term-old",
            "workspace_id": "ws-old",
            "name": "bash",
            "process_name": "bash"
        }"#;
        let d: McpTerminalInfo = serde_json::from_str(json).unwrap();
        assert_eq!(d.id, "term-old");
        assert!(!d.exited);
        assert_eq!(d.exit_code, None);
    }

    #[test]
    fn mcp_terminal_info_backward_compat_exited_only() {
        let json = r#"{
            "id": "term-v2",
            "workspace_id": "ws-1",
            "name": "pwsh",
            "process_name": "pwsh",
            "exited": true
        }"#;
        let d: McpTerminalInfo = serde_json::from_str(json).unwrap();
        assert!(d.exited);
        assert_eq!(d.exit_code, None);
    }

    #[test]
    fn mcp_terminal_info_large_windows_exit_code() {
        let info = make_terminal_info(true, Some(3221225477));
        let json = serde_json::to_string(&info).unwrap();
        let d: McpTerminalInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(d.exit_code, Some(3221225477));
    }

    #[test]
    fn mcp_response_terminal_list_with_exit_info() {
        let terminals = vec![
            make_terminal_info(false, None),
            make_terminal_info(true, Some(0)),
            make_terminal_info(true, Some(1)),
        ];
        let resp = McpResponse::TerminalList { terminals };
        let json = serde_json::to_string(&resp).unwrap();
        let d: McpResponse = serde_json::from_str(&json).unwrap();
        match d {
            McpResponse::TerminalList { terminals } => {
                assert_eq!(terminals.len(), 3);
                assert!(!terminals[0].exited);
                assert_eq!(terminals[0].exit_code, None);
                assert!(terminals[1].exited);
                assert_eq!(terminals[1].exit_code, Some(0));
                assert!(terminals[2].exited);
                assert_eq!(terminals[2].exit_code, Some(1));
            }
            other => panic!("Expected TerminalList, got {:?}", other),
        }
    }

    #[test]
    fn mcp_response_terminal_info_with_exit_code() {
        let info = make_terminal_info(true, Some(42));
        let resp = McpResponse::TerminalInfo { terminal: info };
        let json = serde_json::to_string(&resp).unwrap();
        let d: McpResponse = serde_json::from_str(&json).unwrap();
        match d {
            McpResponse::TerminalInfo { terminal } => {
                assert!(terminal.exited);
                assert_eq!(terminal.exit_code, Some(42));
                assert_eq!(terminal.name, "powershell");
            }
            other => panic!("Expected TerminalInfo, got {:?}", other),
        }
    }

    #[test]
    fn mcp_response_active_terminal_none() {
        let resp = McpResponse::ActiveTerminal { terminal: None };
        let json = serde_json::to_string(&resp).unwrap();
        let d: McpResponse = serde_json::from_str(&json).unwrap();
        match d {
            McpResponse::ActiveTerminal { terminal } => {
                assert!(terminal.is_none());
            }
            other => panic!("Expected ActiveTerminal, got {:?}", other),
        }
    }

    #[test]
    fn mcp_response_active_terminal_exited() {
        let info = make_terminal_info(true, Some(130));
        let resp = McpResponse::ActiveTerminal {
            terminal: Some(info),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let d: McpResponse = serde_json::from_str(&json).unwrap();
        match d {
            McpResponse::ActiveTerminal { terminal } => {
                let t = terminal.unwrap();
                assert!(t.exited);
                assert_eq!(t.exit_code, Some(130));
            }
            other => panic!("Expected ActiveTerminal, got {:?}", other),
        }
    }
}
