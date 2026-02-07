use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Get the PID file path at %APPDATA%/com.godly.terminal/godly-daemon.pid
pub fn pid_file_path() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(app_data).join("com.godly.terminal");
    fs::create_dir_all(&dir).ok();
    dir.join("godly-daemon.pid")
}

/// Write current process PID to the PID file
pub fn write_pid_file() {
    let path = pid_file_path();
    if let Ok(mut f) = fs::File::create(&path) {
        let _ = write!(f, "{}", std::process::id());
    }
}

/// Remove the PID file
pub fn remove_pid_file() {
    let path = pid_file_path();
    let _ = fs::remove_file(path);
}

/// Check if the daemon is already running by attempting to connect to its named pipe.
/// This is more reliable than checking a PID file since the pipe only exists if
/// the daemon is actively listening.
pub fn is_daemon_running() -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        const ERROR_PIPE_BUSY: u32 = 231;

        let pipe_name: Vec<u16> = OsStr::new(godly_protocol::PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = CreateFileW(
                pipe_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            );

            if handle == INVALID_HANDLE_VALUE {
                // ERROR_PIPE_BUSY means the pipe exists but all instances are in use
                // — the daemon IS running, just busy serving another client
                let err = GetLastError();
                err == ERROR_PIPE_BUSY
            } else {
                CloseHandle(handle);
                true
            }
        }
    }

    #[cfg(not(windows))]
    {
        false
    }
}
