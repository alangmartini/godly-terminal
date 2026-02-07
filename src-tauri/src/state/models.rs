use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShellType {
    Windows,
    Wsl { distribution: Option<String> },
}

impl Default for ShellType {
    fn default() -> Self {
        ShellType::Windows
    }
}

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
    #[serde(default)]
    pub shell_type: ShellType,
    #[serde(default)]
    pub worktree_mode: bool,
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
    #[serde(default)]
    pub shell_type: ShellType,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
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

impl Default for Layout {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            terminals: Vec::new(),
            active_workspace_id: None,
        }
    }
}

#[cfg(test)]
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
                },
            ],
            terminals: vec![],
            active_workspace_id: Some("ws-1".to_string()),
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
    fn test_session_metadata_default_worktree_path_on_missing_field() {
        let json = r#"{
            "shell_type": "windows",
            "cwd": "C:\\test"
        }"#;

        let meta: SessionMetadata = serde_json::from_str(json).unwrap();
        assert!(meta.worktree_path.is_none());
    }
}
