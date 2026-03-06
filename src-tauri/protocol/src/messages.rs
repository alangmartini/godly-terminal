use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{SessionInfo, ShellType};

/// Requests sent from the Tauri app to the daemon
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum Request {
    // --- Session lifecycle ---
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
    CloseSession {
        session_id: String,
    },

    // --- I/O ---
    Write {
        session_id: String,
        data: Vec<u8>,
    },
    Resize {
        session_id: String,
        rows: u16,
        cols: u16,
    },

    // --- Grid/Scrollback ---
    ReadBuffer {
        session_id: String,
    },
    /// Read the godly-vt grid (parsed terminal state) for a session.
    ReadGrid {
        session_id: String,
    },
    /// Read rich grid snapshot with per-cell attributes for Canvas2D rendering.
    ReadRichGrid {
        session_id: String,
    },
    /// Read differential rich grid snapshot (only dirty rows since last read).
    ReadRichGridDiff {
        session_id: String,
    },
    /// Read text between two grid positions (for selection/copy).
    /// Row coordinates are viewport-relative (can be negative for selections
    /// extending above the viewport). scrollback_offset is needed to convert
    /// to absolute buffer positions for multi-screen selections.
    ReadGridText {
        session_id: String,
        start_row: i32,
        start_col: u16,
        end_row: i32,
        end_col: u16,
        scrollback_offset: usize,
    },
    /// Set the scrollback viewport offset for a session.
    /// offset=0 means live view, offset>0 scrolls into history.
    SetScrollback {
        session_id: String,
        offset: usize,
    },
    /// Set scrollback offset AND return the rich grid snapshot in a single
    /// round-trip. Used by the frontend scroll path to halve IPC latency.
    ScrollAndReadRichGrid {
        session_id: String,
        offset: usize,
    },

    // --- Session state ---
    GetLastOutputTime {
        session_id: String,
    },
    SearchBuffer {
        session_id: String,
        text: String,
        strip_ansi: bool,
    },
    /// Pause output streaming for a session (session stays alive, VT parser
    /// keeps running, but no Output/GridDiff events are sent to the client).
    PauseSession {
        session_id: String,
    },
    /// Resume output streaming for a previously paused session.
    ResumeSession {
        session_id: String,
    },

    // --- System ---
    Ping,
}

/// Responses sent from the daemon to the Tauri app (one per request)
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum Response {
    // --- General ---
    Ok,
    Error {
        message: String,
    },

    // --- Session ---
    SessionCreated {
        session: SessionInfo,
    },
    SessionList {
        sessions: Vec<SessionInfo>,
    },

    // --- Buffer/Grid ---
    /// Initial buffer replay when attaching to a session
    Buffer {
        session_id: String,
        data: Vec<u8>,
    },
    /// Grid snapshot from the godly-vt terminal state engine.
    Grid {
        grid: crate::types::GridData,
    },
    /// Rich grid snapshot with per-cell attributes for Canvas2D rendering.
    RichGrid {
        grid: crate::types::RichGridData,
    },
    /// Differential rich grid snapshot (only changed rows).
    RichGridDiff {
        diff: crate::types::RichGridDiff,
    },
    /// Text extracted from grid between two positions.
    GridText {
        text: String,
    },

    // --- Query results ---
    LastOutputTime {
        epoch_ms: u64,
        running: bool,
        #[serde(default)]
        exit_code: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input_expected: Option<bool>,
    },
    SearchResult {
        found: bool,
        running: bool,
    },

    // --- System ---
    Pong,
}

/// Asynchronous events pushed from the daemon to attached clients
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum Event {
    Output {
        session_id: String,
        data: Vec<u8>,
    },
    SessionClosed {
        session_id: String,
        /// Process exit code (e.g., 0 for success, non-zero for failure).
        /// None when exit status is unavailable (e.g., session killed externally).
        #[serde(default)]
        exit_code: Option<i64>,
    },
    ProcessChanged {
        session_id: String,
        process_name: String,
    },
    GridDiff {
        session_id: String,
        diff: crate::types::RichGridDiff,
    },
    Bell {
        session_id: String,
    },
}

