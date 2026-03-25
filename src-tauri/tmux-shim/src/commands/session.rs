//! Session management commands (stub).
//!
//! Implemented by the session-commands unit. These stubs allow the binary
//! to compile and report a clear error if invoked before integration.

/// `tmux new-session` — stub
pub fn new_session(_args: &[String]) -> Result<(), String> {
    Err("new-session: not yet implemented".to_string())
}

/// `tmux has-session` — stub
pub fn has_session(_args: &[String]) -> Result<(), String> {
    Err("has-session: not yet implemented".to_string())
}

/// `tmux kill-session` — stub
pub fn kill_session(_args: &[String]) -> Result<(), String> {
    Err("kill-session: not yet implemented".to_string())
}

/// `tmux list-sessions` — stub
pub fn list_sessions(_args: &[String]) -> Result<(), String> {
    Err("list-sessions: not yet implemented".to_string())
}
