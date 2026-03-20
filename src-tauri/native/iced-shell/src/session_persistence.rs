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
    #[serde(default = "default_font_family")]
    pub font_family: String,
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
    pub font_family: String,
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
    /// `None` when no persisted terminal IDs matched live sessions (e.g. after reinstall).
    pub focused_terminal: Option<String>,
    /// `None` when no persisted terminal IDs matched live sessions (e.g. after reinstall).
    pub layout: Option<LayoutNode>,
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

pub fn clear_default_path() -> Result<(), String> {
    clear_path(&default_persistence_path())
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

pub fn clear_path(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to remove {}: {}", path.display(), error)),
    }
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

        let layout = workspace.layout.to_layout_filtered(&live_terminal_ids);

        let (layout, focused_terminal) = match layout {
            Some(layout) => {
                let leaf_ids: Vec<String> = layout
                    .all_leaf_ids()
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect();
                if leaf_ids.is_empty() {
                    // Layout filtered down to nothing — preserve workspace metadata only.
                    (None, None)
                } else {
                    used_terminal_ids.extend(leaf_ids.iter().cloned());
                    let focused = if leaf_ids.iter().any(|id| id == &workspace.focused_terminal) {
                        workspace.focused_terminal.clone()
                    } else {
                        leaf_ids[0].clone()
                    };
                    (Some(layout), Some(focused))
                }
            }
            None => {
                // Bug #619: preserve workspace metadata even when no terminal IDs match.
                (None, None)
            }
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
                    .and_then(|workspace| workspace.focused_terminal.clone())
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
        font_family: persisted.font_family.clone(),
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

fn default_font_family() -> String {
    "Geist Mono".to_string()
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
            font_family: "Geist Mono".to_string(),
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
            font_family: "Geist Mono".to_string(),
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

        // w-1 has live terminal t-1; w-2 lost t-3 but metadata is preserved.
        assert_eq!(merged.workspaces.len(), 2);

        let w1 = &merged.workspaces[0];
        assert_eq!(w1.id, "w-1");
        assert_eq!(w1.focused_terminal.as_deref(), Some("t-1"));
        assert_eq!(
            w1.layout,
            Some(LayoutNode::Leaf {
                terminal_id: "t-1".to_string()
            })
        );

        let w2 = &merged.workspaces[1];
        assert_eq!(w2.id, "w-2");
        assert_eq!(w2.name, "Two");
        assert!(w2.layout.is_none(), "w-2 has no live terminals");
        assert!(w2.focused_terminal.is_none());

        // w-2 was the user's active workspace and is now preserved (with no terminals).
        assert_eq!(merged.active_workspace_id.as_deref(), Some("w-2"));
        // Active terminal falls back to w-2's focused_terminal, which is None.
        // Next fallback: no live terminal in active workspace → None.
        assert_eq!(merged.active_terminal_id.as_deref(), None);
        assert_eq!(merged.missing_live_terminal_ids, vec!["t-4".to_string()]);
    }

    #[test]
    fn vertical_split_direction_round_trips_through_persistence() {
        // Bug #639: vertical splits became horizontal after restart.
        // Full round-trip: LayoutNode → PersistedLayoutNode → JSON → PersistedLayoutNode → LayoutNode
        let original = LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.6,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t-1".to_string(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t-2".to_string(),
            }),
        };

        let persisted = PersistedLayoutNode::from_layout(&original);
        let json = serde_json::to_string(&persisted).expect("serialize");
        let decoded: PersistedLayoutNode = serde_json::from_str(&json).expect("deserialize");

        let live_ids: HashSet<&str> = ["t-1", "t-2"].into_iter().collect();
        let restored = decoded
            .to_layout_filtered(&live_ids)
            .expect("both terminals are live");

        match &restored {
            LayoutNode::Split { direction, ratio, .. } => {
                assert_eq!(*direction, SplitDirection::Vertical, "direction must survive round-trip");
                assert!((ratio - 0.6).abs() < 0.01, "ratio must survive round-trip");
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout after round-trip"),
        }
    }

    #[test]
    fn horizontal_split_direction_round_trips_through_persistence() {
        let original = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t-1".to_string(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t-2".to_string(),
            }),
        };

        let persisted = PersistedLayoutNode::from_layout(&original);
        let json = serde_json::to_string(&persisted).expect("serialize");
        let decoded: PersistedLayoutNode = serde_json::from_str(&json).expect("deserialize");

        let live_ids: HashSet<&str> = ["t-1", "t-2"].into_iter().collect();
        let restored = decoded
            .to_layout_filtered(&live_ids)
            .expect("both terminals are live");

        match &restored {
            LayoutNode::Split { direction, .. } => {
                assert_eq!(*direction, SplitDirection::Horizontal, "direction must survive round-trip");
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout after round-trip"),
        }
    }

    #[test]
    fn nested_split_directions_round_trip_through_merge() {
        // Nested: Horizontal split with a Vertical sub-split.
        // Verify both directions survive the full persist → merge path.
        let persisted = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: true,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 13.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: 2,
            active_workspace_id: Some("w-1".to_string()),
            active_terminal_id: Some("t-1".to_string()),
            workspaces: vec![PersistedWorkspaceState {
                id: "w-1".to_string(),
                name: "Main".to_string(),
                folder_path: ".".to_string(),
                worktree_mode: false,
                focused_terminal: "t-1".to_string(),
                layout: PersistedLayoutNode::Split {
                    direction: PersistedSplitDirection::Horizontal,
                    ratio: 0.5,
                    first: Box::new(PersistedLayoutNode::Leaf {
                        terminal_id: "t-1".to_string(),
                    }),
                    second: Box::new(PersistedLayoutNode::Split {
                        direction: PersistedSplitDirection::Vertical,
                        ratio: 0.4,
                        first: Box::new(PersistedLayoutNode::Leaf {
                            terminal_id: "t-2".to_string(),
                        }),
                        second: Box::new(PersistedLayoutNode::Leaf {
                            terminal_id: "t-3".to_string(),
                        }),
                    }),
                },
            }],
        };

        let live = vec!["t-1".to_string(), "t-2".to_string(), "t-3".to_string()];
        let merged = merge_with_live_sessions(&persisted, &live);

        let ws = &merged.workspaces[0];
        let layout = ws.layout.as_ref().expect("all terminals are live, layout should be Some");
        match layout {
            LayoutNode::Split {
                direction,
                first,
                second,
                ..
            } => {
                assert_eq!(*direction, SplitDirection::Horizontal, "outer direction");
                match second.as_ref() {
                    LayoutNode::Split { direction, .. } => {
                        assert_eq!(*direction, SplitDirection::Vertical, "inner direction");
                    }
                    _ => panic!("inner node should be a split"),
                }
                assert!(matches!(first.as_ref(), LayoutNode::Leaf { terminal_id } if terminal_id == "t-1"));
            }
            _ => panic!("expected split layout"),
        }
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

    #[test]
    fn merge_preserves_workspace_metadata_when_no_live_sessions_match() {
        // Bug #619: after rebuild+reinstall, daemon has no surviving sessions.
        // merge_with_live_sessions is called with empty live_session_ids,
        // which causes ALL workspace metadata (names, folder paths) to be
        // dropped. The app then falls back to a single "Workspace 1".
        //
        // Expected: workspace metadata (id, name, folder_path, worktree_mode)
        // should survive even when all terminals are gone, so the app can
        // recreate terminals in the correct workspace structure.
        let persisted = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: true,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 14.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: 4,
            active_workspace_id: Some("w-dev".to_string()),
            active_terminal_id: Some("t-old-3".to_string()),
            workspaces: vec![
                PersistedWorkspaceState {
                    id: "w-default".to_string(),
                    name: "Workspace 1".to_string(),
                    folder_path: "C:\\Users\\dev\\project-a".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-old-1".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-old-1".to_string(),
                    },
                },
                PersistedWorkspaceState {
                    id: "w-dev".to_string(),
                    name: "Development".to_string(),
                    folder_path: "C:\\Users\\dev\\project-b".to_string(),
                    worktree_mode: true,
                    focused_terminal: "t-old-3".to_string(),
                    layout: PersistedLayoutNode::Split {
                        direction: PersistedSplitDirection::Vertical,
                        ratio: 0.5,
                        first: Box::new(PersistedLayoutNode::Leaf {
                            terminal_id: "t-old-2".to_string(),
                        }),
                        second: Box::new(PersistedLayoutNode::Leaf {
                            terminal_id: "t-old-3".to_string(),
                        }),
                    },
                },
                PersistedWorkspaceState {
                    id: "w-test".to_string(),
                    name: "Testing".to_string(),
                    folder_path: "C:\\Users\\dev\\project-c".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-old-4".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-old-4".to_string(),
                    },
                },
            ],
        };

        // Reinstall scenario: daemon restarted, zero live sessions match old IDs
        let no_live_sessions: Vec<String> = vec![];
        let merged = merge_with_live_sessions(&persisted, &no_live_sessions);

        // Workspace metadata must survive even when all terminals are dead.
        // The app should be able to recreate terminals in the correct structure.
        assert_eq!(
            merged.workspaces.len(),
            3,
            "all 3 workspace definitions must be preserved after reinstall; \
             got {} (workspace metadata was dropped because no terminal IDs matched)",
            merged.workspaces.len()
        );

        let names: Vec<&str> = merged.workspaces.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Workspace 1", "Development", "Testing"],
            "workspace names must survive reinstall"
        );

        let folder_paths: Vec<&str> = merged
            .workspaces
            .iter()
            .map(|w| w.folder_path.as_str())
            .collect();
        assert_eq!(
            folder_paths,
            vec![
                "C:\\Users\\dev\\project-a",
                "C:\\Users\\dev\\project-b",
                "C:\\Users\\dev\\project-c"
            ],
            "workspace folder paths must survive reinstall"
        );

        assert_eq!(
            merged.workspaces[1].worktree_mode, true,
            "worktree_mode must survive reinstall"
        );

        // All workspaces should have None layout (no live terminals to match).
        for ws in &merged.workspaces {
            assert!(ws.layout.is_none(), "workspace '{}' should have no layout", ws.name);
            assert!(ws.focused_terminal.is_none(), "workspace '{}' should have no focused terminal", ws.name);
        }
    }

    #[test]
    fn merge_preserves_workspace_metadata_when_live_sessions_are_all_new() {
        // Bug #619 variant: daemon has sessions but none match saved terminal IDs.
        // This happens when daemon was restarted and auto-spawned new sessions.
        let persisted = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: false,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 13.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: 3,
            active_workspace_id: Some("w-1".to_string()),
            active_terminal_id: Some("t-old-1".to_string()),
            workspaces: vec![
                PersistedWorkspaceState {
                    id: "w-1".to_string(),
                    name: "Main".to_string(),
                    folder_path: "C:\\dev".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-old-1".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-old-1".to_string(),
                    },
                },
                PersistedWorkspaceState {
                    id: "w-2".to_string(),
                    name: "Backend".to_string(),
                    folder_path: "C:\\dev\\backend".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-old-2".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-old-2".to_string(),
                    },
                },
            ],
        };

        // Daemon restarted and spawned new sessions with different UUIDs
        let new_live_sessions = vec![
            "new-uuid-aaa".to_string(),
            "new-uuid-bbb".to_string(),
        ];
        let merged = merge_with_live_sessions(&persisted, &new_live_sessions);

        // Workspace metadata must survive even though no old terminal IDs match
        assert_eq!(
            merged.workspaces.len(),
            2,
            "workspace metadata must be preserved when live sessions don't match old IDs; \
             got {} workspaces",
            merged.workspaces.len()
        );

        assert_eq!(merged.workspaces[0].name, "Main");
        assert_eq!(merged.workspaces[1].name, "Backend");

        // Workspaces should have None layout (old terminal IDs don't match new ones).
        for ws in &merged.workspaces {
            assert!(ws.layout.is_none(), "workspace '{}' should have no layout", ws.name);
            assert!(ws.focused_terminal.is_none());
        }

        // New sessions that don't belong to any workspace should appear in missing list
        assert_eq!(
            merged.missing_live_terminal_ids.len(),
            2,
            "unmatched new sessions should be in missing_live_terminal_ids"
        );
    }
}
