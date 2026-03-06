use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const PERSISTENCE_VERSION: u32 = 1;
const PERSISTENCE_FILE_NAME: &str = "iced-shell-scrollback-offsets.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PersistedScrollbackOffsets {
    version: u32,
    offsets: HashMap<String, usize>,
}

fn default_offsets_path() -> PathBuf {
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

pub fn load_offsets() -> HashMap<String, usize> {
    load_offsets_from_path(&default_offsets_path()).unwrap_or_default()
}

pub fn load_offsets_from_path(path: &Path) -> Option<HashMap<String, usize>> {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!(
                "Failed to read scrollback offsets from {}: {}",
                path.display(),
                error
            );
            return None;
        }
    };

    let payload: PersistedScrollbackOffsets = match serde_json::from_str(&json) {
        Ok(payload) => payload,
        Err(error) => {
            log::warn!(
                "Failed to parse scrollback offsets from {}: {}",
                path.display(),
                error
            );
            return None;
        }
    };

    if payload.version != PERSISTENCE_VERSION {
        log::warn!(
            "Ignoring scrollback offsets with unsupported version {} (expected {})",
            payload.version,
            PERSISTENCE_VERSION
        );
        return None;
    }

    Some(payload.offsets)
}

pub fn save_offsets(offsets: &HashMap<String, usize>) -> Result<(), String> {
    save_offsets_to_path(&default_offsets_path(), offsets)
}

pub fn save_offsets_to_path(path: &Path, offsets: &HashMap<String, usize>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }

    let payload = PersistedScrollbackOffsets {
        version: PERSISTENCE_VERSION,
        offsets: offsets.clone(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Failed to serialize scrollback offsets: {}", error))?;

    std::fs::write(path, json)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))?;
    Ok(())
}

pub fn prune_offsets_for_live_sessions(
    offsets: &HashMap<String, usize>,
    live_session_ids: &[String],
) -> HashMap<String, usize> {
    let live_ids: HashSet<&str> = live_session_ids.iter().map(String::as_str).collect();
    offsets
        .iter()
        .filter(|(id, _)| live_ids.contains(id.as_str()))
        .map(|(id, offset)| (id.clone(), *offset))
        .collect()
}

pub fn restored_offsets_for_recovered_sessions(
    persisted_offsets: &HashMap<String, usize>,
    recovered_session_ids: &[String],
) -> HashMap<String, usize> {
    recovered_session_ids
        .iter()
        .filter_map(|id| {
            persisted_offsets
                .get(id)
                .copied()
                .map(|offset| (id.clone(), offset))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryFetchAction {
    FetchGrid { session_id: String },
    ScrollFetch { session_id: String, offset: usize },
}

pub fn build_recovery_fetch_plan(
    recovered_session_ids: &[String],
    restored_offsets: &HashMap<String, usize>,
) -> Vec<RecoveryFetchAction> {
    recovered_session_ids
        .iter()
        .map(|id| {
            let offset = restored_offsets.get(id).copied().unwrap_or(0);
            if offset > 0 {
                RecoveryFetchAction::ScrollFetch {
                    session_id: id.clone(),
                    offset,
                }
            } else {
                RecoveryFetchAction::FetchGrid {
                    session_id: id.clone(),
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offsets(entries: &[(&str, usize)]) -> HashMap<String, usize> {
        entries
            .iter()
            .map(|(id, offset)| ((*id).to_string(), *offset))
            .collect()
    }

    fn ids(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|id| (*id).to_string()).collect()
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "scrollback-offsets-{}-{}.json",
            name,
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = temp_path("roundtrip");
        let mut offsets = HashMap::new();
        offsets.insert("session-a".to_string(), 17);
        offsets.insert("session-b".to_string(), 0);

        save_offsets_to_path(&path, &offsets).expect("save should succeed");
        let loaded = load_offsets_from_path(&path).expect("offsets should load");
        assert_eq!(loaded, offsets);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_rejects_unsupported_version() {
        let path = temp_path("version");
        let payload = PersistedScrollbackOffsets {
            version: 99,
            offsets: HashMap::new(),
        };
        let json = serde_json::to_string(&payload).expect("serialize should succeed");
        std::fs::write(&path, json).expect("write should succeed");

        assert!(load_offsets_from_path(&path).is_none());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_returns_none_for_corrupt_payload() {
        let path = temp_path("corrupt");
        std::fs::write(&path, "{not valid json").expect("write should succeed");

        assert!(load_offsets_from_path(&path).is_none());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn build_recovery_fetch_plan_distinguishes_grid_and_scroll_fetch() {
        let recovered_ids = ids(&["session-a", "session-b", "session-c"]);
        let restored_offsets = offsets(&[("session-a", 0), ("session-b", 42)]);

        let plan = build_recovery_fetch_plan(&recovered_ids, &restored_offsets);

        assert_eq!(
            plan,
            vec![
                RecoveryFetchAction::FetchGrid {
                    session_id: "session-a".to_string()
                },
                RecoveryFetchAction::ScrollFetch {
                    session_id: "session-b".to_string(),
                    offset: 42
                },
                RecoveryFetchAction::FetchGrid {
                    session_id: "session-c".to_string()
                }
            ]
        );
    }

    #[test]
    fn restored_offsets_for_recovered_sessions_ignores_stale_ids() {
        let persisted_offsets = offsets(&[("session-a", 3), ("session-stale", 99)]);
        let recovered_ids = ids(&["session-a"]);

        let restored = restored_offsets_for_recovered_sessions(&persisted_offsets, &recovered_ids);

        assert_eq!(restored, offsets(&[("session-a", 3)]));
    }

    #[test]
    fn prune_offsets_for_live_sessions_removes_non_live_entries() {
        let persisted_offsets = offsets(&[("session-a", 1), ("session-b", 2), ("session-c", 0)]);
        let live_ids = ids(&["session-a", "session-c"]);

        let pruned = prune_offsets_for_live_sessions(&persisted_offsets, &live_ids);

        assert_eq!(pruned, offsets(&[("session-a", 1), ("session-c", 0)]));
    }
}
