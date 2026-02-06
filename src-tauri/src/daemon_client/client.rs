use std::io::{Read, Write};
use std::sync::mpsc;

use parking_lot::Mutex;
use tauri::AppHandle;

use godly_protocol::{Request, Response};

use super::bridge::{BridgeRequest, DaemonBridge};

/// Client that communicates with the godly-daemon process via named pipes.
///
/// Both the reader and writer are handed off to `DaemonBridge`, which performs
/// all pipe I/O from a single thread using PeekNamedPipe for non-blocking reads.
/// The client sends requests to the bridge via a channel, and receives responses
/// via a per-request one-shot channel.
///
/// If the pipe connection breaks, the client automatically reconnects on the
/// next `send_request` call.
pub struct DaemonClient {
    /// Pipe reader — taken by bridge via `setup_bridge()`
    reader: Mutex<Option<Box<dyn Read + Send>>>,
    /// Pipe writer — taken by bridge via `setup_bridge()`
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    /// Sender for submitting requests to the bridge thread
    request_tx: Mutex<Option<mpsc::Sender<BridgeRequest>>>,
    /// App handle for bridge setup (stored after first `setup_bridge` call)
    app_handle: Mutex<Option<AppHandle>>,
    /// Prevents concurrent reconnection attempts
    reconnect_lock: Mutex<()>,
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

        Ok(Self {
            reader: Mutex::new(Some(reader)),
            writer: Mutex::new(Some(writer)),
            request_tx: Mutex::new(None),
            app_handle: Mutex::new(None),
            reconnect_lock: Mutex::new(()),
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

    /// Set up the bridge: creates channels, starts the bridge I/O thread, and
    /// stores the request sender. Also stores the app_handle for future reconnections.
    pub fn setup_bridge(&self, app_handle: AppHandle) -> Result<(), String> {
        let reader = self
            .reader
            .lock()
            .take()
            .ok_or("Daemon reader not available")?;
        let writer = self
            .writer
            .lock()
            .take()
            .ok_or("Daemon writer not available")?;

        let (request_tx, request_rx) = mpsc::channel();
        *self.request_tx.lock() = Some(request_tx);

        let bridge = DaemonBridge::new();
        bridge.start(reader, writer, request_rx, app_handle.clone());

        *self.app_handle.lock() = Some(app_handle);

        Ok(())
    }

    /// Reconnect to the daemon, establishing a new pipe and bridge.
    /// Called automatically when `send_request` detects a broken connection.
    fn reconnect(&self) -> Result<(), String> {
        let _guard = self.reconnect_lock.lock();

        // Check if another thread already reconnected while we waited for the lock
        if self.request_tx.lock().is_some() {
            // Try a quick ping to verify the connection is alive
            if self.try_send_request(&Request::Ping).is_ok() {
                return Ok(());
            }
        }

        eprintln!("[daemon_client] Reconnecting to daemon...");

        // Clear stale request sender so no new requests go to the dead bridge
        *self.request_tx.lock() = None;

        // Try connecting to existing daemon first, then launch if needed
        let new_client = match Self::try_connect() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("[daemon_client] Daemon not reachable, launching new one...");
                Self::launch_daemon()?;

                let mut retries = 0;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    match Self::try_connect() {
                        Ok(c) => break c,
                        Err(e) => {
                            retries += 1;
                            if retries > 15 {
                                return Err(format!(
                                    "Failed to reconnect to daemon after launch: {}",
                                    e
                                ));
                            }
                        }
                    }
                }
            }
        };

        // Move the new connection's reader/writer into self
        *self.reader.lock() = new_client.reader.lock().take();
        *self.writer.lock() = new_client.writer.lock().take();

        // Set up the bridge with the stored app_handle
        let app_handle = self
            .app_handle
            .lock()
            .clone()
            .ok_or("No app_handle stored — setup_bridge was never called")?;

        self.setup_bridge(app_handle)?;

        eprintln!("[daemon_client] Reconnected to daemon");
        Ok(())
    }

    /// Low-level send: attempts to send a request through the current bridge.
    /// Returns Err if the bridge channel is broken or the response channel is dropped.
    fn try_send_request(&self, request: &Request) -> Result<Response, String> {
        let tx = self
            .request_tx
            .lock()
            .as_ref()
            .ok_or("Bridge not started yet")?
            .clone();

        // Create a one-shot channel for this request's response
        let (response_tx, response_rx) = mpsc::channel();

        tx.send(BridgeRequest {
            request: request.clone(),
            response_tx,
        })
        .map_err(|e| format!("Failed to send request to bridge: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive response: {}", e))
    }

    /// Check if an error indicates a broken connection (bridge channel dead).
    fn is_connection_error(err: &str) -> bool {
        err.contains("Failed to send request to bridge")
            || err.contains("Failed to receive response")
            || err.contains("Bridge not started yet")
    }

    /// Send a request and wait for the response.
    /// If the connection is broken, automatically reconnects and retries once.
    pub fn send_request(&self, request: &Request) -> Result<Response, String> {
        match self.try_send_request(request) {
            Ok(response) => Ok(response),
            Err(e) if Self::is_connection_error(&e) => {
                eprintln!(
                    "[daemon_client] Connection error: {}, attempting reconnect...",
                    e
                );
                self.reconnect()?;
                self.try_send_request(request)
            }
            Err(e) => Err(e),
        }
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
