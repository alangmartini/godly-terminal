use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpec {
    pub cwd: Option<String>,
    pub shell: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub id: String,
    pub title: String,
    pub process_name: String,
    pub exited: bool,
}

/// Side-effect port for terminal session lifecycle and I/O.
pub trait DaemonPort {
    fn create_session(&mut self, spec: SessionSpec) -> Result<String, String>;
    fn close_session(&mut self, session_id: &str) -> Result<(), String>;
    fn write(&mut self, session_id: &str, bytes: &[u8]) -> Result<(), String>;
    fn resize(&mut self, session_id: &str, rows: u16, cols: u16) -> Result<(), String>;
    fn list_sessions(&self) -> Result<Vec<SessionSnapshot>, String>;
}

/// Side-effect port for user clipboard interactions.
pub trait ClipboardPort {
    fn read_text(&self) -> Result<String, String>;
}

/// Side-effect port for user-visible notifications.
pub trait NotificationPort {
    fn notify(&mut self, title: &str, body: &str) -> Result<(), String>;
}

/// Time source abstraction to support deterministic tests.
pub trait ClockPort {
    fn now(&self) -> SystemTime;
}
