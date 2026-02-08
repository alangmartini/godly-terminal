use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// Initialize the file logger. Logs to `godly-mcp.log` next to the binary,
/// falling back to the system temp directory.
pub fn init() {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("godly-mcp.log")))
        .unwrap_or_else(|| std::env::temp_dir().join("godly-mcp.log"));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    match file {
        Ok(f) => {
            LOG_FILE.get_or_init(|| Mutex::new(f));
        }
        Err(e) => {
            // Last resort: try temp dir if we haven't already
            let fallback = std::env::temp_dir().join("godly-mcp.log");
            if let Ok(f) = OpenOptions::new().create(true).append(true).open(&fallback) {
                LOG_FILE.get_or_init(|| Mutex::new(f));
            } else {
                eprintln!("[godly-mcp] Failed to open log file: {}", e);
            }
        }
    }
}

/// Write a log line with a timestamp.
pub fn log(msg: &str) {
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut file) = mutex.lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let secs = ts.as_secs();
            let millis = ts.subsec_millis();
            let _ = writeln!(file, "[{}.{:03}] {}", secs, millis, msg);
            let _ = file.flush();
        }
    }
}

macro_rules! mcp_log {
    ($($arg:tt)*) => {
        crate::log::log(&format!($($arg)*))
    };
}

pub(crate) use mcp_log;
