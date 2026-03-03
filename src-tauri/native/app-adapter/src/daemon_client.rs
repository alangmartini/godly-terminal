use std::io::{Read, Write};
use std::time::Duration;

use parking_lot::Mutex;

use godly_protocol::{Request, Response};

/// Trait for receiving daemon events without Tauri dependency.
/// The native frontend implements this to receive terminal events.
pub trait FrontendEventSink: Send + Sync + 'static {
    fn on_terminal_output(&self, session_id: &str);
    fn on_session_closed(&self, session_id: &str, exit_code: Option<i64>);
    fn on_process_changed(&self, session_id: &str, process_name: &str);
    fn on_grid_diff(&self, session_id: &str, diff_bytes: &[u8]);
    fn on_bell(&self, session_id: &str);
}

/// Tauri-free daemon client that communicates via named pipes.
///
/// Extracted from `src-tauri/src/daemon_client/client.rs`. Uses the same
/// wire protocol (godly-protocol) but replaces Tauri's `AppHandle::emit()`
/// with the `FrontendEventSink` trait.
pub struct NativeDaemonClient {
    reader: Mutex<Option<Box<dyn Read + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    attached_sessions: Mutex<Vec<String>>,
}

impl NativeDaemonClient {
    /// Connect to a running daemon, or launch one if none is running.
    pub fn connect_or_launch() -> Result<Self, String> {
        match Self::try_connect() {
            Ok(client) => {
                log::info!("Connected to existing daemon");
                return Ok(client);
            }
            Err(e) => {
                log::info!("No daemon running ({}), launching...", e);
            }
        }

        Self::launch_daemon()?;

        let mut retries = 0;
        loop {
            std::thread::sleep(Duration::from_millis(200));
            match Self::try_connect() {
                Ok(client) => {
                    log::info!("Connected to newly launched daemon");
                    return Ok(client);
                }
                Err(e) => {
                    retries += 1;
                    if retries > 15 {
                        return Err(format!("Failed to connect after launch: {}", e));
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    fn try_connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::{DuplicateHandle, INVALID_HANDLE_VALUE};
        use winapi::um::namedpipeapi::WaitNamedPipeW;
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::winnt::{
            DUPLICATE_SAME_ACCESS, FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE,
        };

        const ERROR_PIPE_BUSY: u32 = 231;

        let pipe_name_str = godly_protocol::pipe_name();
        let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut handle = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            if err == ERROR_PIPE_BUSY {
                let wait_result = unsafe { WaitNamedPipeW(pipe_name.as_ptr(), 5000) };
                if wait_result != 0 {
                    handle = unsafe {
                        CreateFileW(
                            pipe_name.as_ptr(),
                            GENERIC_READ | GENERIC_WRITE,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            std::ptr::null_mut(),
                            OPEN_EXISTING,
                            0,
                            std::ptr::null_mut(),
                        )
                    };
                }
            }

            if handle == INVALID_HANDLE_VALUE {
                let final_err = unsafe { GetLastError() };
                return Err(format!("Cannot connect to daemon pipe (error: {})", final_err));
            }
        }

        let reader: Box<dyn Read + Send> =
            Box::new(unsafe { std::fs::File::from_raw_handle(handle as _) });

        let mut writer_handle = std::ptr::null_mut();
        let dup_result = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                handle,
                GetCurrentProcess(),
                &mut writer_handle,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };

        if dup_result == 0 {
            return Err(format!("DuplicateHandle failed: {}", unsafe {
                GetLastError()
            }));
        }

        let writer: Box<dyn Write + Send> =
            Box::new(unsafe { std::fs::File::from_raw_handle(writer_handle as _) });

        Ok(Self {
            reader: Mutex::new(Some(reader)),
            writer: Mutex::new(Some(writer)),
            attached_sessions: Mutex::new(Vec::new()),
        })
    }

    #[cfg(not(windows))]
    fn try_connect() -> Result<Self, String> {
        Err("Named pipes only supported on Windows".to_string())
    }

    #[cfg(windows)]
    fn launch_daemon() -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        let daemon_path = Self::find_daemon_binary()?;
        log::info!("Launching daemon: {:?}", daemon_path);

        let base_flags: u32 = 0x00000008 | 0x00000200;
        let breakaway_flag: u32 = 0x01000000;

        let mut cmd = Command::new(&daemon_path);
        cmd.creation_flags(base_flags | breakaway_flag);

        if let Ok(instance) = std::env::var("GODLY_INSTANCE") {
            cmd.args(["--instance", &instance]);
        }

        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(ref e) if e.raw_os_error() == Some(5) => {
                let mut cmd2 = Command::new(&daemon_path);
                cmd2.creation_flags(base_flags);
                if let Ok(instance) = std::env::var("GODLY_INSTANCE") {
                    cmd2.args(["--instance", &instance]);
                }
                cmd2.spawn()
                    .map(|_| ())
                    .map_err(|e| format!("Failed to launch daemon: {}", e))
            }
            Err(e) => Err(format!("Failed to launch daemon: {}", e)),
        }
    }

    #[cfg(not(windows))]
    fn launch_daemon() -> Result<(), String> {
        Err("Daemon launch only supported on Windows".to_string())
    }

    fn find_daemon_binary() -> Result<std::path::PathBuf, String> {
        let current_exe =
            std::env::current_exe().map_err(|e| format!("Failed to get current exe: {}", e))?;
        let exe_dir = current_exe.parent().ok_or("No parent directory")?;

        let daemon_name = if cfg!(windows) {
            "godly-daemon.exe"
        } else {
            "godly-daemon"
        };

        let same_dir = exe_dir.join(daemon_name);
        if same_dir.exists() {
            return Ok(same_dir);
        }

        Err(format!("Daemon binary not found. Looked in: {:?}", same_dir))
    }

    /// Send a request and wait for a response (stub — full bridge in Phase 1).
    pub fn send_request(&self, _request: &Request) -> Result<Response, String> {
        Err("NativeDaemonClient::send_request is a stub — full implementation in Phase 1".into())
    }

    pub fn track_attach(&self, session_id: String) {
        let mut sessions = self.attached_sessions.lock();
        if !sessions.contains(&session_id) {
            sessions.push(session_id);
        }
    }

    pub fn track_detach(&self, session_id: &str) {
        self.attached_sessions.lock().retain(|id| id != session_id);
    }

    pub fn take_reader(&self) -> Option<Box<dyn Read + Send>> {
        self.reader.lock().take()
    }

    pub fn take_writer(&self) -> Option<Box<dyn Write + Send>> {
        self.writer.lock().take()
    }
}
