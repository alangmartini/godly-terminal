//! Session persistence: save/restore workspace layout on exit/startup.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PERSISTENCE_FILE: &str = "godly-shell-session.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub version: u32,
    pub font_size: f32,
    pub sidebar_visible: bool,
    pub active_session_id: Option<String>,
    pub session_ids: Vec<String>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: 1,
            font_size: 14.0,
            sidebar_visible: false,
            active_session_id: None,
            session_ids: Vec::new(),
        }
    }
}

fn persistence_path() -> Option<PathBuf> {
    dirs_next().map(|d| d.join(PERSISTENCE_FILE))
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("com.godly.terminal"))
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .ok()
            .map(|p| PathBuf::from(p).join(".config/godly-terminal"))
    }
}

pub fn load() -> PersistedState {
    let Some(path) = persistence_path() else {
        return PersistedState::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => PersistedState::default(),
    }
}

pub fn save(state: &PersistedState) {
    let Some(path) = persistence_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::error!("Failed to save session: {e}");
            }
        }
        Err(e) => log::error!("Failed to serialize session: {e}"),
    }
}
