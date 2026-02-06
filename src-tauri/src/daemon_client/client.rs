use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc;

use parking_lot::Mutex;

use godly_protocol::{Request, Response};

/// Client that communicates with the godly-daemon process via named pipes.
///
/// The reader is handed off to `DaemonBridge` which becomes the sole reader.
/// Responses are routed back via an mpsc channel.
pub struct DaemonClient {
    /// Pipe reader — taken by bridge via `take_reader()`
    reader: Mutex<Option<Box<dyn Read + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Receives responses routed by the bridge
    response_rx: Mutex<mpsc::Receiver<Response>>,
    /// Sender given to bridge so it can route responses back
    response_tx: mpsc::Sender<Response>,
}

impl DaemonClient {
    /// Connect to a running daemon, or launch one if none is running.
    pub fn connect_or_launch() -> Result<Self, String> {
        // Try connecting first
        match Self::try_connect() {
            Ok(client) => {
                eprintln!("[daemon_client] Connected to existing daemon");
                return Ok(client);
            }
            Err(e) => {
                eprintln!("[daemon_client] No daemon running ({}), launching...", e);
            }
        }

        // Launch daemon
        Self::launch_daemon()?;

        // Retry connection with backoff
        let mut retries = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(200));
            match Self::try_connect() {
                Ok(client) => {
                    eprintln!("[daemon_client] Connected to newly launched daemon");
                    return Ok(client);
                }
                Err(e) => {
                    retries += 1;
                    if retries > 15 {
                        return Err(format!("Failed to connect to daemon after launch: {}", e));
                    }
                }
            }
        }
    }

    /// Try to connect to an existing daemon
    #[cfg(windows)]
    fn try_connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let pipe_name: Vec<u16> = OsStr::new(godly_protocol::PIPE_NAME)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
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
            return Err("Cannot connect to daemon pipe".to_string());
        }

        // Create reader and writer from the handle
        use std::os::windows::io::FromRawHandle;
        let reader: Box<dyn Read + Send> =
            Box::new(unsafe { std::fs::File::from_raw_handle(handle as _) });

        // Duplicate handle for writer
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::handleapi::DuplicateHandle;
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::winnt::DUPLICATE_SAME_ACCESS;

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
            return Err(format!(
                "DuplicateHandle failed: {}",
                unsafe { GetLastError() }
            ));
        }

        let writer: Box<dyn Write + Send> =
            Box::new(unsafe { std::fs::File::from_raw_handle(writer_handle as _) });

        let (response_tx, response_rx) = mpsc::channel();

        Ok(Self {
            reader: Mutex::new(Some(reader)),
            writer: Arc::new(Mutex::new(writer)),
            response_rx: Mutex::new(response_rx),
            response_tx,
        })
    }

    #[cfg(not(windows))]
    fn try_connect() -> Result<Self, String> {
        Err("Named pipes only supported on Windows".to_string())
    }

    /// Launch the daemon process
    #[cfg(windows)]
    fn launch_daemon() -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        // Find the daemon binary
        let daemon_path = Self::find_daemon_binary()?;
        eprintln!("[daemon_client] Launching daemon: {:?}", daemon_path);

        // Launch as a detached process that survives our exit
        Command::new(&daemon_path)
            .creation_flags(
                0x00000008 | // DETACHED_PROCESS (no console)
                0x00000200,  // CREATE_NEW_PROCESS_GROUP
            )
            .spawn()
            .map_err(|e| format!("Failed to launch daemon: {}", e))?;

        Ok(())
    }

    #[cfg(not(windows))]
    fn launch_daemon() -> Result<(), String> {
        Err("Daemon launch only supported on Windows".to_string())
    }

    /// Find the daemon binary location
    fn find_daemon_binary() -> Result<std::path::PathBuf, String> {
        // In dev mode: look next to current exe in target/debug
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get current exe: {}", e))?;
        let exe_dir = current_exe.parent().ok_or("No parent directory")?;

        let daemon_name = if cfg!(windows) {
            "godly-daemon.exe"
        } else {
            "godly-daemon"
        };

        // Check same directory as the app binary
        let same_dir = exe_dir.join(daemon_name);
        if same_dir.exists() {
            return Ok(same_dir);
        }

        // Check externalBin location (Tauri bundled sidecar location)
        let sidecar_path = exe_dir.join("daemon").join(daemon_name);
        if sidecar_path.exists() {
            return Ok(sidecar_path);
        }

        Err(format!(
            "Daemon binary not found. Looked in: {:?} and {:?}",
            same_dir, sidecar_path
        ))
    }

    /// Take the pipe reader (for handing to the bridge). Can only be called once.
    pub fn take_reader(&self) -> Option<Box<dyn Read + Send>> {
        self.reader.lock().take()
    }

    /// Get the response sender (for the bridge to route responses back).
    pub fn response_sender(&self) -> mpsc::Sender<Response> {
        self.response_tx.clone()
    }

    /// Send a request and wait for the response.
    /// The bridge thread routes responses back via the mpsc channel.
    pub fn send_request(&self, request: &Request) -> Result<Response, String> {
        // Write request
        {
            let mut writer = self.writer.lock();
            godly_protocol::write_message(&mut *writer, request)
                .map_err(|e| format!("Failed to send request: {}", e))?;
        }

        // Wait for response from the bridge (which is the sole reader)
        let rx = self.response_rx.lock();
        rx.recv()
            .map_err(|e| format!("Failed to receive response: {}", e))
    }

    /// Verify the connection is alive
    #[allow(dead_code)]
    pub fn ping(&self) -> Result<(), String> {
        match self.send_request(&Request::Ping)? {
            Response::Pong => Ok(()),
            other => Err(format!("Unexpected ping response: {:?}", other)),
        }
    }
}
