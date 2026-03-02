use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// Re-export layout tree types from protocol
pub use godly_protocol::LayoutNode;

// Re-export ShellType from protocol — single source of truth for both crates
pub use godly_protocol::ShellType;

/// AI tool mode for a workspace — which AI tool(s) to auto-launch in new terminals.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AiToolMode {
    None,
    Claude,
    Codex,
    Both,
}

impl Default for AiToolMode {
    fn default() -> Self {
        AiToolMode::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Terminal {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub process_name: String,
}

/// Raw deserialization helper that handles both old (`claude_code_mode: bool`)
/// and new (`ai_tool_mode: AiToolMode`) JSON formats.
#[derive(Deserialize)]
struct WorkspaceRaw {
    id: String,
    name: String,
    folder_path: String,
    tab_order: Vec<String>,
    #[serde(default)]
    shell_type: ShellType,
    #[serde(default)]
    worktree_mode: bool,
    /// New field — present in new-format JSON.
    #[serde(default)]
    ai_tool_mode: Option<AiToolMode>,
    /// Legacy field — present in old-format JSON.
    #[serde(default)]
    claude_code_mode: Option<bool>,
}

impl From<WorkspaceRaw> for Workspace {
    fn from(raw: WorkspaceRaw) -> Self {
        let ai_tool_mode = match raw.ai_tool_mode {
            Some(mode) => mode,
            None => match raw.claude_code_mode {
                Some(true) => AiToolMode::Claude,
                _ => AiToolMode::None,
            },
        };
        Workspace {
            id: raw.id,
            name: raw.name,
            folder_path: raw.folder_path,
            tab_order: raw.tab_order,
            shell_type: raw.shell_type,
            worktree_mode: raw.worktree_mode,
            ai_tool_mode,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(into = "WorkspaceSer")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub tab_order: Vec<String>,
    pub shell_type: ShellType,
    pub worktree_mode: bool,
    pub ai_tool_mode: AiToolMode,
}

/// Serialization helper — writes the new `ai_tool_mode` field only.
#[derive(Serialize)]
struct WorkspaceSer {
    id: String,
    name: String,
    folder_path: String,
    tab_order: Vec<String>,
    shell_type: ShellType,
    worktree_mode: bool,
    ai_tool_mode: AiToolMode,
}

impl From<Workspace> for WorkspaceSer {
    fn from(ws: Workspace) -> Self {
        WorkspaceSer {
            id: ws.id,
            name: ws.name,
            folder_path: ws.folder_path,
            tab_order: ws.tab_order,
            shell_type: ws.shell_type,
            worktree_mode: ws.worktree_mode,
            ai_tool_mode: ws.ai_tool_mode,
        }
    }
}

impl<'de> Deserialize<'de> for Workspace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = WorkspaceRaw::deserialize(deserializer)?;
        Ok(Workspace::from(raw))
    }
}

#[deprecated(note = "Use LayoutNode tree instead for recursive split pane support")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitView {
    pub left_terminal_id: String,
    pub right_terminal_id: String,
    pub direction: String,  // "horizontal" or "vertical"
    pub ratio: f64,
}

#[allow(deprecated)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub workspaces: Vec<Workspace>,
    pub terminals: Vec<TerminalInfo>,
    pub active_workspace_id: Option<String>,
    #[serde(default)]
    pub split_views: HashMap<String, SplitView>,
    /// Recursive layout trees per workspace (replaces flat split_views).
    #[serde(default)]
    pub layout_trees: HashMap<String, LayoutNode>,
    /// Window geometry and monitor for cross-session restoration.
    #[serde(default)]
    pub window_state: Option<WindowState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    #[serde(default)]
    pub shell_type: ShellType,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
}

/// Window geometry and monitor identity for cross-session restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub monitor_name: Option<String>,
}

/// Metadata about a daemon session tracked by the Tauri app (for persistence).
/// Replaces the need to query PTY sessions directly for shell_type and cwd.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub shell_type: ShellType,
    pub cwd: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
}

