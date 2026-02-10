use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

/// Initialize the daemon debug logger.
/// Logs to `godly-daemon-debug.log` in %APPDATA%/com.godly.terminal[suffix]/,
/// falling back to the system temp directory.
pub fn init() {
    START_TIME.get_or_init(Instant::now);

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!(
        "com.godly.terminal{}",
        godly_protocol::instance_suffix()
    );
    let dir = std::path::PathBuf::from(app_data).join(dir_name);
    std::fs::create_dir_all(&dir).ok();

    let path = dir.join("godly-daemon-debug.log");

    // Truncate to avoid unbounded growth (keep last run only)
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path);

    match file {
        Ok(f) => {
            LOG_FILE.get_or_init(|| Mutex::new(f));
        }
        Err(e) => {
            let fallback = std::env::temp_dir().join("godly-daemon-debug.log");
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&fallback)
            {
                LOG_FILE.get_or_init(|| Mutex::new(f));
            } else {
                eprintln!("[daemon] Failed to open debug log: {}", e);
            }
        }
    }
}

/// Write a log line with timestamp (wall clock + monotonic elapsed).
pub fn log(msg: &str) {
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut file) = mutex.lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let elapsed = START_TIME
                .get()
                .map(|s| s.elapsed())
                .unwrap_or_default();
            let _ = writeln!(
                file,
                "[{}.{:03}] [{:>8.3}s] {}",
                ts.as_secs(),
                ts.subsec_millis(),
                elapsed.as_secs_f64(),
                msg
            );
            let _ = file.flush();
        }
    }
}

macro_rules! daemon_log {
    ($($arg:tt)*) => {
        crate::debug_log::log(&format!($($arg)*))
    };
}

pub(crate) use daemon_log;
