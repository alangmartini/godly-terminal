use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{SessionInfo, ShellType};

/// Requests sent from the Tauri app to the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    CreateSession {
        id: String,
        shell_type: ShellType,
        cwd: Option<String>,
        rows: u16,
        cols: u16,
        #[serde(default)]
        env: Option<HashMap<String, String>>,
    },
    ListSessions,
    Attach {
        session_id: String,
    },
    Detach {
        session_id: String,
    },
    Write {
        session_id: String,
        data: Vec<u8>,
    },
    Resize {
        session_id: String,
        rows: u16,
        cols: u16,
    },
    CloseSession {
        session_id: String,
    },
    ReadBuffer {
        session_id: String,
    },
    GetLastOutputTime {
        session_id: String,
    },
    SearchBuffer {
        session_id: String,
        text: String,
        strip_ansi: bool,
    },
    /// Read the godly-vt grid (parsed terminal state) for a session.
    ReadGrid {
        session_id: String,
    },
    /// Read rich grid snapshot with per-cell attributes for Canvas2D rendering.
    ReadRichGrid {
        session_id: String,
    },
    /// Read text between two grid positions (for selection/copy).
    ReadGridText {
        session_id: String,
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
    },
    /// Read differential rich grid snapshot (only dirty rows since last read).
    ReadRichGridDiff {
        session_id: String,
    },
    /// Set the scrollback viewport offset for a session.
    /// offset=0 means live view, offset>0 scrolls into history.
    SetScrollback {
        session_id: String,
        offset: usize,
    },
    Ping,
}

/// Responses sent from the daemon to the Tauri app (one per request)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Response {
    Ok,
    Error { message: String },
    SessionCreated { session: SessionInfo },
    SessionList { sessions: Vec<SessionInfo> },
    Pong,
    /// Initial buffer replay when attaching to a session
    Buffer { session_id: String, data: Vec<u8> },
    LastOutputTime { epoch_ms: u64, running: bool },
    SearchResult { found: bool, running: bool },
    /// Grid snapshot from the godly-vt terminal state engine.
    Grid { grid: crate::types::GridData },
    /// Rich grid snapshot with per-cell attributes for Canvas2D rendering.
    RichGrid { grid: crate::types::RichGridData },
    /// Differential rich grid snapshot (only changed rows).
    RichGridDiff { diff: crate::types::RichGridDiff },
    /// Text extracted from grid between two positions.
    GridText { text: String },
}

/// Asynchronous events pushed from the daemon to attached clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    Output { session_id: String, data: Vec<u8> },
    SessionClosed {
        session_id: String,
        /// Process exit code (e.g., 0 for success, non-zero for failure).
        /// None when exit status is unavailable (e.g., session killed externally).
        #[serde(default)]
        exit_code: Option<i64>,
    },
    ProcessChanged { session_id: String, process_name: String },
    GridDiff { session_id: String, diff: crate::types::RichGridDiff },
    Bell { session_id: String },
}

/// Top-level message from daemon to client (can be a response or async event)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum DaemonMessage {
    Response(Response),
    Event(Event),
}
