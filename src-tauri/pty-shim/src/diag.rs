//! Diagnostic file-based logger for the PTY shim.
//!
//! The shim's stderr is redirected to null by the daemon, so we log to a file
//! in %APPDATA%/com.godly.terminal/ for post-mortem analysis.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct LogState {
    file: std::fs::File,
    bytes_written: u64,
}

static LOG: OnceLock<Mutex<LogState>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

const MAX_SIZE: u64 = 512 * 1024; // 512KB per shim log

/// Initialize the shim diagnostic logger.
/// Logs to `godly-shim-diag-{session_id}.log` in %APPDATA%/com.godly.terminal/.
pub fn init(session_id: &str) {
    START.get_or_init(Instant::now);

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let instance_suffix = std::env::var("GODLY_INSTANCE")
        .ok()
        .map(|v| format!("-{}", v))
        .unwrap_or_default();
    let dir_name = format!("com.godly.terminal{}", instance_suffix);
    let dir = std::path::PathBuf::from(app_data).join(dir_name);
    fs::create_dir_all(&dir).ok();

    let path = dir.join(format!("godly-shim-diag-{}.log", session_id));

    // Truncate on init — each shim instance gets a fresh log
    if let Ok(f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        LOG.get_or_init(|| {
            Mutex::new(LogState {
                file: f,
                bytes_written: 0,
            })
        });
    }
}

/// Write a timestamped log line.
pub fn log(msg: &str) {
    if let Some(mutex) = LOG.get() {
        if let Ok(mut state) = mutex.lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let elapsed = START.get().map(|s| s.elapsed()).unwrap_or_default();
            let line = format!(
                "[{}.{:03}] [{:>8.3}s] {}\n",
                ts.as_secs(),
                ts.subsec_millis(),
                elapsed.as_secs_f64(),
                msg
            );
            let _ = state.file.write_all(line.as_bytes());
            let _ = state.file.flush();
            state.bytes_written += line.len() as u64;

            // Truncate if too large (unlikely for a single session)
            if state.bytes_written > MAX_SIZE {
                state.bytes_written = 0;
                let _ = state.file.set_len(0);
            }
        }
    }
}

macro_rules! shim_log {
    ($($arg:tt)*) => {
        crate::diag::log(&format!($($arg)*))
    };
}

pub(crate) use shim_log;
