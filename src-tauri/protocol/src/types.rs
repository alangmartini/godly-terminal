use serde::{Deserialize, Serialize};

/// Shell type matching the existing Tauri app's ShellType
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShellType {
    Windows,
    Pwsh,
    Cmd,
    Wsl { distribution: Option<String> },
    Custom { program: String, args: Option<Vec<String>> },
}

impl ShellType {
    /// Human-readable display name (extracts basename for Custom).
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
    /// Current scrollback offset (0 = live view, >0 = scrolled into history).
    #[serde(default)]
    pub scrollback_offset: usize,
    /// Total number of scrollback rows available.
    #[serde(default)]
    pub total_scrollback: usize,
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

/// Differential grid snapshot: only contains rows that changed since last read.
/// When `full_repaint` is true, `dirty_rows` contains ALL rows (same as a full snapshot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichGridDiff {
    /// Only the rows that changed, as (row_index, row_data) pairs.
    pub dirty_rows: Vec<(u16, RichGridRow)>,
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
    /// Current scrollback offset (0 = live view, >0 = scrolled into history).
    #[serde(default)]
    pub scrollback_offset: usize,
    /// Total number of scrollback rows available.
    #[serde(default)]
    pub total_scrollback: usize,
    /// If true, this is effectively a full repaint (all rows included).
    /// The frontend should replace its entire cached grid.
    pub full_repaint: bool,
}

/// Metadata about a running pty-shim process.
/// Persisted as JSON in %APPDATA%/com.godly.terminal/shims/{session-id}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShimMetadata {
    pub session_id: String,
    pub shim_pid: u32,
    pub shim_pipe_name: String,
    pub shell_pid: u32,
    pub shell_type: ShellType,
    pub cwd: Option<String>,
    pub rows: u16,
    pub cols: u16,
    pub created_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_windows_serialization() {
        let shell = ShellType::Windows;
        let json = serde_json::to_string(&shell).unwrap();
        assert_eq!(json, "\"windows\"");
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ShellType::Windows);
    }

    #[test]
    fn test_shell_type_pwsh_serialization() {
        let shell = ShellType::Pwsh;
        let json = serde_json::to_string(&shell).unwrap();
        assert_eq!(json, "\"pwsh\"");
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ShellType::Pwsh);
    }

    #[test]
    fn test_shell_type_cmd_serialization() {
        let shell = ShellType::Cmd;
        let json = serde_json::to_string(&shell).unwrap();
        assert_eq!(json, "\"cmd\"");
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ShellType::Cmd);
    }

    #[test]
    fn test_shell_type_wsl_serialization() {
        let shell = ShellType::Wsl { distribution: Some("Ubuntu".to_string()) };
        let json = serde_json::to_string(&shell).unwrap();
        assert!(json.contains("wsl"));
        assert!(json.contains("Ubuntu"));
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, shell);
    }

    #[test]
    fn test_shell_type_backward_compat() {
        // "windows" JSON from older versions must still deserialize
        let shell: ShellType = serde_json::from_str("\"windows\"").unwrap();
        assert_eq!(shell, ShellType::Windows);
    }

    #[test]
    fn test_shell_type_default() {
        assert_eq!(ShellType::default(), ShellType::Windows);
    }

    #[test]
    fn test_shell_type_custom_serialization() {
        let shell = ShellType::Custom {
            program: "nu.exe".to_string(),
            args: Some(vec!["-l".to_string()]),
        };
        let json = serde_json::to_string(&shell).unwrap();
        assert!(json.contains("custom"));
        assert!(json.contains("nu.exe"));
        assert!(json.contains("-l"));
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, shell);
    }

    #[test]
    fn test_shell_type_custom_no_args() {
        let shell = ShellType::Custom {
            program: "fish".to_string(),
            args: None,
        };
        let json = serde_json::to_string(&shell).unwrap();
        let deserialized: ShellType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, shell);
    }

    #[test]
    fn test_shell_type_display_name() {
        assert_eq!(ShellType::Windows.display_name(), "powershell");
        assert_eq!(ShellType::Pwsh.display_name(), "pwsh");
        assert_eq!(ShellType::Cmd.display_name(), "cmd");
        assert_eq!(
            ShellType::Wsl { distribution: Some("Ubuntu".to_string()) }.display_name(),
            "Ubuntu"
        );
        assert_eq!(
            ShellType::Wsl { distribution: None }.display_name(),
            "wsl"
        );
        assert_eq!(
            ShellType::Custom { program: "C:\\Program Files\\nu\\nu.exe".to_string(), args: None }.display_name(),
            "nu"
        );
        assert_eq!(
            ShellType::Custom { program: "fish".to_string(), args: None }.display_name(),
            "fish"
        );
    }

    #[test]
    fn test_shim_metadata_serialization_roundtrip() {
        let meta = ShimMetadata {
            session_id: "sess-abc-123".to_string(),
            shim_pid: 12345,
            shim_pipe_name: r"\\.\pipe\godly-shim-sess-abc-123".to_string(),
            shell_pid: 67890,
            shell_type: ShellType::Windows,
            cwd: Some("C:\\Users\\test".to_string()),
            rows: 24,
            cols: 80,
            created_at: 1700000000,
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ShimMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.session_id, "sess-abc-123");
        assert_eq!(deserialized.shim_pid, 12345);
        assert_eq!(deserialized.shim_pipe_name, r"\\.\pipe\godly-shim-sess-abc-123");
        assert_eq!(deserialized.shell_pid, 67890);
        assert_eq!(deserialized.shell_type, ShellType::Windows);
        assert_eq!(deserialized.cwd, Some("C:\\Users\\test".to_string()));
        assert_eq!(deserialized.rows, 24);
        assert_eq!(deserialized.cols, 80);
        assert_eq!(deserialized.created_at, 1700000000);
    }

    #[test]
    fn test_shim_metadata_with_wsl_shell() {
        let meta = ShimMetadata {
            session_id: "wsl-sess".to_string(),
            shim_pid: 111,
            shim_pipe_name: r"\\.\pipe\godly-shim-wsl-sess".to_string(),
            shell_pid: 222,
            shell_type: ShellType::Wsl { distribution: Some("Ubuntu".to_string()) },
            cwd: None,
            rows: 30,
            cols: 120,
            created_at: 1700000001,
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ShimMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.shell_type, ShellType::Wsl { distribution: Some("Ubuntu".to_string()) });
        assert_eq!(deserialized.cwd, None);
    }

    #[test]
    fn test_shim_metadata_with_custom_shell() {
        let meta = ShimMetadata {
            session_id: "custom-sess".to_string(),
            shim_pid: 333,
            shim_pipe_name: r"\\.\pipe\godly-shim-custom-sess".to_string(),
            shell_pid: 444,
            shell_type: ShellType::Custom {
                program: "nu.exe".to_string(),
                args: Some(vec!["-l".to_string()]),
            },
            cwd: Some("/home/user".to_string()),
            rows: 50,
            cols: 200,
            created_at: 1700000002,
        };

        let json = serde_json::to_string_pretty(&meta).unwrap();
        let deserialized: ShimMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.session_id, "custom-sess");
        assert_eq!(deserialized.shell_type, ShellType::Custom {
            program: "nu.exe".to_string(),
            args: Some(vec!["-l".to_string()]),
        });
    }
}
