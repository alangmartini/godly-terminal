use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// TTL for cached layout data.
const CACHE_TTL: Duration = Duration::from_secs(5);

/// Layout store key (matches Tauri app's LAYOUT_KEY).
const LAYOUT_KEY: &str = "layout";

/// Shell type (mirrors state/models.rs to avoid coupling remote to Tauri app crate).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellType {
    Windows,
    Pwsh,
    Cmd,
    Wsl { distribution: Option<String> },
    Custom { program: String, args: Option<Vec<String>> },
}

impl ShellType {
    pub fn display_name(&self) -> String {
        match self {
            ShellType::Windows => "powershell".to_string(),
            ShellType::Pwsh => "pwsh".to_string(),
            ShellType::Cmd => "cmd".to_string(),
            ShellType::Wsl { distribution } => {
                distribution.clone().unwrap_or_else(|| "wsl".to_string())
            }
            ShellType::Custom { program, .. } => {
                std::path::Path::new(program)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(program)
                    .to_string()
            }
        }
    }
}

impl Default for ShellType {
    fn default() -> Self {
        ShellType::Windows
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
    pub claude_code_mode: bool,
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

/// Tauri plugin-store format: the layout.json file wraps values in a store object.
/// The actual layout is under the "layout" key.
#[derive(Deserialize)]
struct StoreFile {
    #[serde(flatten)]
    entries: HashMap<String, serde_json::Value>,
}

/// Cached layout reader that avoids filesystem hits on every request.
pub struct LayoutReader {
    cache: Mutex<Option<(Layout, Instant)>>,
    layout_path: PathBuf,
}

impl LayoutReader {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            layout_path: Self::find_layout_path(),
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
        let contents = match std::fs::read_to_string(&self.layout_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("Cannot read layout file {}: {}", self.layout_path.display(), e);
                return Layout::default();
            }
        };

        // Tauri plugin-store saves as { "layout": { ... } }
        let store: StoreFile = match serde_json::from_str(&contents) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse layout store: {}", e);
                return Layout::default();
            }
        };

        match store.entries.get(LAYOUT_KEY) {
            Some(value) => serde_json::from_value(value.clone()).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse layout value: {}", e);
                Layout::default()
            }),
            None => {
                tracing::debug!("No '{}' key in layout store", LAYOUT_KEY);
                Layout::default()
            }
        }
    }

    fn find_layout_path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata)
                .join("com.godly.terminal")
                .join("layout.json")
        } else {
            // Fallback
            PathBuf::from("layout.json")
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
            ShellType::Wsl { distribution: Some("Ubuntu".into()) }.display_name(),
            "Ubuntu"
        );
        assert_eq!(
            ShellType::Wsl { distribution: None }.display_name(),
            "wsl"
        );
        assert_eq!(
            ShellType::Custom { program: "nu.exe".into(), args: None }.display_name(),
            "nu"
        );
    }

    #[test]
    fn parse_layout_from_store_format() {
        let store_json = r#"{
            "layout": {
                "workspaces": [{
                    "id": "ws-1",
                    "name": "Test",
                    "folder_path": "C:\\test",
                    "tab_order": ["t-1"]
                }],
                "terminals": [{
                    "id": "t-1",
                    "workspace_id": "ws-1",
                    "name": "Shell"
                }],
                "active_workspace_id": "ws-1"
            }
        }"#;

        let store: StoreFile = serde_json::from_str(store_json).unwrap();
        let layout: Layout = serde_json::from_value(
            store.entries.get(LAYOUT_KEY).unwrap().clone(),
        ).unwrap();

        assert_eq!(layout.workspaces.len(), 1);
        assert_eq!(layout.workspaces[0].name, "Test");
        assert_eq!(layout.terminals.len(), 1);
        assert_eq!(layout.terminals[0].id, "t-1");
    }

    #[test]
    fn empty_store_returns_default_layout() {
        let store_json = "{}";
        let store: StoreFile = serde_json::from_str(store_json).unwrap();
        assert!(store.entries.get(LAYOUT_KEY).is_none());
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
