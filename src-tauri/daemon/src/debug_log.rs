use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

/// Maximum log file size before rotation (2MB).
/// When the log exceeds this, the current file is renamed to `.prev.log`
/// and a fresh file is started. This keeps at least 2 full runs of history.
const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

/// Initialize the daemon debug logger.
/// Logs to `godly-daemon-debug.log` in %APPDATA%/com.godly.terminal[suffix]/,
/// falling back to the system temp directory.
///
/// Uses APPEND mode so logs survive daemon restarts. The previous run's crash
/// info is always available for post-mortem. Rotates to `.prev.log` when the
/// file exceeds MAX_LOG_SIZE.
pub fn init() {
    START_TIME.get_or_init(Instant::now);

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!(
        "com.godly.terminal{}",
        godly_protocol::instance_suffix()
    );
    let dir = std::path::PathBuf::from(app_data).join(dir_name);
    fs::create_dir_all(&dir).ok();

    let path = dir.join("godly-daemon-debug.log");
    let prev_path = dir.join("godly-daemon-debug.prev.log");

    // Rotate if the log file is too large
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_SIZE {
            // Replace .prev.log with current, start fresh
            let _ = fs::copy(&path, &prev_path);
            let _ = fs::remove_file(&path);
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    match file {
        Ok(f) => {
            LOG_FILE.get_or_init(|| Mutex::new(f));
        }
        Err(e) => {
            let fallback = std::env::temp_dir().join("godly-daemon-debug.log");
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&fallback)
            {
                LOG_FILE.get_or_init(|| Mutex::new(f));
            } else {
                eprintln!("[daemon] Failed to open debug log: {}", e);
            }
        }
    }
}

/// Install a panic hook that writes the panic info to the log before dying.
/// Without this, daemon panics vanish into the void (no console attached).
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown payload".to_string()
        };

        let msg = format!(
            "PANIC at {}: {}\nFull info: {}",
            location, payload, info
        );

        // Write to log file
        log(&msg);

        // Also write to stderr in case someone is watching
        eprintln!("[daemon] {}", msg);

        // Call the default hook (prints backtrace etc.)
        default_hook(info);
    }));
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
