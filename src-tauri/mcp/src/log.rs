use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

struct LogState {
    file: File,
    path: PathBuf,
    prev_path: PathBuf,
    bytes_written: u64,
}

static LOG_FILE: OnceLock<Mutex<LogState>> = OnceLock::new();

/// Maximum log file size before rotation (2MB).
const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

/// Initialize the file logger. Logs to `godly-mcp.log` next to the binary,
/// falling back to the system temp directory.
pub fn init() {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("godly-mcp.log")))
        .unwrap_or_else(|| std::env::temp_dir().join("godly-mcp.log"));

    let prev_path = path.with_extension("prev.log");

    // Rotate if the log file is too large
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_SIZE {
            let _ = fs::copy(&path, &prev_path);
            let _ = fs::remove_file(&path);
        }
    }

    let initial_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    match file {
        Ok(f) => {
            LOG_FILE.get_or_init(|| {
                Mutex::new(LogState {
                    file: f,
                    path: path.clone(),
                    prev_path: prev_path.clone(),
                    bytes_written: initial_size,
                })
            });
        }
        Err(e) => {
            // Last resort: try temp dir if we haven't already
            let fallback_path = std::env::temp_dir().join("godly-mcp.log");
            let fallback_prev = fallback_path.with_extension("prev.log");
            if let Ok(f) = OpenOptions::new().create(true).append(true).open(&fallback_path) {
                let initial = fs::metadata(&fallback_path).map(|m| m.len()).unwrap_or(0);
                LOG_FILE.get_or_init(|| {
                    Mutex::new(LogState {
                        file: f,
                        path: fallback_path,
                        prev_path: fallback_prev,
                        bytes_written: initial,
                    })
                });
            } else {
                eprintln!("[godly-mcp] Failed to open log file: {}", e);
            }
        }
    }
}

/// Write a log line with a timestamp.
pub fn log(msg: &str) {
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut state) = mutex.lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let line = format!(
                "[{}.{:03}] {}\n",
                ts.as_secs(),
                ts.subsec_millis(),
                msg
            );
            let _ = state.file.write_all(line.as_bytes());
            let _ = state.file.flush();
            state.bytes_written += line.len() as u64;

            if state.bytes_written > MAX_LOG_SIZE {
                rotate(&mut state);
            }
        }
    }
}

/// Rotate: copy current → .prev.log, truncate current, reset counter.
fn rotate(state: &mut LogState) {
    let _ = fs::copy(&state.path, &state.prev_path);
    let _ = fs::remove_file(&state.path);
    if let Ok(f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.path)
    {
        state.file = f;
        state.bytes_written = 0;
    }
}

macro_rules! mcp_log {
    ($($arg:tt)*) => {
        crate::log::log(&format!($($arg)*))
    };
}

pub(crate) use mcp_log;
