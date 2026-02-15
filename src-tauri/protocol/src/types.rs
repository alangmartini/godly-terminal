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

/// Grid snapshot from the godly-vt terminal state engine.
/// Contains the visible terminal content as plain-text rows plus cursor info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridData {
    /// Each element is one row of plain text (no ANSI escapes).
    pub rows: Vec<String>,
    /// Cursor row (0-based).
    pub cursor_row: u16,
    /// Cursor col (0-based).
    pub cursor_col: u16,
    /// Terminal width (columns).
    pub cols: u16,
    /// Terminal height (rows).
    pub num_rows: u16,
    /// Whether the alternate screen is active.
    pub alternate_screen: bool,
}
