use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;

use godly_protocol::{
    read_daemon_message_ext, write_request_with_id, DaemonMessage, Event, ReadResult, Request,
    Response,
};

/// A raw HANDLE stored as usize for thread safety.
/// Named pipe handles are safe to send between threads.
/// We store as usize to avoid the `*mut c_void` Send issue.
#[cfg(windows)]
#[derive(Clone, Copy)]
struct RawHandle(usize);

#[cfg(windows)]
impl RawHandle {
    fn from_handle(h: winapi::shared::ntdef::HANDLE) -> Self {
        Self(h as usize)
    }
    fn as_handle(self) -> winapi::shared::ntdef::HANDLE {
        self.0 as winapi::shared::ntdef::HANDLE
    }
}

/// Trait for receiving daemon events without Tauri dependency.
/// The native frontend implements this to receive terminal events.
pub trait FrontendEventSink: Send + Sync + 'static {
    fn on_terminal_output(&self, session_id: &str);
    fn on_session_closed(&self, session_id: &str, exit_code: Option<i64>);
    fn on_process_changed(&self, session_id: &str, process_name: &str);
    fn on_grid_diff(&self, session_id: &str, diff_bytes: &[u8]);
    fn on_bell(&self, session_id: &str);
}

/// A request queued for sending to the daemon via the bridge I/O thread.
struct BridgeRequest {
    request: Request,
    request_id: Option<u32>,
    response_tx: Option<mpsc::Sender<Response>>,
}

/// Tauri-free daemon client that communicates via named pipes.
///
/// Spawns a bridge I/O thread that handles all pipe reads/writes.
/// Requests are sent via a channel, responses routed back by request_id.
pub struct NativeDaemonClient {
    /// Pipe reader — taken by bridge via `setup_bridge()`
    reader: Mutex<Option<Box<dyn Read + Send>>>,
    /// Pipe writer — taken by bridge via `setup_bridge()`
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    /// Raw handle for the reader pipe (used by PeekNamedPipe in bridge thread)
    #[cfg(windows)]
    reader_handle: Mutex<Option<RawHandle>>,
    /// Sender for submitting requests to the bridge thread
    request_tx: Mutex<Option<mpsc::Sender<BridgeRequest>>>,
    /// Session IDs currently attached (for re-attach after reconnect)
    attached_sessions: Mutex<Vec<String>>,
    /// Monotonically incrementing counter for request IDs
    next_request_id: AtomicU32,
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
                return Err(format!(
                    "Cannot connect to daemon pipe (error: {})",
                    final_err
                ));
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
            #[cfg(windows)]
            reader_handle: Mutex::new(Some(RawHandle::from_handle(handle))),
            request_tx: Mutex::new(None),
            attached_sessions: Mutex::new(Vec::new()),
            next_request_id: AtomicU32::new(1),
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

        Err(format!(
            "Daemon binary not found. Looked in: {:?}",
            same_dir
        ))
    }

    /// Set up the bridge I/O thread, handing off the pipe reader/writer.
    ///
    /// The bridge thread reads from the pipe (events + responses) and writes
    /// requests queued via the channel. Events are dispatched to the sink;
    /// responses are routed back to the caller via request_id.
    #[cfg(windows)]
    pub fn setup_bridge<S: FrontendEventSink>(&self, sink: Arc<S>) -> Result<(), String> {
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
        let reader_handle = self
            .reader_handle
            .lock()
            .take()
            .ok_or("Reader handle not available")?;

        let (request_tx, request_rx) = mpsc::channel();
        *self.request_tx.lock() = Some(request_tx);

        let raw_handle = reader_handle.as_handle();

        // Spawn the bridge I/O thread
        // Safety: the handle is valid and we're transferring ownership to the thread.
        let handle_usize = raw_handle as usize;
        thread::Builder::new()
            .name("native-bridge-io".into())
            .spawn(move || {
                let handle = handle_usize as winapi::shared::ntdef::HANDLE;
                bridge_io_loop(reader, writer, handle, request_rx, sink);
            })
            .map_err(|e| format!("Failed to spawn bridge thread: {}", e))?;

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn setup_bridge<S: FrontendEventSink>(&self, _sink: Arc<S>) -> Result<(), String> {
        Err("Bridge setup only supported on Windows".to_string())
    }

    /// Send a request and wait for the response.
    pub fn send_request(&self, request: &Request) -> Result<Response, String> {
        let tx = self
            .request_tx
            .lock()
            .as_ref()
            .ok_or("Bridge not started — call setup_bridge() first")?
            .clone();

        let (response_tx, response_rx) = mpsc::channel();
        let request_id = Some(self.next_request_id.fetch_add(1, Ordering::Relaxed));

        tx.send(BridgeRequest {
            request: request.clone(),
            request_id,
            response_tx: Some(response_tx),
        })
        .map_err(|e| format!("Failed to send request to bridge: {}", e))?;

        response_rx
            .recv_timeout(Duration::from_secs(15))
            .map_err(|e| format!("Failed to receive response: {}", e))
    }

    /// Send a request without waiting for the response (fire-and-forget).
    /// Used for Write and Resize where blocking would add latency.
    pub fn send_fire_and_forget(&self, request: &Request) -> Result<(), String> {
        let tx = self
            .request_tx
            .lock()
            .as_ref()
            .ok_or("Bridge not started — call setup_bridge() first")?
            .clone();

        tx.send(BridgeRequest {
            request: request.clone(),
            request_id: None,
            response_tx: None,
        })
        .map_err(|e| format!("Failed to send request to bridge: {}", e))
    }

    /// Track a session as attached (for re-attach after reconnect).
    pub fn track_attach(&self, session_id: String) {
        let mut sessions = self.attached_sessions.lock();
        if !sessions.contains(&session_id) {
            sessions.push(session_id);
        }
    }

    /// Remove a session from the attached tracking list.
    pub fn track_detach(&self, session_id: &str) {
        self.attached_sessions.lock().retain(|id| id != session_id);
    }
}

