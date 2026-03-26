use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PREFS_FILE_NAME: &str = "desktop-notify-prefs.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopNotifyPreferences {
    /// Whether OS desktop notifications are enabled.
    pub enabled: bool,
    /// If true, the user's choice is remembered across launches.
    pub remembered: bool,
}

impl Default for DesktopNotifyPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            remembered: false,
        }
    }
}

fn prefs_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let directory_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    base.join(directory_name)
        .join("native")
        .join(PREFS_FILE_NAME)
}

pub fn load_preferences() -> DesktopNotifyPreferences {
    let path = prefs_path();
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => DesktopNotifyPreferences::default(),
    }
}

pub fn save_preferences(prefs: &DesktopNotifyPreferences) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(prefs) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("Failed to save desktop notify prefs: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialize desktop notify prefs: {e}"),
    }
}
