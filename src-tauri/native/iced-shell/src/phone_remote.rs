use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Child;

// ---------------------------------------------------------------------------
// Persisted preferences
// ---------------------------------------------------------------------------

const PREFS_FILE_NAME: &str = "phone-remote-prefs.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PhoneRemotePreferences {
    pub host: String,
    pub port: u16,
    pub api_key: String,
    pub auto_start: bool,
}

impl Default for PhoneRemotePreferences {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3377,
            api_key: generate_api_key(),
            auto_start: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime status (not persisted)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhoneRemoteStatus {
    Stopped,
    Starting,
    Running,
    Failed(String),
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

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

pub fn load_preferences() -> PhoneRemotePreferences {
    let path = prefs_path();
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => PhoneRemotePreferences::default(),
    }
}

pub fn save_preferences(prefs: &PhoneRemotePreferences) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(prefs) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("Failed to save phone remote prefs: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialize phone remote prefs: {e}"),
    }
}

// ---------------------------------------------------------------------------
// API key generation
// ---------------------------------------------------------------------------

pub fn generate_api_key() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

// ---------------------------------------------------------------------------
// Binary discovery & process management
// ---------------------------------------------------------------------------

pub fn find_remote_binary() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Cannot determine current executable path: {e}"))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Current executable has no parent directory".to_string())?;
    let name = if cfg!(windows) {
        "godly-remote.exe"
    } else {
        "godly-remote"
    };
    let path = exe_dir.join(name);
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "godly-remote binary not found at {}",
            path.display()
        ))
    }
}

pub fn spawn_remote_server(prefs: &PhoneRemotePreferences) -> Result<Child, String> {
    let binary = find_remote_binary()?;
    let mut cmd = std::process::Command::new(&binary);
    cmd.env("GODLY_REMOTE_HOST", &prefs.host)
        .env("GODLY_REMOTE_PORT", prefs.port.to_string())
        .env("GODLY_REMOTE_API_KEY", &prefs.api_key);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn godly-remote: {e}"))
}

pub fn stop_remote_server(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preferences_have_valid_api_key() {
        let prefs = PhoneRemotePreferences::default();
        assert_eq!(prefs.host, "0.0.0.0");
        assert_eq!(prefs.port, 3377);
        assert_eq!(prefs.api_key.len(), 32);
        assert!(!prefs.auto_start);
    }

    #[test]
    fn generate_api_key_produces_unique_32_char_hex() {
        let k1 = generate_api_key();
        let k2 = generate_api_key();
        assert_eq!(k1.len(), 32);
        assert_eq!(k2.len(), 32);
        assert_ne!(k1, k2);
        assert!(k1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn preferences_roundtrip_serde() {
        let prefs = PhoneRemotePreferences {
            host: "127.0.0.1".to_string(),
            port: 4000,
            api_key: "abc123".to_string(),
            auto_start: true,
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: PhoneRemotePreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.host, "127.0.0.1");
        assert_eq!(loaded.port, 4000);
        assert_eq!(loaded.api_key, "abc123");
        assert!(loaded.auto_start);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let json = r#"{"port": 9999}"#;
        let loaded: PhoneRemotePreferences = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.host, "0.0.0.0");
        assert_eq!(loaded.port, 9999);
        assert!(!loaded.auto_start);
        // api_key gets a fresh default (non-empty)
        assert!(!loaded.api_key.is_empty());
    }
}
