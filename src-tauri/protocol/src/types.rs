use serde::{Deserialize, Serialize};

/// Shell type matching the existing Tauri app's ShellType
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

/// Information about a daemon-managed session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub shell_type: ShellType,
    pub pid: u32,
    pub rows: u16,
    pub cols: u16,
    pub cwd: Option<String>,
    pub created_at: u64,
    pub attached: bool,
    pub running: bool,
}
