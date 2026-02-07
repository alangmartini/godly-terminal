pub mod frame;
pub mod messages;
pub mod types;

pub use frame::{read_message, write_message};
pub use messages::{DaemonMessage, Event, Request, Response};
pub use types::{SessionInfo, ShellType};

/// Default named pipe path used by both daemon and client
pub const PIPE_NAME: &str = r"\\.\pipe\godly-terminal-daemon";

/// Get the pipe name, allowing override via GODLY_PIPE_NAME env var.
/// Used by tests to run isolated daemon instances on different pipes.
pub fn pipe_name() -> String {
    std::env::var("GODLY_PIPE_NAME").unwrap_or_else(|_| PIPE_NAME.to_string())
}
