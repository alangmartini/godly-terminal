use std::collections::HashMap;
use std::io::{Read, Write};
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
                Ok((reader, writer)) => {
                    *last_activity.write() = Instant::now();
                    self.has_clients.store(true, Ordering::Relaxed);

                    let sessions = self.sessions.clone();
                    let running = self.running.clone();
                    let has_clients = self.has_clients.clone();
                    let activity = last_activity.clone();

                    tokio::spawn(async move {
                        handle_client(reader, writer, sessions, running, activity).await;
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

    /// Accept a single named pipe connection (Windows implementation)
    #[cfg(windows)]
    async fn accept_connection(
        &self,
    ) -> Result<(Box<dyn Read + Send>, Box<dyn Write + Send>), String> {
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

            // Create reader/writer from the pipe handle
            use std::os::windows::io::FromRawHandle;
            let reader: Box<dyn Read + Send> =
                Box::new(unsafe { std::fs::File::from_raw_handle(handle as _) });

            // We need to duplicate the handle for the writer since File takes ownership
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

            Ok((reader, writer))
        })
        .await
        .map_err(|e| format!("Spawn blocking failed: {}", e))?;

        result
    }

    #[cfg(not(windows))]
    async fn accept_connection(
        &self,
    ) -> Result<(Box<dyn Read + Send>, Box<dyn Write + Send>), String> {
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

/// Handle a single client connection
async fn handle_client(
    mut reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    sessions: Arc<RwLock<HashMap<String, DaemonSession>>>,
    running: Arc<AtomicBool>,
    last_activity: Arc<RwLock<Instant>>,
) {
    // Track which sessions this client has attached to
    let mut attached_sessions: Vec<String> = Vec::new();

    // Channel for sending daemon messages back to client
    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<DaemonMessage>();

    // Spawn a writer task that serializes messages to the pipe
    let _writer_handle = {
        let writer = Arc::new(parking_lot::Mutex::new(writer));
        let writer_clone = writer.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let mut w = writer_clone.lock();
                if godly_protocol::write_message(&mut *w, &msg).is_err() {
                    break;
                }
            }
        });
        writer
    };

    // Read requests from client
    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        // Read request in blocking thread
        let reader_result = tokio::task::spawn_blocking({
            // We need to move reader into the closure, then get it back
            let mut reader_opt = Some(reader);
            move || {
                let mut r = reader_opt.take().unwrap();
                let result = godly_protocol::read_message::<_, Request>(&mut r);
                (r, result)
            }
        })
        .await;

        match reader_result {
            Ok((r, Ok(Some(request)))) => {
                reader = r;
                *last_activity.write() = Instant::now();

                let response = handle_request(
                    &request,
                    &sessions,
                    &msg_tx,
                    &mut attached_sessions,
                )
                .await;

                // Send response
                let msg = DaemonMessage::Response(response);
                if msg_tx.send(msg).is_err() {
                    break;
                }
            }
            Ok((_, Ok(None))) => {
                // Client disconnected (EOF)
                eprintln!("[daemon] Client disconnected (EOF)");
                break;
            }
            Ok((_, Err(e))) => {
                eprintln!("[daemon] Read error: {}", e);
                break;
            }
            Err(e) => {
                eprintln!("[daemon] Spawn blocking error: {}", e);
                break;
            }
        }
    }

    // Client disconnected - detach all sessions (they keep running)
    {
        let sessions_guard = sessions.read();
        for session_id in &attached_sessions {
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

async fn handle_request(
    request: &Request,
    sessions: &Arc<RwLock<HashMap<String, DaemonSession>>>,
    msg_tx: &mpsc::UnboundedSender<DaemonMessage>,
    attached_sessions: &mut Vec<String>,
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
                    attached_sessions.push(session_id.clone());

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
                    attached_sessions.retain(|id| id != session_id);
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
                    attached_sessions.retain(|id| id != session_id);
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
