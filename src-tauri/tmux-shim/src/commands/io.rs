//! I/O commands — stub implementation.
//!
//! These will be implemented by another unit. For now, all commands
//! return a "not implemented" error.

use crate::cli::TmuxArgs;

pub fn send_keys(_args: &TmuxArgs) -> Result<(), String> {
    Err("send-keys: not implemented".to_string())
}

pub fn capture_pane(_args: &TmuxArgs) -> Result<(), String> {
    Err("capture-pane: not implemented".to_string())
}

pub fn wait_for(_args: &TmuxArgs) -> Result<(), String> {
    Err("wait-for: not implemented".to_string())
}
