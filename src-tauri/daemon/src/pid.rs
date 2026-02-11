use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Get the PID file path at %APPDATA%/com.godly.terminal[suffix]/godly-daemon.pid
/// When GODLY_INSTANCE is set (e.g. "test"), the directory becomes
/// "com.godly.terminal-test" so test and production daemons don't collide.
pub fn pid_file_path() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    let dir = PathBuf::from(app_data).join(dir_name);
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
///
/// NOTE: This has a race window between daemon startup and pipe creation.
/// For singleton enforcement, prefer `DaemonLock::try_acquire()` which uses
/// a named mutex and is race-free.
#[allow(dead_code)]
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

        let pipe_name_str = godly_protocol::pipe_name();
        let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
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

/// RAII guard that holds a Windows named mutex to enforce only one daemon
/// instance per GODLY_INSTANCE. The mutex is released automatically when
/// the process exits (normally or via crash), so no stale locks can occur.
///
/// Bug fix: previously, `is_daemon_running()` checked the named pipe, which
/// has a race window between process start and pipe creation. Multiple
/// daemons could slip through during this window.
pub struct DaemonLock {
    #[cfg(windows)]
    _handle: *mut std::ffi::c_void,
}

// SAFETY: The handle is a Windows kernel object used only to keep the mutex
// alive. It is not dereferenced or accessed from multiple threads.
unsafe impl Send for DaemonLock {}
unsafe impl Sync for DaemonLock {}

impl DaemonLock {
    /// Try to acquire the singleton lock. Returns Ok(DaemonLock) if this is
    /// the only daemon, or Err if another instance already holds the lock.
    ///
    /// The mutex name is derived from the pipe name, ensuring tests that use
    /// `GODLY_PIPE_NAME` get isolated mutexes automatically.
    #[cfg(windows)]
    pub fn try_acquire() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::shared::winerror::ERROR_ALREADY_EXISTS;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::synchapi::CreateMutexW;

        // Derive mutex name from the pipe name so each instance (including
        // test instances with GODLY_PIPE_NAME) gets a unique mutex.
        let pipe = godly_protocol::pipe_name();
        let pipe_suffix = pipe.rsplit('\\').next().unwrap_or(&pipe);
        let name = format!("godly-daemon-lock-{}", pipe_suffix);
        let wide: Vec<u16> = OsStr::new(&name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe { CreateMutexW(std::ptr::null_mut(), 0, wide.as_ptr()) };
        if handle.is_null() {
            let err = unsafe { GetLastError() };
            return Err(format!("CreateMutexW failed: error {}", err));
        }

        let err = unsafe { GetLastError() };
        if err == ERROR_ALREADY_EXISTS {
            unsafe { winapi::um::handleapi::CloseHandle(handle) };
            return Err("Another daemon instance is already running".to_string());
        }

        Ok(Self {
            _handle: handle as *mut std::ffi::c_void,
        })
    }

    #[cfg(not(windows))]
    pub fn try_acquire() -> Result<Self, String> {
        // On non-Windows, fall back to pipe check
        if is_daemon_running() {
            return Err("Another daemon instance is already running".to_string());
        }
        Ok(Self {})
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            winapi::um::handleapi::CloseHandle(self._handle as *mut _);
        }
    }
}