/// Top-level message from daemon to client (can be a response or async event)
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "kind")]
pub enum DaemonMessage {
    Response(Response),
    Event(Event),
}

// ── Wire-format envelopes for concurrent IPC ────────────────────────────
//
// These wrapper types carry an optional `request_id` alongside the inner
// Request/DaemonMessage on the wire. They are used only by the frame layer
// (frame.rs) for serialization; the rest of the codebase continues to use
// the plain Request/Response/DaemonMessage types.

/// Wire-format envelope for requests (carries optional request_id for concurrent IPC).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<u32>,
    #[serde(flatten)]
    pub request: Request,
}

/// Wire-format envelope for serializing DaemonMessage with optional request_id.
#[derive(Serialize)]
pub(crate) struct DaemonMessageWriteEnvelope<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<u32>,
    #[serde(flatten)]
    pub message: &'a DaemonMessage,
}

/// Wire-format envelope for deserializing DaemonMessage with optional request_id.
#[derive(Deserialize)]
pub(crate) struct DaemonMessageReadEnvelope {
    #[serde(default)]
    pub request_id: Option<u32>,
    #[serde(flatten)]
    pub message: DaemonMessage,
}

/// Requests sent from the daemon to a pty-shim process
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShimRequest {
    /// Resize the PTY
    Resize { rows: u16, cols: u16 },
    /// Query shim status
    Status,
    /// Tell the shim to shut down gracefully
    Shutdown,
    /// Ask the shim to drain its ring buffer
    DrainBuffer,
}

