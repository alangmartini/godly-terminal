use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use godly_protocol::types::ShellType;
use serde::{Deserialize, Serialize};

/// TTL for cached layout data.
const CACHE_TTL: Duration = Duration::from_secs(5);

/// AI tool mode (mirrors state/models.rs).
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
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub tab_order: Vec<String>,
    #[serde(default)]
    pub shell_type: ShellType,
    #[serde(default)]
    pub worktree_mode: bool,
    #[serde(default)]
    pub ai_tool_mode: AiToolMode,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitView {
    pub left_terminal_id: String,
    pub right_terminal_id: String,
    pub direction: String,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Layout {
    pub workspaces: Vec<Workspace>,
    pub terminals: Vec<TerminalInfo>,
    pub active_workspace_id: Option<String>,
    #[serde(default)]
    pub split_views: HashMap<String, SplitView>,
}

// --- Native session format (iced-shell-session.json) ---

#[derive(Deserialize)]
struct NativeSessionState {
    active_workspace_id: Option<String>,
    #[serde(default)]
    workspaces: Vec<NativeWorkspaceState>,
    #[serde(default)]
    terminal_worktree_paths: HashMap<String, String>,
    /// Background tabs not in any layout tree.
    #[serde(default)]
    terminal_workspace_assignments: HashMap<String, String>,
}

#[derive(Deserialize)]
struct NativeWorkspaceState {
    id: String,
    name: String,
    folder_path: String,
    #[serde(default)]
    worktree_mode: bool,
    #[serde(default)]
    #[allow(dead_code)]
    focused_terminal: Option<String>,
    layout: NativeLayoutNode,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NativeLayoutNode {
    Leaf {
        terminal_id: String,
    },
    Split {
        first: Box<NativeLayoutNode>,
        second: Box<NativeLayoutNode>,
        // direction and ratio are ignored — we only need terminal IDs
    },
}

/// Recursively collect all terminal IDs from a layout tree.
fn collect_terminal_ids(node: &NativeLayoutNode, out: &mut Vec<String>) {
    match node {
        NativeLayoutNode::Leaf { terminal_id } => out.push(terminal_id.clone()),
        NativeLayoutNode::Split { first, second, .. } => {
            collect_terminal_ids(first, out);
            collect_terminal_ids(second, out);
        }
    }
}

/// Cached layout reader that reads from the native iced-shell session file.
pub struct LayoutReader {
    cache: Mutex<Option<(Layout, Instant)>>,
    session_path: PathBuf,
}

impl LayoutReader {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            session_path: Self::find_session_path(),
        }
    }

    /// Read layout, using cache if fresh enough.
    pub fn read(&self) -> Layout {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());

        if let Some((ref layout, ref ts)) = *cache {
            if ts.elapsed() < CACHE_TTL {
                return layout.clone();
            }
        }

        let layout = self.read_from_disk();
        *cache = Some((layout.clone(), Instant::now()));
        layout
    }

    /// Invalidate the cache (e.g., after a known state change).
    pub fn invalidate(&self) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        *cache = None;
    }

    fn read_from_disk(&self) -> Layout {
        let contents = match std::fs::read_to_string(&self.session_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(
                    "Cannot read session file {}: {}",
                    self.session_path.display(),
                    e
                );
                return Layout::default();
            }
        };

        let session: NativeSessionState = match serde_json::from_str(&contents) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse native session: {}", e);
                return Layout::default();
            }
        };

        self.convert_session(session)
    }

    /// Convert the native session format into the Layout struct used by routes.
    fn convert_session(&self, session: NativeSessionState) -> Layout {
        let mut terminals = Vec::new();

        let workspaces: Vec<Workspace> = session
            .workspaces
            .iter()
            .map(|ws| {
                // Collect terminal IDs from the layout tree
                let mut term_ids = Vec::new();
                collect_terminal_ids(&ws.layout, &mut term_ids);

                for tid in &term_ids {
                    terminals.push(TerminalInfo {
                        id: tid.clone(),
                        workspace_id: ws.id.clone(),
                        name: "Terminal".to_string(),
                        shell_type: ShellType::default(),
                        cwd: None,
                        worktree_path: session.terminal_worktree_paths.get(tid).cloned(),
                        worktree_branch: None,
                    });
                }

                Workspace {
                    id: ws.id.clone(),
                    name: ws.name.clone(),
                    folder_path: ws.folder_path.clone(),
                    tab_order: term_ids,
                    shell_type: ShellType::default(),
                    worktree_mode: ws.worktree_mode,
                    ai_tool_mode: AiToolMode::default(),
                }
            })
            .collect();

        // Add background tabs (terminals not in any layout tree)
        let layout_terminal_ids: std::collections::HashSet<String> =
            terminals.iter().map(|t| t.id.clone()).collect();

        for (tid, ws_id) in &session.terminal_workspace_assignments {
            if !layout_terminal_ids.contains(tid) {
                terminals.push(TerminalInfo {
                    id: tid.clone(),
                    workspace_id: ws_id.clone(),
                    name: "Terminal".to_string(),
                    shell_type: ShellType::default(),
                    cwd: None,
                    worktree_path: session.terminal_worktree_paths.get(tid).cloned(),
                    worktree_branch: None,
                });
            }
        }

        Layout {
            workspaces,
            terminals,
            active_workspace_id: session.active_workspace_id,
            split_views: HashMap::new(),
        }
    }

    fn find_session_path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata)
                .join(format!(
                    "com.godly.terminal{}",
                    godly_protocol::instance_suffix()
                ))
                .join("native")
                .join("iced-shell-session.json")
        } else {
            PathBuf::from("iced-shell-session.json")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_type_display_names() {
        assert_eq!(ShellType::Windows.display_name(), "powershell");
        assert_eq!(ShellType::Pwsh.display_name(), "pwsh");
        assert_eq!(ShellType::Cmd.display_name(), "cmd");
        assert_eq!(
            ShellType::Wsl {
                distribution: Some("Ubuntu".into())
            }
            .display_name(),
            "Ubuntu"
        );
        assert_eq!(
            ShellType::Wsl {
                distribution: None
            }
            .display_name(),
            "wsl"
        );
        assert_eq!(
            ShellType::Custom {
                program: "nu.exe".into(),
                args: None
            }
            .display_name(),
            "nu"
        );
    }

    #[test]
    fn parse_native_session_format() {
        let session_json = r#"{
            "version": 1,
            "sidebar_visible": true,
            "settings_open": false,
            "settings_tab": "general",
            "font_size": 14.0,
            "font_family": "Geist Mono",
            "next_workspace_num": 3,
            "active_workspace_id": "ws-1",
            "active_terminal_id": "t-1",
            "workspaces": [{
                "id": "ws-1",
                "name": "Test Project",
                "folder_path": "C:\\test",
                "worktree_mode": false,
                "focused_terminal": "t-1",
                "layout": {
                    "type": "split",
                    "direction": "vertical",
                    "ratio": 0.5,
                    "first": {
                        "type": "leaf",
                        "terminal_id": "t-1"
                    },
                    "second": {
                        "type": "leaf",
                        "terminal_id": "t-2"
                    }
                }
            }],
            "terminal_worktree_paths": {
                "t-1": "C:\\worktrees\\wt-1"
            },
            "terminal_workspace_assignments": {
                "t-1": "ws-1",
                "t-2": "ws-1",
                "t-3": "ws-1"
            }
        }"#;

        let session: NativeSessionState = serde_json::from_str(session_json).unwrap();
        let reader = LayoutReader {
            cache: Mutex::new(None),
            session_path: PathBuf::from("unused"),
        };
        let layout = reader.convert_session(session);

        assert_eq!(layout.workspaces.len(), 1);
        assert_eq!(layout.workspaces[0].name, "Test Project");
        assert_eq!(layout.active_workspace_id, Some("ws-1".to_string()));

        // 2 from layout tree + 1 background tab (t-3)
        assert_eq!(layout.terminals.len(), 3);
        assert!(layout.terminals.iter().all(|t| t.workspace_id == "ws-1"));

        // t-1 has worktree path
        let t1 = layout.terminals.iter().find(|t| t.id == "t-1").unwrap();
        assert_eq!(t1.worktree_path, Some("C:\\worktrees\\wt-1".to_string()));

        // t-3 is a background tab
        assert!(layout.terminals.iter().any(|t| t.id == "t-3"));
    }

    #[test]
    fn collect_terminal_ids_from_nested_tree() {
        let tree = NativeLayoutNode::Split {
            first: Box::new(NativeLayoutNode::Split {
                first: Box::new(NativeLayoutNode::Leaf {
                    terminal_id: "a".into(),
                }),
                second: Box::new(NativeLayoutNode::Leaf {
                    terminal_id: "b".into(),
                }),
            }),
            second: Box::new(NativeLayoutNode::Leaf {
                terminal_id: "c".into(),
            }),
        };
        let mut ids = Vec::new();
        collect_terminal_ids(&tree, &mut ids);
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn layout_default_is_empty() {
        let layout = Layout::default();
        assert!(layout.workspaces.is_empty());
        assert!(layout.terminals.is_empty());
        assert!(layout.active_workspace_id.is_none());
        assert!(layout.split_views.is_empty());
    }
}
