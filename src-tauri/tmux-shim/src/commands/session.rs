//! Session commands — stub implementation.
//!
//! These will be implemented by another unit. For now, all commands
//! return a "not implemented" error.

use crate::cli::TmuxArgs;

pub fn new_session(_args: &TmuxArgs) -> Result<(), String> {
    Err("new-session: not implemented".to_string())
}

pub fn has_session(_args: &TmuxArgs) -> Result<(), String> {
    Err("has-session: not implemented".to_string())
}

pub fn kill_session(_args: &TmuxArgs) -> Result<(), String> {
    Err("kill-session: not implemented".to_string())
}

pub fn list_sessions(_args: &TmuxArgs) -> Result<(), String> {
    Err("list-sessions: not implemented".to_string())
}
