use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use godly_protocol::{DaemonMessage, Event, Request, Response};

use crate::session::DaemonSession;

/// Named pipe server that manages daemon sessions and client connections.
pub struct DaemonServer {
    sessions: Arc<RwLock<HashMap<String, DaemonSession>>>,
    running: Arc<AtomicBool>,
    has_clients: Arc<AtomicBool>,
}

impl DaemonServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(true)),
            has_clients: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run the server, listening for connections on the named pipe.
    /// Returns when the server should shut down (idle timeout or explicit stop).
    pub async fn run(&self) {
        eprintln!("[daemon] Server starting on {}", godly_protocol::PIPE_NAME);

        // Start process monitor
        self.start_process_monitor();

        // Start idle timeout checker
        let running = self.running.clone();
        let sessions = self.sessions.clone();
        let has_clients = self.has_clients.clone();
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
                let no_clients = !has_clients.load(Ordering::Relaxed);
                let elapsed = last_activity_for_timeout.read().elapsed();

                if sessions_empty && no_clients && elapsed > idle_timeout {
                    eprintln!("[daemon] Idle timeout reached, shutting down");
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
                    self.has_clients.store(true, Ordering::Relaxed);

                    let sessions = self.sessions.clone();
                    let running = self.running.clone();
                    let has_clients = self.has_clients.clone();
                    let activity = last_activity.clone();

                    tokio::spawn(async move {
                        handle_client(pipe, sessions, running, activity).await;
                        // Client disconnected - we don't track individual clients count,
                        // just set has_clients to false. If another client exists it will
                        // set it back to true on its next interaction.
                        has_clients.store(false, Ordering::Relaxed);
                    });
                }
                Err(e) => {
                    if self.running.load(Ordering::Relaxed) {
                        eprintln!("[daemon] Accept error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }

        eprintln!("[daemon] Server shutting down");
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

        let pipe_name: Vec<u16> = OsStr::new(godly_protocol::PIPE_NAME)
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
                    4096,
                    4096,
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

/// Non-blocking check: how many bytes are available to read from the pipe?
#[cfg(windows)]
fn peek_pipe(handle: isize) -> u32 {
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
        return 0; // Error (pipe might be closed)
    }
    bytes_available
}

#[cfg(not(windows))]
fn peek_pipe(_handle: isize) -> u32 {
    0
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
    while let Some(request) = req_rx.recv().await {
        *last_activity.write() = Instant::now();
        eprintln!("[daemon] Received request: {:?}", request);

        let response = handle_request(
            &request,
            &sessions,
            &msg_tx,
            &attached_sessions,
        )
        .await;

        // Send response back to I/O thread for writing to pipe
        let msg = DaemonMessage::Response(response);
        if msg_tx.send(msg).is_err() {
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
            }
        }
    }
}

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

    while io_running.load(Ordering::Relaxed) && server_running.load(Ordering::Relaxed) {
        // Step 1: Check if there are bytes available to read (non-blocking)
        let bytes_available = peek_pipe(raw_handle);

        if bytes_available > 0 {
            // Read the request from the pipe
            match godly_protocol::read_message::<_, Request>(&mut pipe) {
                Ok(Some(request)) => {
                    if req_tx.send(request).is_err() {
                        eprintln!("[daemon-io] Request channel closed, stopping");
                        break;
                    }
                }
                Ok(None) => {
                    eprintln!("[daemon-io] Client disconnected (EOF)");
                    break;
                }
                Err(e) => {
                    if io_running.load(Ordering::Relaxed) {
                        eprintln!("[daemon-io] Read error: {}", e);
                    }
                    break;
                }
            }
            continue; // Check for more data immediately
        }

        // Step 2: Check if there are outgoing messages to write
        match msg_rx.try_recv() {
            Ok(msg) => {
                if godly_protocol::write_message(&mut pipe, &msg).is_err() {
                    eprintln!("[daemon-io] Write error, stopping");
                    break;
                }
                continue; // Check for more messages immediately
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // Nothing to do - brief sleep to avoid busy-waiting
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                eprintln!("[daemon-io] Message channel disconnected, stopping");
                break;
            }
        }
    }

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
        } => {
            match DaemonSession::new(id.clone(), shell_type.clone(), cwd.clone(), *rows, *cols) {
                Ok(session) => {
                    let info = session.info();
                    sessions.write().insert(id.clone(), session);
                    eprintln!("[daemon] Created session {}", id);
                    Response::SessionCreated { session: info }
                }
                Err(e) => {
                    eprintln!("[daemon] Failed to create session {}: {}", id, e);
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
                    Response::Ok
                }
                None => Response::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }
    }
}
