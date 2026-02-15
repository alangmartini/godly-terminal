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

/// Rich grid snapshot with per-cell attributes for Canvas2D rendering.
/// Serialized as JSON over Tauri IPC to the frontend renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichGridData {
    /// Per-row cell data with attributes.
    pub rows: Vec<RichGridRow>,
    /// Cursor state.
    pub cursor: CursorState,
    /// Terminal dimensions.
    pub dimensions: GridDimensions,
    /// Whether the alternate screen buffer is active.
    pub alternate_screen: bool,
    /// Whether the cursor should be hidden.
    pub cursor_hidden: bool,
    /// OSC window title, if set.
    pub title: String,
}

/// A single row in the rich grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichGridRow {
    /// Cells in this row.
    pub cells: Vec<RichGridCell>,
    /// Whether this row wraps to the next line.
    pub wrapped: bool,
}

/// A single cell with full attribute information for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichGridCell {
    /// The text content (character(s), may include combining chars).
    pub content: String,
    /// Foreground color as a hex string (e.g. "#cd3131") or "default".
    pub fg: String,
    /// Background color as a hex string (e.g. "#1e1e1e") or "default".
    pub bg: String,
    /// Whether the cell is bold.
    pub bold: bool,
    /// Whether the cell is dim.
    pub dim: bool,
    /// Whether the cell is italic.
    pub italic: bool,
    /// Whether the cell is underlined.
    pub underline: bool,
    /// Whether the cell has inverse video.
    pub inverse: bool,
    /// Whether this cell starts a wide (double-width) character.
    pub wide: bool,
    /// Whether this cell is the continuation of a wide character.
    pub wide_continuation: bool,
}

/// Cursor state for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorState {
    /// Cursor row (0-based, relative to visible area).
    pub row: u16,
    /// Cursor column (0-based).
    pub col: u16,
}

/// Terminal grid dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridDimensions {
    /// Number of visible rows.
    pub rows: u16,
    /// Number of columns.
    pub cols: u16,
}
