use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::split_pane::{LayoutNode, SplitDirection};

pub const PERSISTENCE_VERSION: u32 = 1;
pub const AUTOSAVE_INTERVAL_SECS: u64 = 60;
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
    /// Worktree paths associated with terminal sessions (terminal_id → worktree_path).
    #[serde(default)]
    pub terminal_worktree_paths: HashMap<String, String>,
    /// Terminal IDs whose worktree_path is actually a clone (not a git worktree).
    #[serde(default)]
    pub terminal_clone_ids: HashSet<String>,
    /// Maps every terminal to its workspace (terminal_id → workspace_id).
    /// The layout tree only stores the *visible* terminals; background tabs
    /// are tracked here so they survive a restart without being turned into
    /// split panes.
    #[serde(default)]
    pub terminal_workspace_assignments: HashMap<String, String>,
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
    /// Worktree paths for terminals that survived the merge.
    pub terminal_worktree_paths: HashMap<String, String>,
    /// Terminal IDs whose worktree_path is actually a clone (not a git worktree).
    pub terminal_clone_ids: HashSet<String>,
    /// Workspace assignments for live terminals that are NOT in any layout
    /// (i.e. background tabs). Maps terminal_id → workspace_id.
    pub missing_terminal_workspace_assignments: HashMap<String, String>,
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

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

pub fn load_from_path(path: &Path) -> Option<PersistedSessionState> {
    // Try the primary file first.
    if let Some(state) = try_load_from_path(path) {
        return Some(state);
    }

    // Primary file is missing, empty, or corrupted — try the backup.
    // This recovers from crashes that corrupt the primary file mid-write.
    let backup = backup_path(path);
    if let Some(state) = try_load_from_path(&backup) {
        log::warn!(
            "Primary session file {} is corrupt/missing — recovered from backup {}",
            path.display(),
            backup.display()
        );
        // Restore the backup as the primary so future loads succeed directly.
        if let Err(e) = std::fs::copy(&backup, path) {
            log::warn!("Failed to restore backup to primary: {}", e);
        }
        return Some(state);
    }

    None
}