/// Bridge I/O loop: single thread handling all pipe reads and writes.
///
/// Uses `PeekNamedPipe` for non-blocking reads so it can interleave
/// reading events/responses with writing queued requests.
#[cfg(windows)]
fn bridge_io_loop<S: FrontendEventSink>(
    mut reader: Box<dyn Read + Send>,
    mut writer: Box<dyn Write + Send>,
    reader_handle: winapi::shared::ntdef::HANDLE,
    request_rx: mpsc::Receiver<BridgeRequest>,
    sink: Arc<S>,
) {
    let mut pending_responses: HashMap<u32, mpsc::Sender<Response>> = HashMap::new();

    loop {
        // Phase 1: drain and write all pending requests
        loop {
            match request_rx.try_recv() {
                Ok(bridge_req) => {
                    if let Err(e) = write_request_with_id(
                        &mut writer,
                        &bridge_req.request,
                        bridge_req.request_id,
                    ) {
                        log::error!("Bridge: failed to write request: {}", e);
                        return; // Pipe broken
                    }
                    if let (Some(id), Some(tx)) = (bridge_req.request_id, bridge_req.response_tx) {
                        pending_responses.insert(id, tx);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    log::info!("Bridge: request channel disconnected, shutting down");
                    return;
                }
            }
        }

        // Phase 2: check if data is available to read (non-blocking)
        let data_available = {
            use winapi::um::namedpipeapi::PeekNamedPipe;

            let mut bytes_available: u32 = 0;
            let result = unsafe {
                PeekNamedPipe(
                    reader_handle,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut bytes_available,
                    std::ptr::null_mut(),
                )
            };

            if result == 0 {
                log::error!("Bridge: PeekNamedPipe failed, pipe likely broken");
                return;
            }

            bytes_available > 0
        };

        if data_available {
            // Read and dispatch messages
            match read_daemon_message_ext(&mut reader) {
                Ok(ReadResult::Message(DaemonMessage::Event(event))) => {
                    dispatch_event(&*sink, event);
                }
                Ok(ReadResult::Response {
                    response,
                    request_id,
                }) => {
                    if let Some(id) = request_id {
                        if let Some(tx) = pending_responses.remove(&id) {
                            let _ = tx.send(response);
                        }
                    }
                }
                Ok(ReadResult::RawGridDiff {
                    session_id,
                    binary_diff,
                }) => {
                    sink.on_grid_diff(&session_id, &binary_diff);
                }
                Ok(ReadResult::Eof) => {
                    log::info!("Bridge: daemon pipe EOF");
                    return;
                }
                Ok(ReadResult::Message(DaemonMessage::Response(response))) => {
                    // Response without request_id routing — drop it
                    log::warn!("Bridge: unroutable response: {:?}", response);
                }
                Err(e) => {
                    log::error!("Bridge: read error: {}", e);
                    return;
                }
            }
        } else {
            // No data available, sleep briefly to avoid busy-spinning
            thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Dispatch a daemon event to the frontend event sink.
fn dispatch_event<S: FrontendEventSink>(sink: &S, event: Event) {
    match event {
        Event::Output { session_id, .. } => {
            sink.on_terminal_output(&session_id);
        }
        Event::SessionClosed {
            session_id,
            exit_code,
        } => {
            sink.on_session_closed(&session_id, exit_code);
        }
        Event::ProcessChanged {
            session_id,
            process_name,
        } => {
            sink.on_process_changed(&session_id, &process_name);
        }
        Event::GridDiff { session_id, .. } => {
            // Full GridDiff — the caller already decoded it.
            // Send an empty diff_bytes since the actual diff was in the event.
            sink.on_grid_diff(&session_id, &[]);
        }
        Event::Bell { session_id } => {
            sink.on_bell(&session_id);
        }
    }
}
