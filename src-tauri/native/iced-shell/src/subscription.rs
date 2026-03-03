/// Events forwarded from the daemon bridge I/O thread to the Iced app.
#[derive(Debug, Clone)]
pub enum DaemonEventMsg {
    /// Terminal produced output — grid needs refresh.
    TerminalOutput { session_id: String },
    /// Terminal session closed.
    SessionClosed {
        session_id: String,
        exit_code: Option<i64>,
    },
    /// Process name changed (e.g., shell → vim).
    ProcessChanged {
        session_id: String,
        process_name: String,
    },
    /// Bell character received.
    Bell { session_id: String },
}