fn try_load_from_path(path: &Path) -> Option<PersistedSessionState> {
    let json = match std::fs::read_to_string(path) {
        Ok(json) if json.is_empty() => {
            log::warn!(
                "Session file {} is empty (likely corrupted by crash during write)",
                path.display()
            );
            return None;
        }
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

    // Atomic write: write to a temp file, then rename over the target.
    // This prevents file corruption if the process crashes mid-write.
    // Without this, std::fs::write truncates the file to 0 bytes before
    // writing, so a crash between truncation and write completion leaves
    // an empty/corrupt file and all workspace state is lost.
    let tmp_path = path.with_extension("json.tmp");

    std::fs::write(&tmp_path, &json)
        .map_err(|error| format!("Failed to write {}: {}", tmp_path.display(), error))?;

    // Back up the current file before replacing it. If the rename below
    // fails or the process crashes mid-rename, the backup preserves the
    // previous good state.
    if path.exists() {
        let bak_path = backup_path(path);
        if let Err(e) = std::fs::copy(path, &bak_path) {
            log::warn!("Failed to create session backup: {}", e);
            // Non-fatal — proceed with the rename.
        }
    }

    // Rename the temp file over the target. On NTFS this is close to
    // atomic — the target is either the old file or the new file, never
    // a truncated/empty file.
    std::fs::rename(&tmp_path, path).map_err(|error| {
        format!(
            "Failed to rename {} -> {}: {}",
            tmp_path.display(),
            path.display(),
            error
        )
    })?;

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

    let missing_live_terminal_ids: Vec<String> = live_session_ids
        .iter()
        .filter(|terminal_id| !used_terminal_ids.contains(terminal_id.as_str()))
        .cloned()
        .collect();

    // Filter worktree paths to only include live terminals.
    let terminal_worktree_paths: HashMap<String, String> = persisted
        .terminal_worktree_paths
        .iter()
        .filter(|(id, _)| live_terminal_ids.contains(id.as_str()))
        .map(|(id, path)| (id.clone(), path.clone()))
        .collect();

    // Filter clone IDs to only include live terminals.
    let terminal_clone_ids: HashSet<String> = persisted
        .terminal_clone_ids
        .iter()
        .filter(|id| live_terminal_ids.contains(id.as_str()))
        .cloned()
        .collect();

    // Build workspace assignments for missing live terminals so the caller
    // can add them as background tabs rather than layout splits.
    let workspace_ids_in_merged: HashSet<&str> =
        workspaces.iter().map(|ws| ws.id.as_str()).collect();
    let missing_terminal_workspace_assignments: HashMap<String, String> = missing_live_terminal_ids
        .iter()
        .filter_map(|tid| {
            persisted
                .terminal_workspace_assignments
                .get(tid)
                .filter(|ws_id| workspace_ids_in_merged.contains(ws_id.as_str()))
                .map(|ws_id| (tid.clone(), ws_id.clone()))
        })
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
        terminal_worktree_paths,
        terminal_clone_ids,
        missing_terminal_workspace_assignments,
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
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
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
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
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
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
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
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
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
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
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

    #[test]
    fn merge_returns_workspace_assignments_for_background_tabs() {
        // Bug: multiple tabs in one workspace get restored as splits instead
        // of tabs. Root cause: the layout tree only stores the *visible*
        // terminal; background tabs have no persisted workspace association.
        //
        // This test verifies that terminal_workspace_assignments survives
        // the merge so the caller can add background tabs correctly.
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
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            // Three terminals, but only t-1 is in the layout (visible).
            // t-2 and t-3 are background tabs in workspace w-1.
            terminal_workspace_assignments: HashMap::from([
                ("t-1".to_string(), "w-1".to_string()),
                ("t-2".to_string(), "w-1".to_string()),
                ("t-3".to_string(), "w-1".to_string()),
            ]),
            workspaces: vec![PersistedWorkspaceState {
                id: "w-1".to_string(),
                name: "Main".to_string(),
                folder_path: ".".to_string(),
                worktree_mode: false,
                focused_terminal: "t-1".to_string(),
                layout: PersistedLayoutNode::Leaf {
                    terminal_id: "t-1".to_string(),
                },
            }],
        };

        // All three terminals are still alive in the daemon.
        let live = vec![
            "t-1".to_string(),
            "t-2".to_string(),
            "t-3".to_string(),
        ];
        let merged = merge_with_live_sessions(&persisted, &live);

        // t-1 is in the layout; t-2 and t-3 are missing from any layout.
        assert_eq!(
            merged.workspaces[0].layout,
            Some(LayoutNode::Leaf {
                terminal_id: "t-1".to_string()
            })
        );
        assert_eq!(
            merged.missing_live_terminal_ids,
            vec!["t-2".to_string(), "t-3".to_string()],
        );

        // The workspace assignments tell the caller where the background
        // tabs belong — they must NOT be turned into layout splits.
        assert_eq!(
            merged.missing_terminal_workspace_assignments.get("t-2"),
            Some(&"w-1".to_string()),
            "t-2 should be assigned to w-1"
        );
        assert_eq!(
            merged.missing_terminal_workspace_assignments.get("t-3"),
            Some(&"w-1".to_string()),
            "t-3 should be assigned to w-1"
        );
    }

    #[test]
    fn merge_ignores_workspace_assignments_for_dead_workspaces() {
        // If a terminal's persisted workspace no longer exists in the merged
        // output, the assignment should be dropped (truly orphaned).
        let persisted = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: false,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 13.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: 2,
            active_workspace_id: Some("w-1".to_string()),
            active_terminal_id: Some("t-1".to_string()),
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::from([
                ("t-1".to_string(), "w-1".to_string()),
                ("t-2".to_string(), "w-gone".to_string()), // workspace doesn't exist
            ]),
            workspaces: vec![PersistedWorkspaceState {
                id: "w-1".to_string(),
                name: "Main".to_string(),
                folder_path: ".".to_string(),
                worktree_mode: false,
                focused_terminal: "t-1".to_string(),
                layout: PersistedLayoutNode::Leaf {
                    terminal_id: "t-1".to_string(),
                },
            }],
        };

        let live = vec!["t-1".to_string(), "t-2".to_string()];
        let merged = merge_with_live_sessions(&persisted, &live);

        assert_eq!(merged.missing_live_terminal_ids, vec!["t-2".to_string()]);
        assert!(
            merged.missing_terminal_workspace_assignments.get("t-2").is_none(),
            "t-2's assignment to non-existent workspace should be dropped"
        );
    }

    #[test]
    fn workspace_assignments_round_trip_through_serialization() {
        let state = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: true,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 13.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: 2,
            active_workspace_id: Some("w-1".to_string()),
            active_terminal_id: Some("t-1".to_string()),
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::from([
                ("t-1".to_string(), "w-1".to_string()),
                ("t-2".to_string(), "w-1".to_string()),
            ]),
            workspaces: vec![PersistedWorkspaceState {
                id: "w-1".to_string(),
                name: "Main".to_string(),
                folder_path: ".".to_string(),
                worktree_mode: false,
                focused_terminal: "t-1".to_string(),
                layout: PersistedLayoutNode::Leaf {
                    terminal_id: "t-1".to_string(),
                },
            }],
        };

        let json = serde_json::to_string(&state).expect("serialize");
        let decoded: PersistedSessionState =
            serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            decoded.terminal_workspace_assignments,
            state.terminal_workspace_assignments,
            "workspace assignments must survive serialization round-trip"
        );
    }

    #[test]
    fn merge_preserves_empty_workspaces_when_some_survive_with_live_sessions() {
        // Regression: when daemon survives a rebuild but some PTY sessions die,
        // only running sessions are recovered. Workspaces whose single terminal
        // died have layout=None after merge. The caller (apply_init_result) must
        // create fresh sessions for these instead of dropping them.
        //
        // Scenario: 3 workspaces. "godly-terminal" has live session t-1,
        // "typesense" has dead session t-2, "mercadopago" has live session t-3.
        let persisted = PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: true,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 14.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: 4,
            active_workspace_id: Some("w-godly".to_string()),
            active_terminal_id: Some("t-1".to_string()),
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
            workspaces: vec![
                PersistedWorkspaceState {
                    id: "w-godly".to_string(),
                    name: "godly-terminal".to_string(),
                    folder_path: "C:\\dev\\godly-terminal".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-1".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-1".to_string(),
                    },
                },
                PersistedWorkspaceState {
                    id: "w-typesense".to_string(),
                    name: "typesense".to_string(),
                    folder_path: "C:\\dev\\typesense".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-2".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-2".to_string(),
                    },
                },
                PersistedWorkspaceState {
                    id: "w-mercadopago".to_string(),
                    name: "MercadoPago".to_string(),
                    folder_path: "C:\\dev\\MercadoPago".to_string(),
                    worktree_mode: false,
                    focused_terminal: "t-3".to_string(),
                    layout: PersistedLayoutNode::Leaf {
                        terminal_id: "t-3".to_string(),
                    },
                },
            ],
        };

        // Only t-1 and t-3 survived (t-2's PTY exited).
        let live_sessions = vec!["t-1".to_string(), "t-3".to_string()];
        let merged = merge_with_live_sessions(&persisted, &live_sessions);

        // All 3 workspaces must be preserved.
        assert_eq!(
            merged.workspaces.len(),
            3,
            "all 3 workspaces must survive partial session loss"
        );

        // godly-terminal: layout survives with t-1.
        let ws_godly = &merged.workspaces[0];
        assert_eq!(ws_godly.name, "godly-terminal");
        assert!(ws_godly.layout.is_some(), "godly-terminal should keep its layout");

        // typesense: layout is None (t-2 died), but metadata survives.
        let ws_typesense = &merged.workspaces[1];
        assert_eq!(ws_typesense.name, "typesense");
        assert_eq!(ws_typesense.folder_path, "C:\\dev\\typesense");
        assert!(ws_typesense.layout.is_none(), "typesense should have no layout (terminal died)");

        // MercadoPago: layout survives with t-3.
        let ws_mp = &merged.workspaces[2];
        assert_eq!(ws_mp.name, "MercadoPago");
        assert!(ws_mp.layout.is_some(), "MercadoPago should keep its layout");

        // Critical: no orphan terminals exist (all live sessions match their workspaces).
        // This means apply_init_result MUST create a new session for typesense
        // rather than silently dropping it.
        assert!(
            merged.missing_live_terminal_ids.is_empty(),
            "no orphan terminals should exist when all live sessions match their workspaces"
        );
    }

    #[test]
    fn old_json_without_workspace_assignments_deserializes_with_empty_default() {
        // Backwards compatibility: JSON saved before this field existed
        // should deserialize with an empty HashMap.
        let json = r#"{
            "version": 1,
            "sidebar_visible": true,
            "settings_open": false,
            "settings_tab": "shortcuts",
            "font_size": 13.0,
            "font_family": "Geist Mono",
            "next_workspace_num": 2,
            "active_workspace_id": "w-1",
            "active_terminal_id": "t-1",
            "workspaces": [{
                "id": "w-1",
                "name": "Main",
                "folder_path": ".",
                "worktree_mode": false,
                "focused_terminal": "t-1",
                "layout": {"type": "leaf", "terminal_id": "t-1"}
            }]
        }"#;

        let state: PersistedSessionState =
            serde_json::from_str(json).expect("deserialize old format");
        assert!(
            state.terminal_workspace_assignments.is_empty(),
            "missing field should default to empty HashMap"
        );
    }

    /// Helper: create a temp path unique to the calling test.
    fn test_persistence_path(suffix: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "godly-test-{}-{}.json",
                std::process::id(),
                suffix
            ))
    }

    /// Helper: build a minimal persisted workspace.
    fn make_workspace(id: &str, name: &str, folder: &str) -> PersistedWorkspaceState {
        PersistedWorkspaceState {
            id: id.to_string(),
            name: name.to_string(),
            folder_path: folder.to_string(),
            worktree_mode: false,
            focused_terminal: format!("t-{}", id),
            layout: PersistedLayoutNode::Leaf {
                terminal_id: format!("t-{}", id),
            },
        }
    }

    /// Helper: build a minimal persisted session state from workspaces.
    fn make_session(workspaces: Vec<PersistedWorkspaceState>) -> PersistedSessionState {
        PersistedSessionState {
            version: PERSISTENCE_VERSION,
            sidebar_visible: true,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            font_size: 14.0,
            font_family: "Geist Mono".to_string(),
            next_workspace_num: (workspaces.len() as u32) + 1,
            active_workspace_id: workspaces.first().map(|w| w.id.clone()),
            active_terminal_id: workspaces.first().map(|w| w.focused_terminal.clone()),
            terminal_worktree_paths: HashMap::new(),
            terminal_clone_ids: HashSet::new(),
            terminal_workspace_assignments: HashMap::new(),
            workspaces,
        }
    }

    #[test]
    fn deleted_workspace_survives_crash_with_immediate_persist() {
        // Bug #619 regression: delete_workspace() must call persist_session_state()
        // so that the deletion is immediately written to disk. Without it, a crash
        // within the 60-second autosave window restores the deleted workspace.
        //
        // This test verifies the fix: after deletion, the file is updated and the
        // deleted workspace does NOT reappear on crash recovery.
        let path = test_persistence_path("delete-persist");
        let _cleanup = scopeguard(path.clone());

        // Step 1: app state has 5 workspaces, saved to disk.
        let mut state = make_session(vec![
            make_workspace("w-1", "Workspace 1", "C:\\Users\\dev"),
            make_workspace("w-2", "godly-terminal", "C:\\Users\\dev\\godly"),
            make_workspace("w-3", "Mercado Pago", "C:\\Users\\dev\\mp"),
            make_workspace("w-4", "Backend API", "C:\\Users\\dev\\api"),
            make_workspace("w-5", "Design System", "C:\\Users\\dev\\design"),
        ]);
        save_to_path(&path, &state).expect("initial save");

        // Step 2: user deletes "Workspace 1".
        // Fix: delete_workspace() now calls persist_session_state(), which
        // saves the updated state immediately.
        state.workspaces.retain(|w| w.id != "w-1");
        save_to_path(&path, &state).expect("persist after delete");

        // Step 3: app crashes. On restart, load from the file.
        let loaded = load_from_path(&path).expect("load after crash");
        let merged = merge_with_live_sessions(&loaded, &[]);

        // Deleted workspace should NOT be in the restored state.
        let workspace_names: Vec<&str> =
            merged.workspaces.iter().map(|w| w.name.as_str()).collect();
        assert!(
            !workspace_names.contains(&"Workspace 1"),
            "Bug #619 regression: deleted workspace 'Workspace 1' reappears after crash. \
             persist_session_state() should be called after delete_workspace(). \
             Restored workspaces: {:?}",
            workspace_names
        );
        assert_eq!(
            merged.workspaces.len(),
            4,
            "4 workspaces should survive (5 original - 1 deleted)"
        );
    }

    #[test]
    fn created_workspaces_survive_crash_with_immediate_persist() {
        // Bug #619 regression: create_new_workspace() must call persist_session_state()
        // so that newly created workspaces are immediately written to disk. Without it,
        // a crash within the 60-second autosave window loses the new workspaces.
        //
        // This test verifies the fix: after creation, the file is updated and
        // new workspaces survive crash recovery.
        let path = test_persistence_path("create-persist");
        let _cleanup = scopeguard(path.clone());

        // Step 1: app state has 3 workspaces, saved to disk.
        let mut state = make_session(vec![
            make_workspace("w-1", "Workspace 1", "C:\\Users\\dev"),
            make_workspace("w-2", "godly-terminal", "C:\\Users\\dev\\godly"),
            make_workspace("w-3", "Mercado Pago", "C:\\Users\\dev\\mp"),
        ]);
        save_to_path(&path, &state).expect("initial save");

        // Step 2: user creates 2 new workspaces.
        // Fix: create_new_workspace() now calls persist_session_state(),
        // which saves the updated state immediately after each creation.
        state
            .workspaces
            .push(make_workspace("w-4", "Backend API", "C:\\Users\\dev\\api"));
        save_to_path(&path, &state).expect("persist after create #1");
        state.workspaces.push(make_workspace(
            "w-5",
            "Design System",
            "C:\\Users\\dev\\design",
        ));
        save_to_path(&path, &state).expect("persist after create #2");

        // Step 3: app crashes. On restart, load from the file.
        let loaded = load_from_path(&path).expect("load after crash");
        let merged = merge_with_live_sessions(&loaded, &[]);

        // All 5 workspaces should be restored.
        assert_eq!(
            merged.workspaces.len(),
            5,
            "Bug #619 regression: workspaces created before crash are lost. \
             persist_session_state() should be called after create_new_workspace(). \
             Expected 5, got {}. Restored: {:?}",
            merged.workspaces.len(),
            merged
                .workspaces
                .iter()
                .map(|w| w.name.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn crash_after_mixed_mutations_restores_correct_state() {
        // Bug #619 regression (full scenario): user deletes "Workspace 1",
        // creates new workspaces, then the app crashes.
        //
        // This matches the exact user report: "once again there was a crash/rebuild
        // and the only workspaces are workspace 1, godly-terminal and Mercado Pago
        // and none other of the ones we created. I had also deleted workspace 1."
        //
        // Fix: persist_session_state() is called after every mutation, so the
        // session file always reflects the current state.
        let path = test_persistence_path("mixed-persist");
        let _cleanup = scopeguard(path.clone());

        // Step 1: initial state with 3 workspaces.
        let mut state = make_session(vec![
            make_workspace("w-1", "Workspace 1", "C:\\Users\\dev"),
            make_workspace("w-2", "godly-terminal", "C:\\Users\\dev\\godly"),
            make_workspace("w-3", "Mercado Pago", "C:\\Users\\dev\\mp"),
        ]);
        save_to_path(&path, &state).expect("initial save");

        // Step 2: user deletes "Workspace 1" → persist_session_state() called.
        state.workspaces.retain(|w| w.id != "w-1");
        save_to_path(&path, &state).expect("persist after delete");

        // Step 3: user creates "Backend API" → persist_session_state() called.
        state
            .workspaces
            .push(make_workspace("w-4", "Backend API", "C:\\Users\\dev\\api"));
        save_to_path(&path, &state).expect("persist after create #1");

        // Step 4: user creates "Design System" → persist_session_state() called.
        state.workspaces.push(make_workspace(
            "w-5",
            "Design System",
            "C:\\Users\\dev\\design",
        ));
        save_to_path(&path, &state).expect("persist after create #2");

        // Step 5: app crashes. On restart, load and merge.
        let loaded = load_from_path(&path).expect("load after crash");
        let merged = merge_with_live_sessions(&loaded, &[]);

        let workspace_names: Vec<&str> =
            merged.workspaces.iter().map(|w| w.name.as_str()).collect();

        // Deleted workspace should NOT reappear.
        assert!(
            !workspace_names.contains(&"Workspace 1"),
            "Bug #619 regression: deleted 'Workspace 1' reappears after crash. \
             Restored: {:?}",
            workspace_names
        );

        // Newly created workspaces should be present.
        assert!(
            workspace_names.contains(&"Backend API"),
            "Bug #619 regression: 'Backend API' created before crash is missing. \
             Restored: {:?}",
            workspace_names
        );
        assert!(
            workspace_names.contains(&"Design System"),
            "Bug #619 regression: 'Design System' created before crash is missing. \
             Restored: {:?}",
            workspace_names
        );

        // Expected final workspace set.
        assert_eq!(
            workspace_names,
            vec!["godly-terminal", "Mercado Pago", "Backend API", "Design System"],
            "Bug #619 regression: crash after workspace mutations should restore \
             the current state, not stale autosave state. Restored: {:?}",
            workspace_names
        );
    }

    #[test]
    fn atomic_write_creates_backup_and_temp_file_flow() {
        // Verify that save_to_path uses the atomic write pattern:
        // write to .tmp → backup .json to .bak → rename .tmp to .json
        let path = test_persistence_path("atomic-write");
        let _cleanup = scopeguard(path.clone());

        let state_v1 = make_session(vec![
            make_workspace("w-1", "First", "C:\\first"),
        ]);
        save_to_path(&path, &state_v1).expect("first save");
        assert!(path.exists(), "primary file must exist after first save");

        // Second save should create a backup of the first version.
        let state_v2 = make_session(vec![
            make_workspace("w-1", "First", "C:\\first"),
            make_workspace("w-2", "Second", "C:\\second"),
        ]);
        save_to_path(&path, &state_v2).expect("second save");

        let bak_path = path.with_extension("json.bak");
        assert!(bak_path.exists(), "backup file must exist after second save");

        // Primary should have v2 (2 workspaces).
        let loaded = load_from_path(&path).expect("load primary");
        assert_eq!(loaded.workspaces.len(), 2, "primary should have 2 workspaces");

        // Backup should have v1 (1 workspace).
        let backup = try_load_from_path(&bak_path).expect("load backup");
        assert_eq!(backup.workspaces.len(), 1, "backup should have 1 workspace");

        // Temp file should NOT exist (it was renamed to primary).
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists(), "temp file should be cleaned up after rename");

        // Cleanup
        let _ = std::fs::remove_file(&bak_path);
    }

    #[test]
    fn corrupted_primary_recovers_from_backup() {
        // Bug #619: if the process crashes during std::fs::write, the session
        // file can be empty/corrupted. The backup provides recovery.
        let path = test_persistence_path("backup-recovery");
        let _cleanup = scopeguard(path.clone());

        // Save a good state (creates primary and, on second save, backup).
        let state = make_session(vec![
            make_workspace("w-1", "Workspace 1", "C:\\dev"),
            make_workspace("w-2", "godly-terminal", "C:\\dev\\godly"),
            make_workspace("w-3", "Mercado Pago", "C:\\dev\\mp"),
        ]);
        save_to_path(&path, &state).expect("first save");
        // Save again to create the backup.
        save_to_path(&path, &state).expect("second save (creates backup)");

        let bak_path = path.with_extension("json.bak");
        assert!(bak_path.exists(), "backup must exist");

        // Simulate crash corruption: primary file becomes empty.
        std::fs::write(&path, "").expect("simulate corruption");

        // Load should recover from backup.
        let recovered = load_from_path(&path).expect(
            "load should recover from backup when primary is corrupted"
        );
        assert_eq!(
            recovered.workspaces.len(),
            3,
            "all 3 workspaces should be recovered from backup"
        );

        // Primary should be restored from backup.
        let primary = try_load_from_path(&path).expect("primary should be restored");
        assert_eq!(primary.workspaces.len(), 3);

        // Cleanup
        let _ = std::fs::remove_file(&bak_path);
    }

    #[test]
    fn truncated_json_recovers_from_backup() {
        // Crash during write can leave partially-written JSON.
        let path = test_persistence_path("truncated-recovery");
        let _cleanup = scopeguard(path.clone());

        let state = make_session(vec![
            make_workspace("w-1", "Workspace 1", "C:\\dev"),
            make_workspace("w-2", "godly-terminal", "C:\\dev\\godly"),
        ]);
        save_to_path(&path, &state).expect("first save");
        save_to_path(&path, &state).expect("second save (creates backup)");

        // Simulate crash mid-write: truncated JSON.
        std::fs::write(&path, r#"{"version": 1, "workspaces": [{"id":"#)
            .expect("simulate truncation");

        let recovered = load_from_path(&path).expect(
            "load should recover from backup when primary is truncated JSON"
        );
        assert_eq!(recovered.workspaces.len(), 2);

        // Cleanup
        let _ = std::fs::remove_file(&path.with_extension("json.bak"));
    }

    /// RAII guard to clean up temp files after test.
    struct ScopeGuard {
        path: PathBuf,
    }
    impl Drop for ScopeGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(self.path.with_extension("json.bak"));
            let _ = std::fs::remove_file(self.path.with_extension("json.tmp"));
        }
    }
    fn scopeguard(path: PathBuf) -> ScopeGuard {
        ScopeGuard { path }
    }
}
