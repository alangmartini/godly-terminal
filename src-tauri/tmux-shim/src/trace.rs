//! Lightweight file-append tracing for debugging tmux shim interactions.
//!
//! Enabled by setting `GODLY_TMUX_TRACE=1`. Logs to
//! `$APPDATA/com.godly.terminal/tmux-shim.log` in append mode.
//!
//! Each line: `[<unix_ms>] pid=<pid> <message>`
//!
//! The log file is never truncated — rotate externally if it grows large.

use std::io::Write;
use std::sync::OnceLock;

static ENABLED: OnceLock<bool> = OnceLock::new();

/// Check if tracing is enabled (cached after first call).
fn is_enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var("GODLY_TMUX_TRACE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

fn log_path() -> std::path::PathBuf {
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    std::path::PathBuf::from(appdata)
        .join("com.godly.terminal")
        .join("tmux-shim.log")
}

/// Append a trace line to the log file. No-op if tracing is disabled.
pub fn write_line(msg: &str) {
    if !is_enabled() {
        return;
    }
    let path = log_path();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] pid={} {}", timestamp, pid, msg);
    }
}

/// Trace macro — formats and appends to log file when tracing is enabled.
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::trace::write_line(&format!($($arg)*))
    };
}