#[allow(deprecated)]
impl Default for Layout {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            terminals: Vec::new(),
            active_workspace_id: None,
            split_views: HashMap::new(),
            layout_trees: HashMap::new(),
            window_state: None,
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_windows_serialization() {
        let shell = ShellType::Windows;
        let json = serde_json::to_string(&shell).unwrap();
        assert_eq!(json, "\"windows\"");

        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ShellType::Windows);
    }

    #[test]
    fn test_shell_type_wsl_with_distribution() {
        let shell = ShellType::Wsl {
            distribution: Some("Ubuntu".to_string()),
        };
        let json = serde_json::to_string(&shell).unwrap();
        assert!(json.contains("wsl"));
        assert!(json.contains("Ubuntu"));

        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, shell);
    }

    #[test]
    fn test_shell_type_wsl_without_distribution() {
        let shell = ShellType::Wsl { distribution: None };
        let json = serde_json::to_string(&shell).unwrap();
        assert!(json.contains("wsl"));
        assert!(json.contains("null"));

        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, shell);
    }

    #[test]
    fn test_shell_type_pwsh_serialization() {
        let shell = ShellType::Pwsh;
        let json = serde_json::to_string(&shell).unwrap();
        assert_eq!(json, "\"pwsh\"");

        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ShellType::Pwsh);
    }

    #[test]
    fn test_shell_type_cmd_serialization() {
        let shell = ShellType::Cmd;
        let json = serde_json::to_string(&shell).unwrap();
        assert_eq!(json, "\"cmd\"");

        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ShellType::Cmd);
    }

    #[test]
    fn test_shell_type_windows_backward_compat() {
        // Existing persisted data with "windows" should still deserialize correctly
        let json = "\"windows\"";
        let shell: ShellType = serde_json::from_str(json).unwrap();
        assert_eq!(shell, ShellType::Windows);
    }

    #[test]
    fn test_shell_type_default_is_windows() {
        let shell = ShellType::default();
        assert_eq!(shell, ShellType::Windows);
    }

    #[test]
    fn test_workspace_roundtrip_with_shell_type() {
        let workspace = Workspace {
            id: "ws-1".to_string(),
            name: "Test Workspace".to_string(),
            folder_path: "C:\\Users\\test".to_string(),
            tab_order: vec!["term-1".to_string(), "term-2".to_string()],
            shell_type: ShellType::Wsl {
                distribution: Some("Debian".to_string()),
            },
            worktree_mode: false,
            ai_tool_mode: AiToolMode::None,
        };

        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, workspace.id);
        assert_eq!(deserialized.name, workspace.name);
        assert_eq!(deserialized.folder_path, workspace.folder_path);
        assert_eq!(deserialized.tab_order, workspace.tab_order);
        assert_eq!(deserialized.shell_type, workspace.shell_type);
    }

    #[test]
    fn test_workspace_default_shell_type_on_missing_field() {
        // Simulate JSON without shell_type or worktree_mode fields
        let json = r#"{
            "id": "ws-1",
            "name": "Test",
            "folder_path": "/home/user",
            "tab_order": []
        }"#;

        let workspace: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(workspace.shell_type, ShellType::Windows);
        assert!(!workspace.worktree_mode);
        assert_eq!(workspace.ai_tool_mode, AiToolMode::None);
    }

    #[test]
    fn test_terminal_info_roundtrip() {
        let terminal = TerminalInfo {
            id: "term-1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "Shell".to_string(),
            shell_type: ShellType::Wsl {
                distribution: Some("Ubuntu-22.04".to_string()),
            },
            cwd: Some("/home/user/project".to_string()),
            worktree_path: None,
            worktree_branch: None,
        };

        let json = serde_json::to_string(&terminal).unwrap();
        let deserialized: TerminalInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, terminal.id);
        assert_eq!(deserialized.shell_type, terminal.shell_type);
        assert_eq!(deserialized.cwd, terminal.cwd);
    }

    #[test]
    fn test_layout_with_mixed_shell_types() {
        let layout = Layout {
            workspaces: vec![
                Workspace {
                    id: "ws-1".to_string(),
                    name: "Windows".to_string(),
                    folder_path: "C:\\".to_string(),
                    tab_order: vec![],
                    shell_type: ShellType::Windows,
                    worktree_mode: false,
                    ai_tool_mode: AiToolMode::None,
                },
                Workspace {
                    id: "ws-2".to_string(),
                    name: "WSL".to_string(),
                    folder_path: "/home".to_string(),
                    tab_order: vec![],
                    shell_type: ShellType::Wsl {
                        distribution: Some("Alpine".to_string()),
                    },
                    worktree_mode: false,
                    ai_tool_mode: AiToolMode::None,
                },
            ],
            terminals: vec![],
            active_workspace_id: Some("ws-1".to_string()),
            split_views: HashMap::new(),
            layout_trees: HashMap::new(),
            window_state: None,
        };

        let json = serde_json::to_string(&layout).unwrap();
        let deserialized: Layout = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.workspaces.len(), 2);
        assert_eq!(deserialized.workspaces[0].shell_type, ShellType::Windows);
        assert_eq!(
            deserialized.workspaces[1].shell_type,
            ShellType::Wsl {
                distribution: Some("Alpine".to_string())
            }
        );
    }

    #[test]
    fn test_layout_full_roundtrip_with_terminals() {
        // This test simulates what gets saved and loaded
        let layout = Layout {
            workspaces: vec![Workspace {
                id: "ws-abc123".to_string(),
                name: "My Project".to_string(),
                folder_path: "C:\\Projects\\myapp".to_string(),
                tab_order: vec!["term-1".to_string(), "term-2".to_string()],
                shell_type: ShellType::Windows,
                worktree_mode: false,
                ai_tool_mode: AiToolMode::None,
            }],
            terminals: vec![
                TerminalInfo {
                    id: "term-1".to_string(),
                    workspace_id: "ws-abc123".to_string(),
                    name: "Terminal 1".to_string(),
                    shell_type: ShellType::Windows,
                    cwd: Some("C:\\Projects\\myapp\\src".to_string()),
                    worktree_path: None,
                    worktree_branch: None,
                },
                TerminalInfo {
                    id: "term-2".to_string(),
                    workspace_id: "ws-abc123".to_string(),
                    name: "WSL Terminal".to_string(),
                    shell_type: ShellType::Wsl {
                        distribution: Some("Ubuntu".to_string()),
                    },
                    cwd: Some("/home/user/projects".to_string()),
                    worktree_path: None,
                    worktree_branch: None,
                },
            ],
            active_workspace_id: Some("ws-abc123".to_string()),
            split_views: HashMap::new(),
            layout_trees: HashMap::new(),
            window_state: None,
        };

        // Serialize to JSON (simulates save)
        let json = serde_json::to_string_pretty(&layout).unwrap();

        // Deserialize from JSON (simulates load)
        let restored: Layout = serde_json::from_str(&json).unwrap();

        // Verify all data is preserved
        assert_eq!(restored.workspaces.len(), 1);
        assert_eq!(restored.workspaces[0].id, "ws-abc123");
        assert_eq!(restored.workspaces[0].tab_order.len(), 2);

        assert_eq!(restored.terminals.len(), 2);
        assert_eq!(restored.terminals[0].id, "term-1");
        assert_eq!(restored.terminals[0].cwd, Some("C:\\Projects\\myapp\\src".to_string()));
        assert_eq!(restored.terminals[1].id, "term-2");
        assert_eq!(
            restored.terminals[1].shell_type,
            ShellType::Wsl {
                distribution: Some("Ubuntu".to_string())
            }
        );

        assert_eq!(restored.active_workspace_id, Some("ws-abc123".to_string()));
    }

    #[test]
    fn test_terminal_ids_preserved_in_layout() {
        // This is the critical test for the ID preservation fix
        let original_id = "original-terminal-id-12345";

        let layout = Layout {
            workspaces: vec![Workspace {
                id: "ws-1".to_string(),
                name: "Test".to_string(),
                folder_path: "/test".to_string(),
                tab_order: vec![],
                shell_type: ShellType::Windows,
                worktree_mode: false,
                ai_tool_mode: AiToolMode::None,
            }],
            terminals: vec![TerminalInfo {
                id: original_id.to_string(),
                workspace_id: "ws-1".to_string(),
                name: "Test Terminal".to_string(),
                shell_type: ShellType::Windows,
                cwd: None,
                worktree_path: None,
                worktree_branch: None,
            }],
            active_workspace_id: Some("ws-1".to_string()),
            split_views: HashMap::new(),
            layout_trees: HashMap::new(),
            window_state: None,
        };

        let json = serde_json::to_string(&layout).unwrap();
        let restored: Layout = serde_json::from_str(&json).unwrap();

        // The terminal ID must be exactly preserved
        assert_eq!(restored.terminals[0].id, original_id);
    }

    #[test]
    fn test_workspace_default_worktree_mode_on_missing_field() {
        let json = r#"{
            "id": "ws-1",
            "name": "Test",
            "folder_path": "/home/user",
            "tab_order": [],
            "shell_type": "windows"
        }"#;

        let workspace: Workspace = serde_json::from_str(json).unwrap();
        assert!(!workspace.worktree_mode);
    }

    #[test]
    fn test_workspace_roundtrip_with_worktree_mode() {
        let workspace = Workspace {
            id: "ws-1".to_string(),
            name: "Worktree WS".to_string(),
            folder_path: "C:\\repo".to_string(),
            tab_order: vec![],
            shell_type: ShellType::Windows,
            worktree_mode: true,
            ai_tool_mode: AiToolMode::None,
        };

        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert!(deserialized.worktree_mode);
    }

    #[test]
    fn test_terminal_info_default_worktree_path_on_missing_field() {
        let json = r#"{
            "id": "term-1",
            "workspace_id": "ws-1",
            "name": "Shell",
            "shell_type": "windows"
        }"#;

        let info: TerminalInfo = serde_json::from_str(json).unwrap();
        assert!(info.worktree_path.is_none());
    }

    #[test]
    fn test_workspace_default_ai_tool_mode_on_missing_field() {
        let json = r#"{
            "id": "ws-1",
            "name": "Test",
            "folder_path": "/home/user",
            "tab_order": [],
            "shell_type": "windows"
        }"#;

        let workspace: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(workspace.ai_tool_mode, AiToolMode::None);
    }

    #[test]
    fn test_workspace_roundtrip_with_ai_tool_mode() {
        let workspace = Workspace {
            id: "ws-1".to_string(),
            name: "Claude WS".to_string(),
            folder_path: "C:\\repo".to_string(),
            tab_order: vec![],
            shell_type: ShellType::Windows,
            worktree_mode: false,
            ai_tool_mode: AiToolMode::Claude,
        };

        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.ai_tool_mode, AiToolMode::Claude);
    }

    #[test]
    fn test_workspace_backward_compat_claude_code_mode_true() {
        // Old JSON with "claude_code_mode": true should migrate to AiToolMode::Claude
        let json = r#"{
            "id": "ws-1",
            "name": "Old WS",
            "folder_path": "C:\\repo",
            "tab_order": [],
            "shell_type": "windows",
            "claude_code_mode": true
        }"#;

        let workspace: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(workspace.ai_tool_mode, AiToolMode::Claude);
    }

    #[test]
    fn test_workspace_backward_compat_claude_code_mode_false() {
        let json = r#"{
            "id": "ws-1",
            "name": "Old WS",
            "folder_path": "C:\\repo",
            "tab_order": [],
            "claude_code_mode": false
        }"#;

        let workspace: Workspace = serde_json::from_str(json).unwrap();
        assert_eq!(workspace.ai_tool_mode, AiToolMode::None);
    }

    #[test]
    fn test_ai_tool_mode_serialization_roundtrip() {
        for mode in &[AiToolMode::None, AiToolMode::Claude, AiToolMode::Codex, AiToolMode::Both] {
            let json = serde_json::to_string(mode).unwrap();
            let deserialized: AiToolMode = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, *mode);
        }
    }

    #[test]
    fn test_split_view_serialization_roundtrip() {
        let split = SplitView {
            left_terminal_id: "term-1".to_string(),
            right_terminal_id: "term-2".to_string(),
            direction: "horizontal".to_string(),
            ratio: 0.6,
        };

        let json = serde_json::to_string(&split).unwrap();
        let restored: SplitView = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.left_terminal_id, "term-1");
        assert_eq!(restored.right_terminal_id, "term-2");
        assert_eq!(restored.direction, "horizontal");
        assert!((restored.ratio - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_layout_with_split_views_roundtrip() {
        let mut split_views = HashMap::new();
        split_views.insert(
            "ws-1".to_string(),
            SplitView {
                left_terminal_id: "term-1".to_string(),
                right_terminal_id: "term-2".to_string(),
                direction: "horizontal".to_string(),
                ratio: 0.5,
            },
        );

        let layout = Layout {
            workspaces: vec![Workspace {
                id: "ws-1".to_string(),
                name: "Test".to_string(),
                folder_path: "C:\\test".to_string(),
                tab_order: vec![],
                shell_type: ShellType::Windows,
                worktree_mode: false,
                ai_tool_mode: AiToolMode::None,
            }],
            terminals: vec![],
            active_workspace_id: Some("ws-1".to_string()),
            split_views,
            layout_trees: HashMap::new(),
            window_state: None,
        };

        let json = serde_json::to_string(&layout).unwrap();
        let restored: Layout = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.split_views.len(), 1);
        let sv = restored.split_views.get("ws-1").unwrap();
        assert_eq!(sv.left_terminal_id, "term-1");
        assert_eq!(sv.right_terminal_id, "term-2");
        assert_eq!(sv.direction, "horizontal");
    }

    #[test]
    fn test_layout_backward_compat_without_split_views() {
        // Simulate old JSON without split_views field
        let json = r#"{
            "workspaces": [],
            "terminals": [],
            "active_workspace_id": null
        }"#;

        let layout: Layout = serde_json::from_str(json).unwrap();
        assert!(layout.split_views.is_empty());
    }

    #[test]
    fn test_session_metadata_default_worktree_path_on_missing_field() {
        let json = r#"{
            "shell_type": "windows",
            "cwd": "C:\\test"
        }"#;

        let meta: SessionMetadata = serde_json::from_str(json).unwrap();
        assert!(meta.worktree_path.is_none());
    }

    #[test]
    fn test_shell_type_custom_serialization() {
        let shell = ShellType::Custom {
            program: "nu.exe".to_string(),
            args: Some(vec!["-l".to_string()]),
        };
        let json = serde_json::to_string(&shell).unwrap();
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, shell);
    }

    #[test]
    fn test_shell_type_custom_display_name() {
        let shell = ShellType::Custom {
            program: "C:\\Tools\\nu.exe".to_string(),
            args: None,
        };
        assert_eq!(shell.display_name(), "nu");
    }

    #[test]
    fn test_workspace_roundtrip_with_custom_shell() {
        let workspace = Workspace {
            id: "ws-1".to_string(),
            name: "Custom Shell".to_string(),
            folder_path: "C:\\Projects".to_string(),
            tab_order: vec![],
            shell_type: ShellType::Custom {
                program: "nu.exe".to_string(),
                args: Some(vec!["-l".to_string()]),
            },
            worktree_mode: false,
            ai_tool_mode: AiToolMode::None,
        };

        let json = serde_json::to_string(&workspace).unwrap();
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.shell_type, workspace.shell_type);
    }

    #[test]
    fn test_layout_with_custom_and_mixed_shell_types() {
        let layout = Layout {
            workspaces: vec![],
            terminals: vec![
                TerminalInfo {
                    id: "term-1".to_string(),
                    workspace_id: "ws-1".to_string(),
                    name: "Nu Shell".to_string(),
                    shell_type: ShellType::Custom {
                        program: "nu.exe".to_string(),
                        args: None,
                    },
                    cwd: Some("C:\\Projects".to_string()),
                    worktree_path: None,
                    worktree_branch: None,
                },
                TerminalInfo {
                    id: "term-2".to_string(),
                    workspace_id: "ws-1".to_string(),
                    name: "PowerShell".to_string(),
                    shell_type: ShellType::Windows,
                    cwd: None,
                    worktree_path: None,
                    worktree_branch: None,
                },
            ],
            active_workspace_id: None,
            split_views: HashMap::new(),
            layout_trees: HashMap::new(),
            window_state: None,
        };

        let json = serde_json::to_string(&layout).unwrap();
        let restored: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.terminals[0].shell_type,
            ShellType::Custom { program: "nu.exe".to_string(), args: None }
        );
        assert_eq!(restored.terminals[1].shell_type, ShellType::Windows);
    }
}
