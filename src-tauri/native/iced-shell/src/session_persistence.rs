use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::split_pane::{LayoutNode, SplitDirection};

pub const PERSISTENCE_VERSION: u32 = 1;
pub const AUTOSAVE_INTERVAL_SECS: u64 = 5 * 60;
const PERSISTENCE_FILE_NAME: &str = "iced-shell-session.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedSessionState {
    pub version: u32,
    pub sidebar_visible: bool,
    pub settings_open: bool,
    pub settings_tab: String,
    pub font_size: f32,
    pub next_workspace_num: u32,
    pub active_workspace_id: Option<String>,
    pub active_terminal_id: Option<String>,
    pub workspaces: Vec<PersistedWorkspaceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistedWorkspaceState {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub worktree_mode: bool,
    pub focused_terminal: String,
    pub layout: PersistedLayoutNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PersistedLayoutNode {
    Leaf {
        terminal_id: String,
    },
    Split {
        direction: PersistedSplitDirection,
        ratio: f32,
        first: Box<PersistedLayoutNode>,
        second: Box<PersistedLayoutNode>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PersistedSplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedSessionState {
    pub sidebar_visible: bool,
    pub settings_open: bool,
    pub settings_tab: String,
    pub font_size: f32,
    pub next_workspace_num: u32,
    pub active_workspace_id: Option<String>,
    pub active_terminal_id: Option<String>,
    pub workspaces: Vec<MergedWorkspaceState>,
    pub missing_live_terminal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedWorkspaceState {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub worktree_mode: bool,
    pub focused_terminal: String,
    pub layout: LayoutNode,
}

impl PersistedLayoutNode {
    pub fn from_layout(layout: &LayoutNode) -> Self {
        match layout {
            LayoutNode::Leaf { terminal_id } => Self::Leaf {
                terminal_id: terminal_id.clone(),
            },
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => Self::Split {
                direction: (*direction).into(),
                ratio: *ratio,
                first: Box::new(Self::from_layout(first)),
                second: Box::new(Self::from_layout(second)),
            },
        }
    }

    fn to_layout_filtered(&self, live_terminal_ids: &HashSet<&str>) -> Option<LayoutNode> {
        match self {
            PersistedLayoutNode::Leaf { terminal_id } => {
                if live_terminal_ids.contains(terminal_id.as_str()) {
                    Some(LayoutNode::Leaf {
                        terminal_id: terminal_id.clone(),
                    })
                } else {
                    None
                }
            }
            PersistedLayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let first = first.to_layout_filtered(live_terminal_ids);
                let second = second.to_layout_filtered(live_terminal_ids);

                match (first, second) {
                    (Some(first), Some(second)) => {
                        let ratio = if ratio.is_finite() {
                            ratio.clamp(0.05, 0.95)
                        } else {
                            0.5
                        };
                        Some(LayoutNode::Split {
                            direction: (*direction).into(),
                            ratio,
                            first: Box::new(first),
                            second: Box::new(second),
                        })
                    }
                    (Some(node), None) | (None, Some(node)) => Some(node),
                    (None, None) => None,
                }
            }
        }
    }
}

impl From<SplitDirection> for PersistedSplitDirection {
    fn from(value: SplitDirection) -> Self {
        match value {
            SplitDirection::Horizontal => Self::Horizontal,
            SplitDirection::Vertical => Self::Vertical,
        }
    }
}

impl From<PersistedSplitDirection> for SplitDirection {
    fn from(value: PersistedSplitDirection) -> Self {
        match value {
            PersistedSplitDirection::Horizontal => SplitDirection::Horizontal,
            PersistedSplitDirection::Vertical => SplitDirection::Vertical,
        }
    }
}

pub fn default_persistence_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let directory_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    base.join(directory_name)
        .join("native")
        .join(PERSISTENCE_FILE_NAME)
}

pub fn load_from_default_path() -> Option<PersistedSessionState> {
    load_from_path(&default_persistence_path())
}

pub fn load_from_path(path: &Path) -> Option<PersistedSessionState> {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!(
                "Failed to read native session state from {}: {}",
                path.display(),
                error
            );
            return None;
        }
    };

    let state: PersistedSessionState = match serde_json::from_str(&json) {
        Ok(state) => state,
        Err(error) => {
            log::warn!(
                "Failed to parse native session state from {}: {}",
                path.display(),
                error
            );
            return None;
        }
    };

    if state.version != PERSISTENCE_VERSION {
        log::warn!(
            "Ignoring native session state with unsupported version {} (expected {})",
            state.version,
            PERSISTENCE_VERSION
        );
        return None;
    }

    Some(state)
}

pub fn save_to_default_path(state: &PersistedSessionState) -> Result<(), String> {
    save_to_path(&default_persistence_path(), state)
}

pub fn save_to_path(path: &Path, state: &PersistedSessionState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }

    let mut state = state.clone();
    state.version = PERSISTENCE_VERSION;
    let json = serde_json::to_string_pretty(&state)
        .map_err(|error| format!("Failed to serialize native session state: {}", error))?;

    std::fs::write(path, json)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(())
}

