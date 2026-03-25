//! Pane management commands (stub).
//!
//! Implemented by the pane-commands unit. These stubs allow the binary
//! to compile and report a clear error if invoked before integration.

/// `tmux split-window` — stub
pub fn split_window(_args: &[String]) -> Result<(), String> {
    Err("split-window: not yet implemented".to_string())
}

/// `tmux select-pane` — stub
pub fn select_pane(_args: &[String]) -> Result<(), String> {
    Err("select-pane: not yet implemented".to_string())
}

/// `tmux resize-pane` — stub
pub fn resize_pane(_args: &[String]) -> Result<(), String> {
    Err("resize-pane: not yet implemented".to_string())
}

/// `tmux list-panes` — stub
pub fn list_panes(_args: &[String]) -> Result<(), String> {
    Err("list-panes: not yet implemented".to_string())
}
