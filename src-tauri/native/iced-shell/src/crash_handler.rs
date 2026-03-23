use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct LogState {
    file: std::fs::File,
    path: PathBuf,
    prev_path: PathBuf,
    bytes_written: u64,
}

static LOG_FILE: OnceLock<Mutex<LogState>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

/// Initialize the crash log file.
/// Must be called first in main(), before any iced code.
pub fn init() {
    START_TIME.get_or_init(Instant::now);

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!(
        "com.godly.terminal{}",
        godly_protocol::instance_suffix()
    );
    let dir = PathBuf::from(app_data).join(&dir_name);
    fs::create_dir_all(&dir).ok();

    let path = dir.join("iced-crash.log");
    let prev_path = dir.join("iced-crash.prev.log");

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
        Err(_) => {
            let fallback_path = std::env::temp_dir().join("iced-crash.log");
            let fallback_prev = std::env::temp_dir().join("iced-crash.prev.log");
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&fallback_path)
            {
                let initial = fs::metadata(&fallback_path).map(|m| m.len()).unwrap_or(0);
                LOG_FILE.get_or_init(|| {
                    Mutex::new(LogState {
                        file: f,
                        path: fallback_path,
                        prev_path: fallback_prev,
                        bytes_written: initial,
                    })
                });
            }
        }
    }

    log(&format!(
        "=== iced-shell crash handler initialized (v{}) ===",
        env!("GODLY_APP_VERSION")
    ));
}

/// Write a timestamped line to the crash log.
/// Uses try_lock to avoid deadlock in panic/exception contexts.
pub fn log(msg: &str) {
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut state) = mutex.try_lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let elapsed = START_TIME
                .get()
                .map(|s| s.elapsed())
                .unwrap_or_default();
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

            if state.bytes_written > MAX_LOG_SIZE {
                rotate(&mut state);
            }
        }
    }
}

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
