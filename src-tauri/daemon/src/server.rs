use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use godly_protocol::{DaemonMessage, Event, Request, Response, read_request, write_daemon_message};

use crate::debug_log::daemon_log;
use crate::handlers;
use crate::handlers::HandlerContext;
use crate::scrollback_budget::ScrollbackBudget;
use crate::session::DaemonSession;

/// Log current process memory usage (Windows: working set via GetProcessMemoryInfo).
#[cfg(windows)]
pub(crate) fn log_memory_usage(label: &str) {
    use winapi::um::processthreadsapi::GetCurrentProcess;
    use winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};

    unsafe {
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) != 0 {
            daemon_log!(
                "MEMORY [{}]: working_set={:.1}MB, peak_working_set={:.1}MB, pagefile={:.1}MB",
                label,
                pmc.WorkingSetSize as f64 / (1024.0 * 1024.0),
                pmc.PeakWorkingSetSize as f64 / (1024.0 * 1024.0),
                pmc.PagefileUsage as f64 / (1024.0 * 1024.0),
            );
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn log_memory_usage(label: &str) {
    daemon_log!("MEMORY [{}]: (not available on this platform)", label);
}

/// Named pipe server that manages daemon sessions and client connections.
pub struct DaemonServer {
    sessions: Arc<RwLock<HashMap<String, DaemonSession>>>,
    running: Arc<AtomicBool>,
    /// Number of currently connected clients. Used by the idle timeout checker
    /// to avoid shutting down while clients are still connected. Previously
    /// this was an AtomicBool which caused a race: when one client disconnected,
    /// it set has_clients=false even if other clients were still connected,
    /// allowing the idle timeout to kill the daemon prematurely.
    client_count: Arc<AtomicUsize>,
    scrollback_budget: Arc<parking_lot::Mutex<ScrollbackBudget>>,
}

impl DaemonServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(true)),
            client_count: Arc::new(AtomicUsize::new(0)),
            scrollback_budget: Arc::new(parking_lot::Mutex::new(ScrollbackBudget::new())),
        }
    }

    /// Run the server, listening for connections on the named pipe.
    /// Returns when the server should shut down (idle timeout or explicit stop).
    pub async fn run(&self) {
        let pipe_name = godly_protocol::pipe_name();
        eprintln!("[daemon] Server starting on {}", pipe_name);
        daemon_log!("Server starting on {}", pipe_name);

        // Reconnect to surviving shim processes from a previous daemon instance
        self.reconnect_surviving_shims();

        // Start process monitor
        self.start_process_monitor();

        // Start idle timeout checker
        let running = self.running.clone();
        let sessions = self.sessions.clone();
        let client_count = self.client_count.clone();
        let last_activity = Arc::new(RwLock::new(Instant::now()));
        let last_activity_for_timeout = last_activity.clone();
        let scrollback_budget = self.scrollback_budget.clone();

        tokio::spawn(async move {
            let idle_timeout = Duration::from_secs(300); // 5 minutes
            let mut health_tick: u64 = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                health_tick += 1;
                let (session_count, session_ids) = {
                    let guard = sessions.read();
                    let count = guard.len();
                    let ids: Vec<String> = guard.keys().cloned().collect();
                    (count, ids)
                };

                let num_clients = client_count.load(Ordering::Relaxed);
                let elapsed = last_activity_for_timeout.read().elapsed();

                // Log health every 30s (every 3rd tick)
                if health_tick % 3 == 0 {
                    daemon_log!(
                        "HEALTH: sessions={}, clients={}, idle={:.0}s, session_ids={:?}",
                        session_count,
                        num_clients,
                        elapsed.as_secs_f64(),
                        session_ids
                    );
                    log_memory_usage("health_check");
                }

                // Scrollback budget enforcement (every 6th tick = ~60s)
                if health_tick % 6 == 0 && session_count > 0 {
                    let mut budget = scrollback_budget.lock();
                    budget.retain_sessions(&session_ids);
                    {
                        let guard = sessions.read();
                        for (id, session) in guard.iter() {
                            let (rows, _bytes) = session.scrollback_stats();
                            budget.update_session(
                                id,
                                rows,
                                session.cols(),
                                session.is_paused(),
                                session.last_output_epoch_ms(),
                            );
                        }
                    }
                    let actions = budget.check_and_trim();
                    if !actions.is_empty() {
                        let guard = sessions.read();
                        for (id, new_len) in &actions {
                            if let Some(session) = guard.get(id.as_str()) {
                                session.set_scrollback_len(*new_len);
                            }
                        }
                    }
                }

                if session_count == 0 && num_clients == 0 && elapsed > idle_timeout {
                    eprintln!("[daemon] Idle timeout reached, shutting down");
                    daemon_log!("Idle timeout reached (no sessions, no clients for {:?}), shutting down", elapsed);
                    log_memory_usage("idle_shutdown");
                    running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        });

        // Accept connections loop
        while self.running.load(Ordering::Relaxed) {
            match self.accept_connection().await {
                Ok(pipe) => {
                    *last_activity.write() = Instant::now();
                    self.client_count.fetch_add(1, Ordering::Relaxed);

                    let sessions = self.sessions.clone();
                    let running = self.running.clone();
                    let client_count = self.client_count.clone();
                    let activity = last_activity.clone();

                    let client_num = self.client_count.load(Ordering::Relaxed);
                    daemon_log!("Client connected, spawning handler (clients={})", client_num);
                    log_memory_usage("client_connect");

                    tokio::spawn(async move {
                        handle_client(pipe, sessions.clone(), running, activity).await;
                        let remaining = client_count.fetch_sub(1, Ordering::Relaxed) - 1;
                        let session_count = sessions.read().len();
                        daemon_log!(
                            "Client disconnected (clients={}, sessions={})",
                            remaining,
                            session_count
                        );
                        log_memory_usage("client_disconnect");
                    });
                }
                Err(e) => {
                    if self.running.load(Ordering::Relaxed) {
                        eprintln!("[daemon] Accept error: {}", e);
                        daemon_log!("Accept error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }

        eprintln!("[daemon] Server shutting down");
        daemon_log!("Server shutting down");

        // Close all sessions before exiting — sends Shutdown to each shim
        // so they don't become orphan processes.
        self.shutdown_all_sessions();
    }

    /// Close all active sessions, sending Shutdown to each shim process.
    /// Called during daemon shutdown to prevent orphaned pty-shim processes.
    fn shutdown_all_sessions(&self) {
        let sessions = self.sessions.read();
        let count = sessions.len();
        if count == 0 {
            return;
        }
        daemon_log!("Shutting down {} session(s)", count);
        for (id, session) in sessions.iter() {
            daemon_log!("Closing session {} on shutdown", id);
            session.close();
        }
        drop(sessions);
        self.sessions.write().clear();
        daemon_log!("All sessions closed");
    }

    /// Accept a single named pipe connection (Windows implementation).
    /// Returns a single File handle used for both reading and writing.
    #[cfg(windows)]
    async fn accept_connection(&self) -> Result<std::fs::File, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::shared::winerror::ERROR_PIPE_CONNECTED;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
        use winapi::um::namedpipeapi::{ConnectNamedPipe, CreateNamedPipeW};
        use winapi::um::winbase::{
            PIPE_ACCESS_DUPLEX, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
            PIPE_WAIT,
        };

        let pipe_name_str = godly_protocol::pipe_name();
        let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let running = self.running.clone();

        // Create and wait for connection in a blocking thread
        let result = tokio::task::spawn_blocking(move || {
            let handle = unsafe {
                CreateNamedPipeW(
                    pipe_name.as_ptr(),
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    262144, // 256KB outbound buffer (was 4KB - too small, caused write blocking)
                    262144, // 256KB inbound buffer
                    0,
                    std::ptr::null_mut(),
                )
            };

            if handle == INVALID_HANDLE_VALUE {
                return Err(format!(
                    "CreateNamedPipe failed: {}",
                    unsafe { GetLastError() }
                ));
            }

            // ConnectNamedPipe blocks until a client connects
            let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
            if connected == 0 {
                let err = unsafe { GetLastError() };
                if err != ERROR_PIPE_CONNECTED {
                    unsafe { CloseHandle(handle) };
                    if !running.load(Ordering::Relaxed) {
                        return Err("Server shutting down".to_string());
                    }
                    return Err(format!("ConnectNamedPipe failed: {}", err));
                }
            }

            eprintln!("[daemon] Client connected");

            // Return a single File — used for both reading and writing in one thread
            use std::os::windows::io::FromRawHandle;
            let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };

            Ok(pipe)
        })
        .await
        .map_err(|e| format!("Spawn blocking failed: {}", e))?;

        result
    }

    #[cfg(not(windows))]
    async fn accept_connection(&self) -> Result<std::fs::File, String> {
        Err("Named pipes are only supported on Windows".to_string())
    }

    /// Start a background process monitor that tracks foreground process names
    fn start_process_monitor(&self) {
        let sessions = self.sessions.clone();
        let running = self.running.clone();

        // NOTE: Process monitoring is implemented but events are only sent to
        // attached clients via the session output channel. The bridge in the
        // Tauri app translates these to process-changed events.
        // For now, we keep the process monitor logic in the Tauri app side
        // since it needs access to the AppHandle to emit Tauri events.
        // The daemon just keeps sessions alive.
        let _ = (sessions, running);
    }

    /// Scan for surviving pty-shim processes and reconnect to them.
    /// Called on daemon startup before accepting client connections.
    fn reconnect_surviving_shims(&self) {
        let survivors = crate::shim_metadata::discover_surviving_shims();

        if survivors.is_empty() {
            daemon_log!("No surviving shims found");
            return;
        }

        daemon_log!("Reconnecting to {} surviving shim(s)", survivors.len());

        for meta in survivors {
            let session_id = meta.session_id.clone();
            let shim_pid = meta.shim_pid;

            // Defensive check: only reconnect shims that belong to this instance.
            // The shim pipe name includes the instance suffix, so if it doesn't match
            // what this daemon would generate, the shim belongs to a different instance.
            let expected_pipe = godly_protocol::shim_pipe_name(&session_id);
            if meta.shim_pipe_name != expected_pipe {
                daemon_log!(
                    "Skipping foreign shim: session={} pipe={} (expected {})",
                    session_id,
                    meta.shim_pipe_name,
                    expected_pipe
                );
                continue;
            }

            match DaemonSession::reconnect(meta) {
                Ok(session) => {
                    self.sessions.write().insert(session_id.clone(), session);
                    daemon_log!("Reconnected session {}", session_id);
                }
                Err(e) => {
                    daemon_log!("Failed to reconnect session {}: {}", session_id, e);
                    // Kill the orphaned shim — it's alive but we can't talk to it,
                    // so it would sit around consuming memory until its orphan timeout.
                    if crate::shim_client::kill_process(shim_pid) {
                        daemon_log!(
                            "Killed unreconnectable shim pid={} for session {}",
                            shim_pid,
                            session_id
                        );
                    }
                    crate::shim_metadata::remove_metadata(&session_id);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Get the raw handle from a File for use with PeekNamedPipe
#[cfg(windows)]
fn get_raw_handle(file: &std::fs::File) -> isize {
    use std::os::windows::io::AsRawHandle;
    file.as_raw_handle() as isize
}

#[cfg(not(windows))]
fn get_raw_handle(_file: &std::fs::File) -> isize {
    0
}

/// Result of a non-blocking pipe peek: data available, empty, or error (pipe closed/broken).
enum PeekResult {
    Data,
    Empty,
    Error,
}

/// Non-blocking check: how many bytes are available to read from the pipe?
#[cfg(windows)]
fn peek_pipe(handle: isize) -> PeekResult {
    use winapi::um::namedpipeapi::PeekNamedPipe;

    let mut bytes_available: u32 = 0;
    let result = unsafe {
        PeekNamedPipe(
            handle as *mut _,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut bytes_available,
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        return PeekResult::Error; // Pipe closed or broken
    }
    if bytes_available > 0 {
        PeekResult::Data
    } else {
        PeekResult::Empty
    }
}

#[cfg(not(windows))]
fn peek_pipe(_handle: isize) -> PeekResult {
    PeekResult::Empty
}

/// Handle a single client connection using a single I/O thread.
///
/// On Windows, synchronous named pipe I/O is serialized per file object.
/// Using separate threads for reading and writing the same pipe deadlocks.
/// Instead, we do ALL pipe I/O from one blocking thread using PeekNamedPipe
/// for non-blocking read checks (same pattern as the client-side DaemonBridge).
///
/// Architecture:
/// ```text
/// [Named Pipe] <--read/write--> [I/O Thread (spawn_blocking)]
///                                     |            ^
///                                     | req_tx     | msg_tx (responses + events)
///                                     v            |
///                               [Async Handler] --+
///                                     |
///                                     v
///                               [Session forwarding tasks]
/// ```
async fn handle_client(
    pipe: std::fs::File,
    sessions: Arc<RwLock<HashMap<String, DaemonSession>>>,
    running: Arc<AtomicBool>,
    last_activity: Arc<RwLock<Instant>>,
) {
    // Track which sessions this client has attached to
    let attached_sessions: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));

    // Channel: I/O thread -> async handler (incoming requests from client)
    let (req_tx, mut req_rx) = mpsc::unbounded_channel::<Request>();

    // Channel: async handler -> I/O thread (responses only, HIGH PRIORITY)
    // Responses are always written before events to prevent user input from
    // timing out when the terminal is producing heavy output (e.g. Claude CLI).
    let (resp_tx, resp_rx) = mpsc::unbounded_channel::<DaemonMessage>();

    // Channel: session forwarding tasks -> I/O thread (output events, NORMAL PRIORITY)
    let (event_tx, event_rx) = mpsc::channel::<DaemonMessage>(1024);

    // Signal to stop the I/O thread when the async handler is done
    let io_running = Arc::new(AtomicBool::new(true));

    // Spawn the single I/O thread that does ALL pipe reads and writes.
    // Passes sessions so Write/Resize can be handled directly in the I/O thread
    // without bouncing through the async handler (eliminates 2 channel hops).
    let io_running_clone = io_running.clone();
    let running_clone = running.clone();
    let io_sessions = sessions.clone();
    let io_handle = tokio::task::spawn_blocking(move || {
        io_thread(pipe, req_tx, resp_rx, event_rx, io_running_clone, running_clone, io_sessions);
    });

    // Async handler loop: process requests from the I/O thread
    eprintln!("[daemon] Entering request loop for client");
    daemon_log!("Entering request loop for client");
    while let Some(request) = req_rx.recv().await {
        // If the I/O thread has died (pipe closed/broken), stop processing.
        // Without this check, a stuck handler keeps accumulating when the bridge
        // reconnects — each new client's handler blocks on session locks while
        // the old handler never exits.
        if !io_running.load(Ordering::Relaxed) {
            daemon_log!("io_thread stopped, breaking handler loop");
            break;
        }

        *last_activity.write() = Instant::now();
        daemon_log!("Received request: {:?}", request);

        let response = handle_request(
            &request,
            &sessions,
            &event_tx,
            &attached_sessions,
        )
        .await;

        daemon_log!("Sending response for {:?}", std::mem::discriminant(&request));

        // Send response to the HIGH PRIORITY channel so it's written before events.
        // Bug fix: previously responses shared a channel with output events, causing
        // user input to time out during heavy terminal output (e.g. Claude CLI).
        let msg = DaemonMessage::Response(response);
        if resp_tx.send(msg).is_err() {
            daemon_log!("resp_tx send failed, breaking handler loop");
            break;
        }
    }

    // Signal I/O thread to stop and wait for it
    io_running.store(false, Ordering::Relaxed);
    let _ = io_handle.await;

    // Client disconnected - detach all sessions (they keep running)
    {
        let sessions_guard = sessions.read();
        let attached = attached_sessions.read();
        for session_id in attached.iter() {
            if let Some(session) = sessions_guard.get(session_id) {
                session.detach();
                eprintln!(
                    "[daemon] Auto-detached session {} (client disconnect)",
                    session_id
                );
                daemon_log!(
                    "Auto-detached session {} (client disconnect)",
                    session_id
                );
            }
        }
    }

    daemon_log!("Client handler exiting");
}

/// Maximum number of outgoing messages to write per I/O loop iteration.
/// After writing this many messages, we check for incoming data before writing more.
/// This prevents write-heavy scenarios from starving request reads.
const MAX_WRITES_PER_ITERATION: usize = 128;

/// Single I/O thread: performs all pipe reads and writes.
/// Uses PeekNamedPipe for non-blocking read checks to avoid deadlock.
///
/// Write and Resize requests are handled DIRECTLY in this thread to avoid
/// bouncing through the async handler (eliminates 2 tokio channel hops and
/// scheduler latency per keystroke). Other requests still go to the async handler.
///
/// Responses are written from `resp_rx` with HIGH PRIORITY (always drained first).
/// Events are written from `event_rx` with NORMAL PRIORITY (batch-limited).
/// This prevents user input responses from being delayed by output event floods.
fn io_thread(
    mut pipe: std::fs::File,
    req_tx: mpsc::UnboundedSender<Request>,
    mut resp_rx: mpsc::UnboundedReceiver<DaemonMessage>,
    mut event_rx: mpsc::Receiver<DaemonMessage>,
    io_running: Arc<AtomicBool>,
    server_running: Arc<AtomicBool>,
    sessions: Arc<RwLock<HashMap<String, DaemonSession>>>,
) {
    let raw_handle = get_raw_handle(&pipe);
    let mut last_log_time = Instant::now();
    let mut total_reads: u64 = 0;
    let mut total_writes: u64 = 0;
    let mut total_resp_writes: u64 = 0;
    let mut total_bytes_written: u64 = 0;
    let mut write_stall_count: u64 = 0;
    let direct_writes: u64 = 0; // Always 0: writes now go through async handler (deadlock fix)

    daemon_log!("io_thread started, handle={}", raw_handle);

    while io_running.load(Ordering::Relaxed) && server_running.load(Ordering::Relaxed) {
        let mut did_work = false;

        // Step 1: ALWAYS check for incoming data first (requests from client).
        // This ensures user input (Write, Resize) is never starved by outgoing events.
        match peek_pipe(raw_handle) {
            PeekResult::Data => {
                // Read the request from the pipe
                let read_start = Instant::now();
                match read_request(&mut pipe) {
                    Ok(Some(request)) => {
                        total_reads += 1;
                        let elapsed = read_start.elapsed();
                        if elapsed > Duration::from_millis(50) {
                            daemon_log!(
                                "SLOW READ: {:?} took {:.1}ms",
                                std::mem::discriminant(&request),
                                elapsed.as_secs_f64() * 1000.0
                            );
                        }

                        // Handle Resize directly in the I/O thread.
                        // Resize is fast (no I/O to ConPTY input pipe) and latency-sensitive.
                        // Write is handled async via spawn_blocking to avoid blocking the
                        // I/O thread when ConPTY input fills during heavy output (deadlock fix).
                        match &request {
                            Request::Resize { session_id, rows, cols } => {
                                let response = handlers::resize::handle_raw(&sessions, session_id, *rows, *cols);
                                let msg = DaemonMessage::Response(response);
                                if write_daemon_message(&mut pipe, &msg).is_err() {
                                    daemon_log!("Write error on direct Resize response, stopping");
                                    io_running.store(false, Ordering::Relaxed);
                                    break;
                                }
                                total_resp_writes += 1;
                                total_writes += 1;
                                did_work = true;
                            }
                            Request::PauseSession { session_id } => {
                                let response = handlers::pause_session::handle_raw(&sessions, session_id);
                                let msg = DaemonMessage::Response(response);
                                if write_daemon_message(&mut pipe, &msg).is_err() {
                                    daemon_log!("Write error on direct PauseSession response, stopping");
                                    io_running.store(false, Ordering::Relaxed);
                                    break;
                                }
                                total_resp_writes += 1;
                                total_writes += 1;
                                did_work = true;
                            }
                            Request::ResumeSession { session_id } => {
                                let response = handlers::resume_session::handle_raw(&sessions, session_id);
                                let msg = DaemonMessage::Response(response);
                                if write_daemon_message(&mut pipe, &msg).is_err() {
                                    daemon_log!("Write error on direct ResumeSession response, stopping");
                                    io_running.store(false, Ordering::Relaxed);
                                    break;
                                }
                                total_resp_writes += 1;
                                total_writes += 1;
                                did_work = true;
                            }
                            _ => {
                                // All other requests go through the async handler
                                if req_tx.send(request).is_err() {
                                    eprintln!("[daemon-io] Request channel closed, stopping");
                                    daemon_log!("Request channel closed, stopping");
                                    break;
                                }
                                did_work = true;
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("[daemon-io] Client disconnected (EOF)");
                        daemon_log!("Client disconnected (EOF)");
                        break;
                    }
                    Err(e) => {
                        if io_running.load(Ordering::Relaxed) {
                            eprintln!("[daemon-io] Read error: {}", e);
                            daemon_log!("Read error: {}", e);
                        }
                        break;
                    }
                }
            }
            PeekResult::Error => {
                eprintln!("[daemon-io] Pipe closed or broken, stopping");
                daemon_log!("Pipe peek error, stopping");
                break;
            }
            PeekResult::Empty => {
                // No incoming data - fall through to write outgoing messages
            }
        }

        // Step 2a: HIGH PRIORITY — write ALL pending responses immediately.
        // Responses (to Ping, CreateSession, etc.) must not be delayed by
        // queued output events. Write/Resize responses are already written
        // directly in step 1, so this handles only async-handler responses.
        loop {
            match resp_rx.try_recv() {
                Ok(msg) => {
                    let write_start = Instant::now();
                    if write_daemon_message(&mut pipe, &msg).is_err() {
                        eprintln!("[daemon-io] Write error on response, stopping");
                        daemon_log!("Write error on response, stopping");
                        io_running.store(false, Ordering::Relaxed);
                        break;
                    }
                    let elapsed = write_start.elapsed();
                    total_writes += 1;
                    total_resp_writes += 1;
                    did_work = true;

                    if elapsed > Duration::from_millis(50) {
                        write_stall_count += 1;
                        daemon_log!(
                            "SLOW WRITE: Response took {:.1}ms (stall #{})",
                            elapsed.as_secs_f64() * 1000.0,
                            write_stall_count
                        );
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Response channel closed — async handler died (possibly from
                    // an eprintln! panic when running without a console).
                    // Stop the io_thread so the client gets EOF instead of hanging.
                    daemon_log!("Response channel disconnected, stopping io_thread");
                    io_running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }

        if !io_running.load(Ordering::Relaxed) {
            break;
        }

        // Step 2b: NORMAL PRIORITY — write events with batch limit.
        // Limit per iteration to avoid starving reads. If we have many queued
        // events and a slow pipe, writing them all would block and prevent
        // reading new requests.
        let mut writes_this_iteration = 0;
        while writes_this_iteration < MAX_WRITES_PER_ITERATION {
            match event_rx.try_recv() {
                Ok(msg) => {
                    let write_start = Instant::now();
                    let msg_kind = match &msg {
                        DaemonMessage::Event(Event::Output { .. }) => "Output",
                        DaemonMessage::Event(Event::SessionClosed { .. }) => "SessionClosed",
                        DaemonMessage::Event(Event::ProcessChanged { .. }) => "ProcessChanged",
                        DaemonMessage::Event(Event::GridDiff { .. }) => "GridDiff",
                        DaemonMessage::Event(Event::Bell { .. }) => "Bell",
                        DaemonMessage::Response(_) => "Response", // shouldn't happen
                    };

                    if write_daemon_message(&mut pipe, &msg).is_err() {
                        eprintln!("[daemon-io] Write error, stopping");
                        daemon_log!("Write error on {}, stopping", msg_kind);
                        io_running.store(false, Ordering::Relaxed);
                        break;
                    }

                    let elapsed = write_start.elapsed();
                    total_writes += 1;
                    writes_this_iteration += 1;
                    did_work = true;

                    // Track bytes written for diagnostics
                    match &msg {
                        DaemonMessage::Event(Event::Output { ref data, .. }) => {
                            total_bytes_written += data.len() as u64;
                        }
                        _ => {}
                    }

                    // Log slow writes (indicates pipe buffer full / client not reading)
                    if elapsed > Duration::from_millis(50) {
                        write_stall_count += 1;
                        daemon_log!(
                            "SLOW WRITE: {} took {:.1}ms (stall #{})",
                            msg_kind,
                            elapsed.as_secs_f64() * 1000.0,
                            write_stall_count
                        );
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    break;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    eprintln!("[daemon-io] Event channel disconnected, stopping");
                    daemon_log!("Event channel disconnected, stopping");
                    io_running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }

        // If we hit the write limit, check for incoming data before writing more
        if writes_this_iteration >= MAX_WRITES_PER_ITERATION {
            continue; // Loop back to peek for incoming requests
        }

        if !did_work {
            // Sleep 1ms when idle. timeBeginPeriod(1) is set at daemon startup
            // so this actually sleeps ~1ms (not ~15ms as with default timer resolution).
            std::thread::sleep(Duration::from_millis(1));
        }

        // Periodic stats logging
        if last_log_time.elapsed() > Duration::from_secs(30) {
            let resp_depth = resp_rx.len();
            daemon_log!(
                "io_thread stats: reads={}, writes={} (resp={}, direct={}), bytes_out={}, stalls={}, resp_queue={}, event_cap=1024",
                total_reads,
                total_writes,
                total_resp_writes,
                direct_writes,
                total_bytes_written,
                write_stall_count,
                resp_depth
            );
            last_log_time = Instant::now();
        }
    }

    let resp_depth = resp_rx.len();
    daemon_log!(
        "io_thread stopped: reads={}, writes={} (resp={}, direct={}), bytes_out={}, stalls={}, resp_queue={}, event_cap=1024",
        total_reads,
        total_writes,
        total_resp_writes,
        direct_writes,
        total_bytes_written,
        write_stall_count,
        resp_depth
    );
    eprintln!("[daemon-io] I/O thread stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug: Responses shared a channel with output events. Under heavy terminal
    /// output (e.g. Claude CLI), hundreds of events queued before the response,
    /// causing the client to time out after 5s.
    ///
    /// Fix: Separate response and event channels. I/O thread always drains the
    /// response channel first.
    ///
    /// This test verifies that responses are written to the pipe before events,
    /// even when the event channel has many queued messages.
    #[cfg(windows)]
    #[test]
    fn test_io_thread_response_priority() {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;
        use winapi::shared::winerror::ERROR_PIPE_CONNECTED;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::namedpipeapi::{ConnectNamedPipe, CreateNamedPipeW};
        use winapi::um::winbase::{
            PIPE_ACCESS_DUPLEX, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
        };
        use winapi::um::winnt::{
            FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE,
        };

        // Create a unique named pipe for this test
        let pipe_name = format!(
            r"\\.\pipe\godly-test-priority-{}",
            std::process::id()
        );
        let pipe_wide: Vec<u16> = OsStr::new(&pipe_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Create server-side named pipe
        let server_handle = unsafe {
            CreateNamedPipeW(
                pipe_wide.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,
                262144,
                262144,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(
            server_handle,
            INVALID_HANDLE_VALUE,
            "CreateNamedPipeW failed"
        );

        // Connect client in a separate thread (ConnectNamedPipe blocks)
        let pipe_name_clone = pipe_name.clone();
        let client_thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let wide: Vec<u16> = OsStr::new(&pipe_name_clone)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            assert_ne!(handle, INVALID_HANDLE_VALUE, "Client CreateFileW failed");
            unsafe { std::fs::File::from_raw_handle(handle as _) }
        });

        // Wait for client connection
        let connected = unsafe { ConnectNamedPipe(server_handle, std::ptr::null_mut()) };
        if connected == 0 {
            let err = unsafe { GetLastError() };
            assert_eq!(err, ERROR_PIPE_CONNECTED, "ConnectNamedPipe failed: {}", err);
        }
        let server_file: std::fs::File =
            unsafe { std::fs::File::from_raw_handle(server_handle as _) };
        let client_file = client_thread.join().unwrap();

        // Set up channels
        let (req_tx, _req_rx) = mpsc::unbounded_channel::<Request>();
        let (resp_tx, resp_rx) = mpsc::unbounded_channel::<DaemonMessage>();
        let (event_tx, event_rx) = mpsc::channel::<DaemonMessage>(1024);
        let io_running = Arc::new(AtomicBool::new(true));
        let server_running = Arc::new(AtomicBool::new(true));

        // Pre-queue: 100 output events THEN 1 response.
        // Without priority, the response would be written after all events.
        for i in 0..100 {
            event_tx
                .try_send(DaemonMessage::Event(Event::Output {
                    session_id: "test".into(),
                    data: vec![i as u8; 64],
                }))
                .unwrap();
        }
        resp_tx
            .send(DaemonMessage::Response(Response::Pong))
            .unwrap();

        // Run io_thread — it will write to the server side of the pipe
        let io_running_clone = io_running.clone();
        let server_running_clone = server_running.clone();
        let test_sessions = Arc::new(RwLock::new(HashMap::new()));
        let io_handle = std::thread::spawn(move || {
            let stopper = io_running_clone.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                stopper.store(false, Ordering::Relaxed);
            });
            io_thread(
                server_file,
                req_tx,
                resp_rx,
                event_rx,
                io_running_clone,
                server_running_clone,
                test_sessions,
            );
        });

        // Read from the client side — the FIRST message should be the Response
        let mut reader = std::io::BufReader::new(&client_file);
        let first_msg: DaemonMessage =
            godly_protocol::read_daemon_message(&mut reader).unwrap().unwrap();

        // The response MUST come first, before any of the 100 events
        assert!(
            matches!(first_msg, DaemonMessage::Response(Response::Pong)),
            "Expected first message to be Response::Pong (priority), got {:?}",
            match &first_msg {
                DaemonMessage::Response(r) => format!("Response({:?})", r),
                DaemonMessage::Event(e) => format!("Event({:?})", std::mem::discriminant(e)),
            }
        );

        io_running.store(false, Ordering::Relaxed);
        let _ = io_handle.join();
    }
}

/// Bug: when a shell process exited, the forwarding task's rx.recv() returned
/// None but it never sent SessionClosed — the tab just looked frozen.
/// Fix: after the forwarding loop exits, check running_flag. If false (PTY
/// exited), send SessionClosed. If true (client detached), don't send.
#[cfg(test)]
mod forwarding_tests {
    use super::*;

    /// When the channel closes with running=false (PTY exited), the forwarding
    /// task should send a SessionClosed event.
    #[tokio::test]
    async fn test_forwarding_sends_session_closed_on_pty_exit() {
        use crate::session::SessionOutput;
        let (tx, mut rx) = mpsc::channel::<SessionOutput>(64);
        let (event_tx, mut event_rx) = mpsc::channel::<DaemonMessage>(16);
        let running_flag = Arc::new(AtomicBool::new(true));
        let sid = "test-fwd-closed".to_string();

        let flag = running_flag.clone();
        let handle = tokio::spawn({
            let sid = sid.clone();
            async move {
                while let Some(output) = rx.recv().await {
                    let event = match output {
                        SessionOutput::RawBytes(data) => DaemonMessage::Event(Event::Output {
                            session_id: sid.clone(),
                            data,
                        }),
                        SessionOutput::GridDiff(diff) => DaemonMessage::Event(Event::GridDiff {
                            session_id: sid.clone(),
                            diff,
                        }),
                        SessionOutput::Bell => DaemonMessage::Event(Event::Bell {
                            session_id: sid.clone(),
                        }),
                    };
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
                if !flag.load(Ordering::Relaxed) {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                            exit_code: None,
                        }))
                        .await;
                }
            }
        });

        // Send some output, then simulate PTY exit
        tx.send(SessionOutput::RawBytes(b"hello".to_vec())).await.unwrap();
        running_flag.store(false, Ordering::Relaxed);
        drop(tx); // close the channel

        handle.await.unwrap();

        // Should get Output then SessionClosed
        let msg1 = event_rx.recv().await.unwrap();
        assert!(
            matches!(msg1, DaemonMessage::Event(Event::Output { .. })),
            "first message should be Output"
        );

        let msg2 = event_rx.recv().await.unwrap();
        match msg2 {
            DaemonMessage::Event(Event::SessionClosed { session_id, .. }) => {
                assert_eq!(session_id, sid);
            }
            other => panic!("expected SessionClosed, got {:?}", std::mem::discriminant(&other)),
        }
    }

    /// When the channel closes with running=true (client detached), the
    /// forwarding task should NOT send SessionClosed.
    #[tokio::test]
    async fn test_forwarding_no_session_closed_on_detach() {
        use crate::session::SessionOutput;
        let (tx, mut rx) = mpsc::channel::<SessionOutput>(64);
        let (event_tx, mut event_rx) = mpsc::channel::<DaemonMessage>(16);
        let running_flag = Arc::new(AtomicBool::new(true));
        let sid = "test-fwd-detach".to_string();

        let flag = running_flag.clone();
        let handle = tokio::spawn({
            let sid = sid.clone();
            async move {
                while let Some(output) = rx.recv().await {
                    let event = match output {
                        SessionOutput::RawBytes(data) => DaemonMessage::Event(Event::Output {
                            session_id: sid.clone(),
                            data,
                        }),
                        SessionOutput::GridDiff(diff) => DaemonMessage::Event(Event::GridDiff {
                            session_id: sid.clone(),
                            diff,
                        }),
                        SessionOutput::Bell => DaemonMessage::Event(Event::Bell {
                            session_id: sid.clone(),
                        }),
                    };
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
                if !flag.load(Ordering::Relaxed) {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                            exit_code: None,
                        }))
                        .await;
                }
            }
        });

        // Send some output, then simulate detach (running stays true)
        tx.send(SessionOutput::RawBytes(b"hello".to_vec())).await.unwrap();
        // running_flag stays true — this is a detach, not a PTY exit
        drop(tx);

        handle.await.unwrap();

        // Should get Output only, no SessionClosed
        let msg1 = event_rx.recv().await.unwrap();
        assert!(
            matches!(msg1, DaemonMessage::Event(Event::Output { .. })),
            "first message should be Output"
        );

        // Channel should be empty — no SessionClosed sent
        assert!(
            event_rx.try_recv().is_err(),
            "should NOT receive SessionClosed on detach (running=true)"
        );
    }

    /// Bug A2: Attaching to a session whose PTY already exited should send
    /// SessionClosed immediately, not block forever on rx.recv().
    #[tokio::test]
    async fn test_attach_to_dead_session_sends_session_closed() {
        use crate::session::SessionOutput;
        let (_tx, mut rx) = mpsc::channel::<SessionOutput>(64);
        let (event_tx, mut event_rx) = mpsc::channel::<DaemonMessage>(16);
        let running_flag = Arc::new(AtomicBool::new(false));
        let sid = "test-attach-dead".to_string();
        let is_already_dead = true;

        let flag = running_flag.clone();
        let handle = tokio::spawn({
            let sid = sid.clone();
            async move {
                if is_already_dead {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                            exit_code: None,
                        }))
                        .await;
                    return;
                }
                while let Some(output) = rx.recv().await {
                    let event = match output {
                        SessionOutput::RawBytes(data) => DaemonMessage::Event(Event::Output {
                            session_id: sid.clone(),
                            data,
                        }),
                        SessionOutput::GridDiff(diff) => DaemonMessage::Event(Event::GridDiff {
                            session_id: sid.clone(),
                            diff,
                        }),
                        SessionOutput::Bell => DaemonMessage::Event(Event::Bell {
                            session_id: sid.clone(),
                        }),
                    };
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
                if !flag.load(Ordering::Relaxed) {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                            exit_code: None,
                        }))
                        .await;
                }
            }
        });

        handle.await.unwrap();

        let msg = event_rx.recv().await.unwrap();
        match msg {
            DaemonMessage::Event(Event::SessionClosed { session_id, .. }) => {
                assert_eq!(session_id, "test-attach-dead");
            }
            other => panic!(
                "expected SessionClosed, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
}

async fn handle_request(
    request: &Request,
    sessions: &Arc<RwLock<HashMap<String, DaemonSession>>>,
    msg_tx: &mpsc::Sender<DaemonMessage>,
    attached_sessions: &Arc<RwLock<Vec<String>>>,
) -> Response {
    let ctx = HandlerContext {
        sessions: sessions.clone(),
        msg_tx: msg_tx.clone(),
        attached_sessions: attached_sessions.clone(),
    };

    match request {
        Request::Ping => Response::Pong,

        Request::CreateSession { id, shell_type, cwd, rows, cols, env } => {
            handlers::create_session::handle(&ctx, id, shell_type, cwd, *rows, *cols, env).await
        }

        Request::ListSessions => handlers::list_sessions::handle(&ctx).await,

        Request::Attach { session_id } => handlers::attach::handle(&ctx, session_id).await,

        Request::Detach { session_id } => handlers::detach::handle(&ctx, session_id).await,

        Request::Write { session_id, data } => {
            handlers::write::handle(&ctx, session_id, data).await
        }

        Request::Resize { session_id, rows, cols } => {
            handlers::resize::handle(&ctx, session_id, *rows, *cols).await
        }

        Request::CloseSession { session_id } => {
            handlers::close_session::handle(&ctx, session_id).await
        }

        Request::ReadBuffer { session_id } => {
            handlers::read_buffer::handle(&ctx, session_id).await
        }

        Request::GetLastOutputTime { session_id } => {
            handlers::get_last_output_time::handle(&ctx, session_id).await
        }

        Request::SearchBuffer { session_id, text, strip_ansi } => {
            handlers::search_buffer::handle(&ctx, session_id, text, *strip_ansi).await
        }

        Request::ReadGrid { session_id } => {
            handlers::read_grid::handle(&ctx, session_id).await
        }

        Request::ReadRichGrid { session_id } => {
            handlers::read_rich_grid::handle(&ctx, session_id).await
        }

        Request::ReadRichGridDiff { session_id } => {
            handlers::read_rich_grid_diff::handle(&ctx, session_id).await
        }

        Request::ReadGridText { session_id, start_row, start_col, end_row, end_col, scrollback_offset } => {
            handlers::read_grid_text::handle(&ctx, session_id, *start_row, *start_col, *end_row, *end_col, *scrollback_offset).await
        }

        Request::SetScrollback { session_id, offset } => {
            handlers::set_scrollback::handle(&ctx, session_id, *offset).await
        }

        Request::ScrollAndReadRichGrid { session_id, offset } => {
            handlers::scroll_and_read_rich_grid::handle(&ctx, session_id, *offset).await
        }

        // PauseSession/ResumeSession are handled directly in the I/O thread
        // for low latency. If they somehow reach here, handle them gracefully.
        Request::PauseSession { session_id } => {
            handlers::pause_session::handle(&ctx, session_id).await
        }

        Request::ResumeSession { session_id } => {
            handlers::resume_session::handle(&ctx, session_id).await
        }
    }
}