pub fn merge_with_live_sessions(
    persisted: &PersistedSessionState,
    live_session_ids: &[String],
) -> MergedSessionState {
    let live_terminal_ids: HashSet<&str> = live_session_ids.iter().map(String::as_str).collect();
    let mut workspaces = Vec::new();
    let mut used_terminal_ids = HashSet::new();
    let mut seen_workspace_ids = HashSet::new();

    for workspace in &persisted.workspaces {
        if !seen_workspace_ids.insert(workspace.id.clone()) {
            continue;
        }

        let Some(layout) = workspace.layout.to_layout_filtered(&live_terminal_ids) else {
            continue;
        };

        let leaf_ids: Vec<String> = layout
            .all_leaf_ids()
            .into_iter()
            .map(|id| id.to_string())
            .collect();
        if leaf_ids.is_empty() {
            continue;
        }
        used_terminal_ids.extend(leaf_ids.iter().cloned());

        let focused_terminal = if leaf_ids.iter().any(|id| id == &workspace.focused_terminal) {
            workspace.focused_terminal.clone()
        } else {
            leaf_ids[0].clone()
        };

        workspaces.push(MergedWorkspaceState {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            folder_path: workspace.folder_path.clone(),
            worktree_mode: workspace.worktree_mode,
            focused_terminal,
            layout,
        });
    }

    let active_workspace_id = persisted
        .active_workspace_id
        .as_ref()
        .and_then(|workspace_id| {
            workspaces
                .iter()
                .find(|workspace| workspace.id == *workspace_id)
                .map(|_| workspace_id.clone())
        })
        .or_else(|| workspaces.first().map(|workspace| workspace.id.clone()));

    let active_terminal_id = persisted
        .active_terminal_id
        .as_ref()
        .filter(|terminal_id| live_terminal_ids.contains(terminal_id.as_str()))
        .cloned()
        .or_else(|| {
            active_workspace_id.as_ref().and_then(|workspace_id| {
                workspaces
                    .iter()
                    .find(|workspace| workspace.id == *workspace_id)
                    .map(|workspace| workspace.focused_terminal.clone())
            })
        });

    let missing_live_terminal_ids = live_session_ids
        .iter()
        .filter(|terminal_id| !used_terminal_ids.contains(terminal_id.as_str()))
        .cloned()
        .collect();

    MergedSessionState {
        sidebar_visible: persisted.sidebar_visible,
        settings_open: persisted.settings_open,
        settings_tab: sanitize_settings_tab(&persisted.settings_tab),
        font_size: sanitize_font_size(persisted.font_size),
        next_workspace_num: persisted.next_workspace_num.max(2),
        active_workspace_id,
        active_terminal_id,
        workspaces,
        missing_live_terminal_ids,
    }
}

fn sanitize_settings_tab(settings_tab: &str) -> String {
    if settings_tab.trim().is_empty() {
        "shortcuts".to_string()
    } else {
        settings_tab.to_string()
    }
}

fn sanitize_font_size(font_size: f32) -> f32 {
    if font_size.is_finite() {
        font_size.max(8.0)
    } else {
        13.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_and_deserializes_state() {
        let state = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: true,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 14.0,
            next_workspace_num: 4,
            active_workspace_id: Some("w-1".to_string()),
            active_terminal_id: Some("t-2".to_string()),
            workspaces: vec![PersistedWorkspaceState {
                id: "w-1".to_string(),
                name: "Workspace 1".to_string(),
                folder_path: ".".to_string(),
                worktree_mode: false,
                focused_terminal: "t-2".to_string(),
                layout: PersistedLayoutNode::Split {
                    direction: PersistedSplitDirection::Vertical,
                    ratio: 0.5,
                    first: Box::new(PersistedLayoutNode::Leaf {
                        terminal_id: "t-1".to_string(),
                    }),
                    second: Box::new(PersistedLayoutNode::Leaf {
                        terminal_id: "t-2".to_string(),
                    }),
                },
            }],
        };

        let json = serde_json::to_string(&state).expect("serialization should succeed");
        let decoded: PersistedSessionState =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(decoded, state);
    }

    #[test]
    fn merge_filters_stale_terminals_from_layout_and_focus() {
        let persisted = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: false,
            settings_open: true,
            settings_tab: "shortcuts".to_string(),
            font_size: 13.0,
            next_workspace_num: 8,
            active_workspace_id: Some("w-2".to_string()),
            active_terminal_id: Some("t-3".to_string()),
            workspaces: vec![
                PersistedWorkspaceState {
                    id: "w-1".to_string(),
                    name: "One".to_string(),
                    folder_path: ".".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-2".to_string(),
                    layout: PersistedLayoutNode::Split {
                        direction: PersistedSplitDirection::Horizontal,
                        ratio: 0.5,
                        first: Box::new(PersistedLayoutNode::Leaf {
                            terminal_id: "t-1".to_string(),
                        }),
                        second: Box::new(PersistedLayoutNode::Leaf {
                            terminal_id: "t-2".to_string(),
                        }),
                    },
                },
                PersistedWorkspaceState {
                    id: "w-2".to_string(),
                    name: "Two".to_string(),
                    folder_path: ".".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-3".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-3".to_string(),
                    },
                },
            ],
        };

        let live_sessions = vec!["t-1".to_string(), "t-4".to_string()];
        let merged = merge_with_live_sessions(&persisted, &live_sessions);

        assert_eq!(merged.workspaces.len(), 1);
        let workspace = &merged.workspaces[0];
        assert_eq!(workspace.id, "w-1");
        assert_eq!(workspace.focused_terminal, "t-1");
        assert_eq!(
            workspace.layout,
            LayoutNode::Leaf {
                terminal_id: "t-1".to_string()
            }
        );
        assert_eq!(merged.active_workspace_id.as_deref(), Some("w-1"));
        assert_eq!(merged.active_terminal_id.as_deref(), Some("t-1"));
        assert_eq!(merged.missing_live_terminal_ids, vec!["t-4".to_string()]);
    }

    #[test]
    fn load_returns_none_for_corrupt_payload() {
        let path = std::env::temp_dir().join(format!(
            "iced-shell-session-corrupt-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, "{not valid json").expect("failed to write corrupt payload");

        let loaded = load_from_path(&path);
        assert!(loaded.is_none());

        let _ = std::fs::remove_file(&path);
    }
}
