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

/// Install a panic hook that writes crash info + backtrace to the log.
/// Without this, iced-shell panics vanish (no console attached to GUI process).
pub fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();
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

        let bt = std::backtrace::Backtrace::force_capture();

        let msg = format!(
            "PANIC at {}\nPayload: {}\nBacktrace:\n{}",
            location, payload, bt
        );

        log(&msg);

        // Also write to stderr in case a console is attached
        eprintln!("[iced-shell] {}", msg);

        // Chain to previous hook
        prev_hook(info);
    }));
}

/// Install a Windows structured exception handler for silent crashes.
/// Catches access violations, stack overflows, heap corruption that bypass
/// Rust's panic machinery entirely.
///
/// Note: does not chain to a previous filter. Iced/wgpu/winit do not install
/// their own SEH handlers, so there is nothing to chain to. This matches the
/// daemon's proven approach in debug_log.rs.
#[cfg(windows)]
pub fn install_exception_handler() {
    use winapi::um::errhandlingapi::SetUnhandledExceptionFilter;
    use winapi::um::winnt::EXCEPTION_POINTERS;

    unsafe extern "system" fn handler(info: *mut EXCEPTION_POINTERS) -> i32 {
        if info.is_null() {
            return 1; // EXCEPTION_EXECUTE_HANDLER
        }

        let record = (*info).ExceptionRecord;
        if record.is_null() {
            return 1;
        }

        let code = (*record).ExceptionCode;
        let address = (*record).ExceptionAddress as usize;

        let code_name = match code {
            0xC0000005 => "ACCESS_VIOLATION",
            0xC00000FD => "STACK_OVERFLOW",
            0xC0000374 => "HEAP_CORRUPTION",
            0xC0000409 => "STACK_BUFFER_OVERRUN",
            // GPU / device related
            0xC000000E => "NO_SUCH_DEVICE",
            0xC0000017 => "NO_MEMORY",
            0xC0000008 => "INVALID_HANDLE",
            // Code / driver integrity
            0xC000001D => "ILLEGAL_INSTRUCTION",
            0xC0000025 => "NONCONTINUABLE_EXCEPTION",
            0xC0000026 => "INVALID_DISPOSITION",
            // Arithmetic (shader/driver)
            0xC0000094 => "INTEGER_DIVIDE_BY_ZERO",
            0xC000008E => "FLOAT_DIVIDE_BY_ZERO",
            0xC0000091 => "FLOAT_OVERFLOW",
            _ => "UNKNOWN",
        };

        // Use try_lock to avoid deadlock if exception occurs during logging
        if let Some(mutex) = LOG_FILE.get() {
            if let Ok(mut state) = mutex.try_lock() {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                let _ = writeln!(
                    state.file,
                    "[{}.{:03}] FATAL EXCEPTION: code=0x{:08X} ({}) address=0x{:X}",
                    ts.as_secs(),
                    ts.subsec_millis(),
                    code,
                    code_name,
                    address
                );
                let _ = state.file.flush();
            }
        }

        1 // EXCEPTION_EXECUTE_HANDLER
    }

    unsafe {
        SetUnhandledExceptionFilter(Some(handler));
    }
}

#[cfg(not(windows))]
pub fn install_exception_handler() {
    // No-op on non-Windows
}

/// Register a C-level atexit handler that logs when the process exits.
/// This catches `std::process::exit()`, `ExitProcess()`, and normal returns
/// from main — all of which bypass panic hooks and SEH handlers.
pub fn install_atexit_handler() {
    extern "C" {
        fn atexit(cb: extern "C" fn()) -> i32;
    }

    extern "C" fn on_exit() {
        log("PROCESS EXIT — atexit handler fired (process terminating)");
    }

    unsafe {
        atexit(on_exit);
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
