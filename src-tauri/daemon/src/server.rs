use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use godly_protocol::{DaemonMessage, Event, Request, Response, read_request, write_daemon_message};

use crate::debug_log::daemon_log;
use crate::session::DaemonSession;

/// Log current process memory usage (Windows: working set via GetProcessMemoryInfo).
#[cfg(windows)]
fn log_memory_usage(label: &str) {
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
fn log_memory_usage(label: &str) {
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
}

impl DaemonServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(true)),
            client_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Run the server, listening for connections on the named pipe.
    /// Returns when the server should shut down (idle timeout or explicit stop).
    pub async fn run(&self) {
        let pipe_name = godly_protocol::pipe_name();
        eprintln!("[daemon] Server starting on {}", pipe_name);
        daemon_log!("Server starting on {}", pipe_name);

        // Start process monitor
        self.start_process_monitor();

        // Start idle timeout checker
        let running = self.running.clone();
        let sessions = self.sessions.clone();
        let client_count = self.client_count.clone();
        let last_activity = Arc::new(RwLock::new(Instant::now()));
        let last_activity_for_timeout = last_activity.clone();

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

/// Number of idle loop iterations to spin (yield_now) before falling back to sleep.
/// During active I/O this keeps latency near-zero; when truly idle, the sleep
/// prevents burning CPU. On Windows, thread::sleep(1ms) can actually sleep ~15ms
/// due to the default timer resolution (15.625ms), so avoiding sleep during
/// active use is critical for input responsiveness.
const SPIN_BEFORE_SLEEP: u32 = 100;

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

    // Adaptive polling: spin (yield_now) before falling back to sleep.
    // On Windows, thread::sleep(1ms) can actually sleep ~15ms due to the
    // default system timer resolution (15.625ms). Spinning avoids this.
    let mut idle_count: u32 = 0;

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
                                let response = {
                                    let sessions_guard = sessions.read();
                                    match sessions_guard.get(session_id) {
                                        Some(session) => match session.resize(*rows, *cols) {
                                            Ok(()) => Response::Ok,
                                            Err(e) => Response::Error { message: e },
                                        },
                                        None => Response::Error {
                                            message: format!("Session {} not found", session_id),
                                        },
                                    }
                                };
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
                    if let DaemonMessage::Event(Event::Output { ref data, .. }) = msg {
                        total_bytes_written += data.len() as u64;
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
            // Adaptive polling: spin for a while before sleeping.
            // On Windows, thread::sleep(1ms) can actually sleep ~15ms due to
            // the default system timer resolution (15.625ms). During active I/O,
            // spinning avoids this penalty. When truly idle, we fall back to
            // sleep to avoid burning CPU.
            idle_count += 1;
            if idle_count > SPIN_BEFORE_SLEEP {
                std::thread::sleep(Duration::from_millis(1));
            } else {
                std::thread::yield_now();
            }
        } else {
            idle_count = 0;
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
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
        let (event_tx, mut event_rx) = mpsc::channel::<DaemonMessage>(16);
        let running_flag = Arc::new(AtomicBool::new(true));
        let sid = "test-fwd-closed".to_string();

        let flag = running_flag.clone();
        let handle = tokio::spawn({
            let sid = sid.clone();
            async move {
                while let Some(data) = rx.recv().await {
                    let event = DaemonMessage::Event(Event::Output {
                        session_id: sid.clone(),
                        data,
                    });
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
                if !flag.load(Ordering::Relaxed) {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                        }))
                        .await;
                }
            }
        });

        // Send some output, then simulate PTY exit
        tx.send(b"hello".to_vec()).await.unwrap();
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
            DaemonMessage::Event(Event::SessionClosed { session_id }) => {
                assert_eq!(session_id, sid);
            }
            other => panic!("expected SessionClosed, got {:?}", std::mem::discriminant(&other)),
        }
    }

    /// When the channel closes with running=true (client detached), the
    /// forwarding task should NOT send SessionClosed.
    #[tokio::test]
    async fn test_forwarding_no_session_closed_on_detach() {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
        let (event_tx, mut event_rx) = mpsc::channel::<DaemonMessage>(16);
        let running_flag = Arc::new(AtomicBool::new(true));
        let sid = "test-fwd-detach".to_string();

        let flag = running_flag.clone();
        let handle = tokio::spawn({
            let sid = sid.clone();
            async move {
                while let Some(data) = rx.recv().await {
                    let event = DaemonMessage::Event(Event::Output {
                        session_id: sid.clone(),
                        data,
                    });
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
                if !flag.load(Ordering::Relaxed) {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                        }))
                        .await;
                }
            }
        });

        // Send some output, then simulate detach (running stays true)
        tx.send(b"hello".to_vec()).await.unwrap();
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
        let (_tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
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
                        }))
                        .await;
                    return;
                }
                while let Some(data) = rx.recv().await {
                    let event = DaemonMessage::Event(Event::Output {
                        session_id: sid.clone(),
                        data,
                    });
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
                if !flag.load(Ordering::Relaxed) {
                    let _ = event_tx
                        .send(DaemonMessage::Event(Event::SessionClosed {
                            session_id: sid,
                        }))
                        .await;
                }
            }
        });

        handle.await.unwrap();

        let msg = event_rx.recv().await.unwrap();
        match msg {
            DaemonMessage::Event(Event::SessionClosed { session_id }) => {
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
    match request {
        Request::Ping => Response::Pong,

        Request::CreateSession {
            id,
            shell_type,
            cwd,
            rows,
            cols,
            env,
        } => {
            match DaemonSession::new(id.clone(), shell_type.clone(), cwd.clone(), *rows, *cols, env.clone()) {
                Ok(session) => {
                    let info = session.info();
                    sessions.write().insert(id.clone(), session);
                    let session_count = sessions.read().len();
                    eprintln!("[daemon] Created session {}", id);
                    daemon_log!("Created session {} (total sessions: {})", id, session_count);
                    log_memory_usage(&format!("session_create({})", session_count));
                    Response::SessionCreated { session: info }
                }
                Err(e) => {
                    eprintln!("[daemon] Failed to create session {}: {}", id, e);
                    daemon_log!("Failed to create session {}: {}", id, e);
                    Response::Error { message: e }
                }
            }
        }

        Request::ListSessions => {
            let sessions_guard = sessions.read();
            let list: Vec<_> = sessions_guard.values().map(|s| s.info()).collect();
            Response::SessionList { sessions: list }
        }

        Request::Attach { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    let is_already_dead = !session.is_running();
                    let (buffer, mut rx) = session.attach();
                    attached_sessions.write().push(session_id.clone());

                    // Spawn a task to forward live output as events.
                    // When the channel closes, check if the PTY exited (running == false)
                    // and send SessionClosed so the client knows the process is dead.
                    let tx = msg_tx.clone();
                    let sid = session_id.clone();
                    let running_flag = session.running_flag();
                    tokio::spawn(async move {
                        // Bug A2 fix: if the session is already dead when we attach,
                        // send SessionClosed immediately. The reader thread and child
                        // monitor are already gone, so the output channel will never
                        // close on its own — rx.recv() would block forever.
                        if is_already_dead {
                            daemon_log!(
                                "Session {} already dead at attach time, sending SessionClosed",
                                sid
                            );
                            let _ = tx
                                .send(DaemonMessage::Event(Event::SessionClosed {
                                    session_id: sid,
                                }))
                                .await;
                            return;
                        }

                        while let Some(data) = rx.recv().await {
                            let event = DaemonMessage::Event(Event::Output {
                                session_id: sid.clone(),
                                data,
                            });
                            if tx.send(event).await.is_err() {
                                break;
                            }
                        }
                        // Channel closed — check why:
                        // - running == false → PTY exited → notify client
                        // - running == true  → client detached → session still alive, don't notify
                        if !running_flag.load(Ordering::Relaxed) {
                            daemon_log!("Session {} PTY exited, sending SessionClosed", sid);
                            let _ = tx
                                .send(DaemonMessage::Event(Event::SessionClosed {
                                    session_id: sid,
                                }))
                                .await;
                        }
                    });

                    eprintln!(
                        "[daemon] Attached to session {} (buffer: {} bytes)",
                        session_id,
                        buffer.len()
                    );
                    daemon_log!(
                        "Attached to session {} (buffer: {} bytes)",
                        session_id,
                        buffer.len()
                    );

                    // Return buffered data for replay
                    if buffer.is_empty() {
                        Response::Ok
                    } else {
                        Response::Buffer {
                            session_id: session_id.clone(),
                            data: buffer,
                        }
                    }
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::Detach { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    session.detach();
                    attached_sessions.write().retain(|id| id != session_id);
                    eprintln!("[daemon] Detached from session {}", session_id);
                    daemon_log!("Detached from session {}", session_id);
                    Response::Ok
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::Write { session_id, data } => {
            // Fire-and-forget: spawn_blocking so write_all() never blocks
            // the async handler or the I/O thread. This breaks the circular
            // deadlock when ConPTY input fills during heavy output.
            // Write ordering is preserved by session.writer Mutex.
            let sessions = sessions.clone();
            let session_id = session_id.clone();
            let data = data.clone();
            tokio::task::spawn_blocking(move || {
                let sessions_guard = sessions.read();
                if let Some(session) = sessions_guard.get(&session_id) {
                    if let Err(e) = session.write(&data) {
                        daemon_log!("Write failed for session {}: {}", session_id, e);
                    }
                }
            });
            Response::Ok
        }

        Request::Resize {
            session_id,
            rows,
            cols,
        } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => match session.resize(*rows, *cols) {
                    Ok(()) => Response::Ok,
                    Err(e) => Response::Error { message: e },
                },
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::ReadBuffer { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    let data = session.read_output_history();
                    Response::Buffer {
                        session_id: session_id.clone(),
                        data,
                    }
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::GetLastOutputTime { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => Response::LastOutputTime {
                    epoch_ms: session.last_output_epoch_ms(),
                    running: session.is_running(),
                },
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::SearchBuffer {
            session_id,
            text,
            strip_ansi,
        } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => Response::SearchResult {
                    found: session.search_output_history(text, *strip_ansi),
                    running: session.is_running(),
                },
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::ReadGrid { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    let grid = session.read_grid();
                    Response::Grid { grid }
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::ReadRichGrid { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    let grid = session.read_rich_grid();
                    Response::RichGrid { grid }
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::ReadRichGridDiff { session_id } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    let diff = session.read_rich_grid_diff();
                    Response::RichGridDiff { diff }
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::ReadGridText {
            session_id,
            start_row,
            start_col,
            end_row,
            end_col,
        } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    let text = session.read_grid_text(*start_row, *start_col, *end_row, *end_col);
                    Response::GridText { text }
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::SetScrollback { session_id, offset } => {
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => {
                    session.set_scrollback(*offset);
                    Response::Ok
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        Request::CloseSession { session_id } => {
            let mut sessions_guard = sessions.write();
            match sessions_guard.remove(session_id) {
                Some(session) => {
                    session.close();
                    attached_sessions.write().retain(|id| id != session_id);
                    let remaining = sessions_guard.len(); // already removed
                    eprintln!("[daemon] Closed session {}", session_id);
                    daemon_log!("Closed session {} (remaining sessions: {})", session_id, remaining);
                    log_memory_usage(&format!("session_close({})", remaining));
                    Response::Ok
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }
    }
}