/// Responses/events sent from a pty-shim to the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShimResponse {
    /// Status information about the shim and its shell
    StatusInfo {
        shell_pid: u32,
        running: bool,
        rows: u16,
        cols: u16,
    },
    /// Shell process exited
    ShellExited { exit_code: Option<i64> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_request_resize_serialization() {
        let req = ShimRequest::Resize { rows: 24, cols: 80 };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"resize\""));
        assert!(json.contains("\"rows\":24"));
        assert!(json.contains("\"cols\":80"));
        let deserialized: ShimRequest = serde_json::from_str(&json).unwrap();
        match deserialized {
            ShimRequest::Resize { rows, cols } => {
                assert_eq!(rows, 24);
                assert_eq!(cols, 80);
            }
            other => panic!("Expected Resize, got {:?}", other),
        }
    }

    #[test]
    fn shim_request_status_serialization() {
        let req = ShimRequest::Status;
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"status\""));
        let deserialized: ShimRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ShimRequest::Status));
    }

    #[test]
    fn shim_request_shutdown_serialization() {
        let req = ShimRequest::Shutdown;
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"shutdown\""));
        let deserialized: ShimRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ShimRequest::Shutdown));
    }

    #[test]
    fn shim_request_drain_buffer_serialization() {
        let req = ShimRequest::DrainBuffer;
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"drain_buffer\""));
        let deserialized: ShimRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ShimRequest::DrainBuffer));
    }

    #[test]
    fn shim_response_status_info_serialization() {
        let resp = ShimResponse::StatusInfo {
            shell_pid: 12345,
            running: true,
            rows: 30,
            cols: 120,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"status_info\""));
        assert!(json.contains("\"shell_pid\":12345"));
        assert!(json.contains("\"running\":true"));
        let deserialized: ShimResponse = serde_json::from_str(&json).unwrap();
        match deserialized {
            ShimResponse::StatusInfo {
                shell_pid,
                running,
                rows,
                cols,
            } => {
                assert_eq!(shell_pid, 12345);
                assert!(running);
                assert_eq!(rows, 30);
                assert_eq!(cols, 120);
            }
            other => panic!("Expected StatusInfo, got {:?}", other),
        }
    }

    #[test]
    fn shim_response_shell_exited_with_code() {
        let resp = ShimResponse::ShellExited { exit_code: Some(1) };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"shell_exited\""));
        assert!(json.contains("\"exit_code\":1"));
        let deserialized: ShimResponse = serde_json::from_str(&json).unwrap();
        match deserialized {
            ShimResponse::ShellExited { exit_code } => {
                assert_eq!(exit_code, Some(1));
            }
            other => panic!("Expected ShellExited, got {:?}", other),
        }
    }

    #[test]
    fn last_output_time_with_exit_code_roundtrip() {
        let resp = Response::LastOutputTime {
            epoch_ms: 1700000000000,
            running: false,
            exit_code: Some(1),
            input_expected: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"exit_code\":1"));
        assert!(!json.contains("input_expected"));
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        match deserialized {
            Response::LastOutputTime {
                epoch_ms,
                running,
                exit_code,
                input_expected,
            } => {
                assert_eq!(epoch_ms, 1700000000000);
                assert!(!running);
                assert_eq!(exit_code, Some(1));
                assert_eq!(input_expected, None);
            }
            other => panic!("Expected LastOutputTime, got {:?}", other),
        }
    }

    #[test]
    fn last_output_time_without_exit_code_backward_compat() {
        // Simulate an older daemon that doesn't send exit_code or input_expected
        let json = r#"{"type":"LastOutputTime","epoch_ms":1700000000000,"running":true}"#;
        let deserialized: Response = serde_json::from_str(json).unwrap();
        match deserialized {
            Response::LastOutputTime {
                epoch_ms,
                running,
                exit_code,
                input_expected,
            } => {
                assert_eq!(epoch_ms, 1700000000000);
                assert!(running);
                assert_eq!(exit_code, None);
                assert_eq!(input_expected, None);
            }
            other => panic!("Expected LastOutputTime, got {:?}", other),
        }
    }

    #[test]
    fn last_output_time_none_exit_code_roundtrip() {
        let resp = Response::LastOutputTime {
            epoch_ms: 1700000000000,
            running: true,
            exit_code: None,
            input_expected: Some(true),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"input_expected\":true"));
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        match deserialized {
            Response::LastOutputTime {
                exit_code,
                input_expected,
                ..
            } => {
                assert_eq!(exit_code, None);
                assert_eq!(input_expected, Some(true));
            }
            other => panic!("Expected LastOutputTime, got {:?}", other),
        }
    }

    #[test]
    fn shim_response_shell_exited_without_code() {
        let resp = ShimResponse::ShellExited { exit_code: None };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: ShimResponse = serde_json::from_str(&json).unwrap();
        match deserialized {
            ShimResponse::ShellExited { exit_code } => {
                assert_eq!(exit_code, None);
            }
            other => panic!("Expected ShellExited, got {:?}", other),
        }
    }

    // ── RequestEnvelope tests ───────────────────────────────────────────

    #[test]
    fn request_envelope_with_id_roundtrip() {
        let envelope = RequestEnvelope {
            request_id: Some(42),
            request: Request::Ping,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("\"request_id\":42"));
        assert!(json.contains("\"type\":\"Ping\""));
        let deserialized: RequestEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.request_id, Some(42));
        assert!(matches!(deserialized.request, Request::Ping));
    }

    #[test]
    fn request_envelope_without_id_backward_compat() {
        let json = r#"{"type":"Ping"}"#;
        let deserialized: RequestEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.request_id, None);
        assert!(matches!(deserialized.request, Request::Ping));
    }

    #[test]
    fn request_envelope_none_id_omits_field() {
        let envelope = RequestEnvelope {
            request_id: None,
            request: Request::Ping,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(!json.contains("request_id"));
    }

    #[test]
    fn daemon_message_envelope_response_with_id() {
        let msg = DaemonMessage::Response(Response::Pong);
        let envelope = DaemonMessageWriteEnvelope {
            request_id: Some(7),
            message: &msg,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("\"request_id\":7"));
        assert!(json.contains("\"kind\":\"Response\""));
        assert!(json.contains("\"type\":\"Pong\""));

        let read_env: DaemonMessageReadEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(read_env.request_id, Some(7));
        assert!(matches!(
            read_env.message,
            DaemonMessage::Response(Response::Pong)
        ));
    }

    #[test]
    fn daemon_message_envelope_backward_compat() {
        let json = r#"{"kind":"Response","type":"Pong"}"#;
        let read_env: DaemonMessageReadEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(read_env.request_id, None);
        assert!(matches!(
            read_env.message,
            DaemonMessage::Response(Response::Pong)
        ));
    }
}
