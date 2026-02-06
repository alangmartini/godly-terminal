pub mod frame;
pub mod messages;
pub mod types;

pub use frame::{read_message, write_message};
pub use messages::{DaemonMessage, Event, Request, Response};
pub use types::{SessionInfo, ShellType};

/// Named pipe path used by both daemon and client
pub const PIPE_NAME: &str = r"\\.\pipe\godly-terminal-daemon";
