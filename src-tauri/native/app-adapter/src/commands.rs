use godly_protocol::{Response, ShellType};

use crate::daemon_client::NativeDaemonClient;

/// Create a terminal session (stub).
pub fn create_terminal(
    _client: &NativeDaemonClient,
    _id: &str,
    _shell_type: ShellType,
    _cwd: Option<&str>,
    _rows: u16,
    _cols: u16,
) -> Result<Response, String> {
    Err("create_terminal stub — full implementation in Phase 1".into())
}

/// Write input to a terminal session (stub).
pub fn write_to_terminal(
    _client: &NativeDaemonClient,
    _session_id: &str,
    _data: &[u8],
) -> Result<(), String> {
    Err("write_to_terminal stub — full implementation in Phase 1".into())
}

/// Resize a terminal session (stub).
pub fn resize_terminal(
    _client: &NativeDaemonClient,
    _session_id: &str,
    _rows: u16,
    _cols: u16,
) -> Result<(), String> {
    Err("resize_terminal stub — full implementation in Phase 1".into())
}
