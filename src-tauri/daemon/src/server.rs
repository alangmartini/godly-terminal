use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use godly_protocol::{DaemonMessage, Event, Request, Response};

use crate::debug_log::daemon_log;
use crate::session::DaemonSession;

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
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                let sessions_empty = sessions.read().is_empty();
                let num_clients = client_count.load(Ordering::Relaxed);
                let elapsed = last_activity_for_timeout.read().elapsed();

                if sessions_empty && num_clients == 0 && elapsed > idle_timeout {
                    eprintln!("[daemon] Idle timeout reached, shutting down");
                    daemon_log!("Idle timeout reached (no sessions, no clients for {:?}), shutting down", elapsed);
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

                    daemon_log!("Client connected, spawning handler (clients={})", self.client_count.load(Ordering::Relaxed));

                    tokio::spawn(async move {
                        handle_client(pipe, sessions, running, activity).await;
                        client_count.fetch_sub(1, Ordering::Relaxed);
                        daemon_log!("Client disconnected (clients={})", client_count.load(Ordering::Relaxed));
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

    // Channel: async handler -> I/O thread (outgoing messages to client)
    let (msg_tx, msg_rx) = mpsc::unbounded_channel::<DaemonMessage>();

    // Signal to stop the I/O thread when the async handler is done
    let io_running = Arc::new(AtomicBool::new(true));

    // Spawn the single I/O thread that does ALL pipe reads and writes
    let io_running_clone = io_running.clone();
    let running_clone = running.clone();
    let io_handle = tokio::task::spawn_blocking(move || {
        io_thread(pipe, req_tx, msg_rx, io_running_clone, running_clone);
    });

    // Async handler loop: process requests from the I/O thread
    eprintln!("[daemon] Entering request loop for client");
    daemon_log!("Entering request loop for client");
    while let Some(request) = req_rx.recv().await {
        *last_activity.write() = Instant::now();
        daemon_log!("Received request: {:?}", request);

        let response = handle_request(
            &request,
            &sessions,
            &msg_tx,
            &attached_sessions,
        )
        .await;

        daemon_log!("Sending response for {:?}", std::mem::discriminant(&request));

        // Send response back to I/O thread for writing to pipe
        let msg = DaemonMessage::Response(response);
        if msg_tx.send(msg).is_err() {
            daemon_log!("msg_tx send failed, breaking handler loop");
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
const MAX_WRITES_PER_ITERATION: usize = 8;

/// Single I/O thread: performs all pipe reads and writes.
/// Uses PeekNamedPipe for non-blocking read checks to avoid deadlock.
fn io_thread(
    mut pipe: std::fs::File,
    req_tx: mpsc::UnboundedSender<Request>,
    mut msg_rx: mpsc::UnboundedReceiver<DaemonMessage>,
    io_running: Arc<AtomicBool>,
    server_running: Arc<AtomicBool>,
) {
    let raw_handle = get_raw_handle(&pipe);
    let mut last_log_time = Instant::now();
    let mut total_reads: u64 = 0;
    let mut total_writes: u64 = 0;
    let mut total_bytes_written: u64 = 0;
    let mut write_stall_count: u64 = 0;

    daemon_log!("io_thread started, handle={}", raw_handle);

    while io_running.load(Ordering::Relaxed) && server_running.load(Ordering::Relaxed) {
        let mut did_work = false;

        // Step 1: ALWAYS check for incoming data first (requests from client).
        // This ensures user input (Write, Resize) is never starved by outgoing events.
        match peek_pipe(raw_handle) {
            PeekResult::Data => {
                // Read the request from the pipe
                let read_start = Instant::now();
                match godly_protocol::read_message::<_, Request>(&mut pipe) {
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
                        if req_tx.send(request).is_err() {
                            eprintln!("[daemon-io] Request channel closed, stopping");
                            daemon_log!("Request channel closed, stopping");
                            break;
                        }
                        did_work = true;
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

        // Step 2: Write outgoing messages, but limit per iteration to avoid
        // starving reads. If we have many queued events and a slow pipe,
        // writing them all would block and prevent reading new requests.
        let mut writes_this_iteration = 0;
        while writes_this_iteration < MAX_WRITES_PER_ITERATION {
            match msg_rx.try_recv() {
                Ok(msg) => {
                    let write_start = Instant::now();
                    let msg_kind = match &msg {
                        DaemonMessage::Response(_) => "Response",
                        DaemonMessage::Event(Event::Output { .. }) => "Output",
                        DaemonMessage::Event(Event::SessionClosed { .. }) => "SessionClosed",
                        DaemonMessage::Event(Event::ProcessChanged { .. }) => "ProcessChanged",
                    };

                    if godly_protocol::write_message(&mut pipe, &msg).is_err() {
                        eprintln!("[daemon-io] Write error, stopping");
                        daemon_log!("Write error on {}, stopping", msg_kind);
                        // Set io_running to false so the outer loop exits
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
                    eprintln!("[daemon-io] Message channel disconnected, stopping");
                    daemon_log!("Message channel disconnected, stopping");
                    io_running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }

        // If we hit the write limit, check for incoming data before writing more
        if writes_this_iteration >= MAX_WRITES_PER_ITERATION {
            daemon_log!(
                "Write batch limit hit ({}/{}), checking for incoming data",
                writes_this_iteration,
                MAX_WRITES_PER_ITERATION
            );
            continue; // Loop back to peek for incoming requests
        }

        if !did_work {
            // Nothing to do - brief sleep to avoid busy-waiting
            std::thread::sleep(Duration::from_millis(1));
        }

        // Periodic stats logging
        if last_log_time.elapsed() > Duration::from_secs(30) {
            daemon_log!(
                "io_thread stats: reads={}, writes={}, bytes_out={}, stalls={}",
                total_reads,
                total_writes,
                total_bytes_written,
                write_stall_count
            );
            last_log_time = Instant::now();
        }
    }

    daemon_log!(
        "io_thread stopped: reads={}, writes={}, bytes_out={}, stalls={}",
        total_reads,
        total_writes,
        total_bytes_written,
        write_stall_count
    );
    eprintln!("[daemon-io] I/O thread stopped");
}

async fn handle_request(
    request: &Request,
    sessions: &Arc<RwLock<HashMap<String, DaemonSession>>>,
    msg_tx: &mpsc::UnboundedSender<DaemonMessage>,
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
                    eprintln!("[daemon] Created session {}", id);
                    daemon_log!("Created session {}", id);
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
                    let (buffer, mut rx) = session.attach();
                    attached_sessions.write().push(session_id.clone());

                    // Spawn a task to forward live output as events
                    let tx = msg_tx.clone();
                    let sid = session_id.clone();
                    tokio::spawn(async move {
                        while let Some(data) = rx.recv().await {
                            let event = DaemonMessage::Event(Event::Output {
                                session_id: sid.clone(),
                                data,
                            });
                            if tx.send(event).is_err() {
                                break;
                            }
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
            let sessions_guard = sessions.read();
            match sessions_guard.get(session_id) {
                Some(session) => match session.write(data) {
                    Ok(()) => Response::Ok,
                    Err(e) => Response::Error { message: e },
                },
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
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

        Request::CloseSession { session_id } => {
            let mut sessions_guard = sessions.write();
            match sessions_guard.remove(session_id) {
                Some(session) => {
                    session.close();
                    attached_sessions.write().retain(|id| id != session_id);
                    eprintln!("[daemon] Closed session {}", session_id);
                    daemon_log!("Closed session {}", session_id);
                    Response::Ok
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }
    }
}
