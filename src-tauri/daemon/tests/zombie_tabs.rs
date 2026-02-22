//! Zombie terminal tab tests: verify that SessionClosed events are emitted
//! when a PTY process exits.
//!
//! Bug A2: When a shell process exits (user types `exit`, process crashes, PTY
//! reads EOF), the daemon's reader thread detects the EOF but the frontend is
//! never notified via SessionClosed. The terminal tab stays open forever,
//! polling for grid data and getting "Session not found" errors.
//!
//! Root cause: SessionClosed is only sent by the forwarding task when the
//! output channel closes with running=false. But if the client disconnects
//! before the PTY exits, the forwarding task exits early (running=true at
//! that point), and when the PTY later exits, nobody sends SessionClosed.
//! Additionally, attaching to an already-dead session never triggers
//! SessionClosed because the reader thread is dead and never closes the
//! new output channel.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test zombie_tabs -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Event, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// Helpers (DaemonFixture pattern — see handler_starvation.rs)
// ---------------------------------------------------------------------------

fn connect_pipe(pipe_name: &str, timeout: Duration) -> std::fs::File {
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let start = Instant::now();
    loop {
        let handle = unsafe {
            CreateFileW(
                wide_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle != INVALID_HANDLE_VALUE {
            return unsafe { std::fs::File::from_raw_handle(handle as _) };
        }

        if start.elapsed() > timeout {
            let err = unsafe { GetLastError() };
            panic!(
                "Failed to connect to pipe '{}' within {:?} (error: {})",
                pipe_name, timeout, err
            );
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Non-blocking check for data available on a pipe handle.
fn pipe_has_data(pipe: &std::fs::File) -> bool {
    use winapi::um::namedpipeapi::PeekNamedPipe;

    let handle = pipe.as_raw_handle();
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
    result != 0 && bytes_available > 0
}

/// Send a request and read the response with a deadline. Skips events.
/// Collects skipped events for the caller to inspect if needed.
fn send_request_deadline(
    pipe: &mut std::fs::File,
    request: &Request,
    deadline: Duration,
) -> Result<(Response, Vec<Event>), String> {
    godly_protocol::write_request(pipe, request)
        .map_err(|e| format!("Failed to write request: {}", e))?;

    let start = Instant::now();
    let mut skipped_events = Vec::new();

    loop {
        if start.elapsed() > deadline {
            return Err(format!(
                "Deadline exceeded ({:?}): no response received",
                deadline
            ));
        }

        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }

        let msg: DaemonMessage = godly_protocol::read_daemon_message(pipe)
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Unexpected EOF".to_string())?;

        match msg {
            DaemonMessage::Response(resp) => return Ok((resp, skipped_events)),
            DaemonMessage::Event(event) => {
                skipped_events.push(event);
                continue;
            }
        }
    }
}

/// Send a request and read the response (blocking with 30s deadline). Skips events.
fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
    match send_request_deadline(pipe, request, Duration::from_secs(30)) {
        Ok((resp, _)) => resp,
        Err(e) => panic!("send_request failed: {}", e),
    }
}

/// Poll ListSessions until a specific session reports running=false.
fn wait_for_session_dead(
    pipe: &mut std::fs::File,
    session_id: &str,
    deadline: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    loop {
        if start.elapsed() > deadline {
            return Err(format!(
                "Deadline exceeded ({:?}): session {} still running",
                deadline, session_id
            ));
        }

        let (resp, _events) = send_request_deadline(
            pipe,
            &Request::ListSessions,
            Duration::from_secs(5),
        )?;

        if let Response::SessionList { sessions } = resp {
            if let Some(session) = sessions.iter().find(|s| s.id == session_id) {
                if !session.running {
                    return Ok(());
                }
            }
        }

        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Wait for a specific event type from the daemon, with a deadline.
/// Returns the event if found before the deadline, or Err if timeout.
fn wait_for_session_closed(
    pipe: &mut std::fs::File,
    session_id: &str,
    deadline: Duration,
) -> Result<(), String> {
    let start = Instant::now();

    loop {
        if start.elapsed() > deadline {
            return Err(format!(
                "Deadline exceeded ({:?}): no SessionClosed received for session {}",
                deadline, session_id
            ));
        }

        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }

        let msg: DaemonMessage = godly_protocol::read_daemon_message(pipe)
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Unexpected EOF".to_string())?;

        match msg {
            DaemonMessage::Event(Event::SessionClosed { session_id: sid, .. }) if sid == session_id => {
                return Ok(());
            }
            _ => continue, // Skip other events
        }
    }
}

struct DaemonFixture {
    child: Child,
    pipe_name: String,
}

impl DaemonFixture {
    fn spawn(test_name: &str) -> Self {
        let pipe_name = format!(
            r"\\.\pipe\godly-test-{}-{}",
            test_name,
            std::process::id()
        );

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir
            .parent()
            .unwrap()
            .join("target")
            .join("debug");
        let daemon_exe = target_dir.join("godly-daemon.exe");
        assert!(
            daemon_exe.exists(),
            "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
            daemon_exe
        );

        let child = Command::new(&daemon_exe)
            .env("GODLY_PIPE_NAME", &pipe_name)
            .env("GODLY_NO_DETACH", "1")
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn daemon");

        std::thread::sleep(Duration::from_millis(500));

        Self { child, pipe_name }
    }

    fn connect(&self) -> std::fs::File {
        connect_pipe(&self.pipe_name, Duration::from_secs(5))
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug A2: When a shell process exits while a client is attached, the client
/// should receive a SessionClosed event. This is the "happy path" — the client
/// is attached when the PTY dies.
///
/// Steps:
/// 1. Create a cmd.exe session
/// 2. Attach to it
/// 3. Make cmd.exe exit by writing "exit\r\n"
/// 4. Wait for session to become dead (via ListSessions)
/// 5. Verify SessionClosed event arrives within a deadline
#[test]
#[ntest::timeout(60_000)] // 1min — daemon spawn + PTY exit detection
fn test_session_closed_on_pty_exit_while_attached() {
    let daemon = DaemonFixture::spawn("zombie-attached");
    let mut pipe = daemon.connect();

    let session_id = "test-zombie-attached".to_string();

    // Create session
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Cmd,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    // Attach
    let resp = send_request(&mut pipe, &Request::Attach { session_id: session_id.clone() });
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Attach failed: {:?}",
        resp
    );

    // Wait for the shell to initialize (cmd.exe startup)
    std::thread::sleep(Duration::from_secs(1));

    // Make the shell exit
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: b"exit\r\n".to_vec(),
        },
    );
    assert!(
        matches!(resp, Response::Ok),
        "Write failed: {:?}",
        resp
    );

    // Wait for SessionClosed event
    let result = wait_for_session_closed(&mut pipe, &session_id, Duration::from_secs(10));
    assert!(
        result.is_ok(),
        "Bug A2: SessionClosed not received after PTY exit: {}",
        result.unwrap_err()
    );
}

/// Bug A2: When a client attaches to a session whose PTY has already exited
/// (running=false), the client should receive a SessionClosed event immediately
/// (or very quickly). This covers the case where the PTY dies while no client
/// is attached, and the client reconnects later.
///
/// Steps:
/// 1. Create a cmd.exe session and attach
/// 2. Make cmd.exe exit
/// 3. Wait for SessionClosed
/// 4. Detach
/// 5. Connect a new client and re-attach to the same session
/// 6. Verify SessionClosed arrives again for the new client
#[test]
#[ntest::timeout(60_000)]
fn test_session_closed_on_attach_to_dead_session() {
    let daemon = DaemonFixture::spawn("zombie-dead-attach");
    let mut pipe1 = daemon.connect();

    let session_id = "test-zombie-dead".to_string();

    // Create session
    let resp = send_request(
        &mut pipe1,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Cmd,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    // Attach
    let resp = send_request(&mut pipe1, &Request::Attach { session_id: session_id.clone() });
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Attach failed: {:?}",
        resp
    );

    std::thread::sleep(Duration::from_secs(1));

    // Make the shell exit
    let resp = send_request(
        &mut pipe1,
        &Request::Write {
            session_id: session_id.clone(),
            data: b"exit\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok), "Write failed: {:?}", resp);

    // Wait for SessionClosed on the first client
    let result = wait_for_session_closed(&mut pipe1, &session_id, Duration::from_secs(10));
    assert!(
        result.is_ok(),
        "First client did not get SessionClosed: {}",
        result.unwrap_err()
    );

    // Now connect a second client and attach to the dead session
    let mut pipe2 = daemon.connect();

    let resp = send_request(&mut pipe2, &Request::Attach { session_id: session_id.clone() });
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Re-attach failed: {:?}",
        resp
    );

    // The second client should also get SessionClosed since the session is dead
    let result = wait_for_session_closed(&mut pipe2, &session_id, Duration::from_secs(5));
    assert!(
        result.is_ok(),
        "Bug A2: Second client did not get SessionClosed for dead session: {}",
        result.unwrap_err()
    );
}

/// Diagnostic: Verify that cmd.exe actually exits and the reader thread detects it.
/// If this fails, cmd.exe isn't exiting from "exit\r\n" via ConPTY.
#[test]
#[ntest::timeout(60_000)]
fn test_cmd_exit_is_detected_by_daemon() {
    let daemon = DaemonFixture::spawn("zombie-diag");
    let mut pipe = daemon.connect();

    let session_id = "test-zombie-diag".to_string();

    // Create session
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Cmd,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach
    let resp = send_request(&mut pipe, &Request::Attach { session_id: session_id.clone() });
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    std::thread::sleep(Duration::from_secs(1));

    // Make the shell exit
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: b"exit\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Poll ListSessions to check if the session dies
    let result = wait_for_session_dead(&mut pipe, &session_id, Duration::from_secs(10));
    assert!(
        result.is_ok(),
        "cmd.exe did not exit (session still shows running=true): {}",
        result.unwrap_err()
    );
}

/// Bug A2: ListSessions should report running=false for a session whose PTY
/// has exited, allowing the frontend to know the session is dead even without
/// the SessionClosed event.
#[test]
#[ntest::timeout(60_000)]
fn test_list_sessions_shows_dead_session() {
    let daemon = DaemonFixture::spawn("zombie-list");
    let mut pipe = daemon.connect();

    let session_id = "test-zombie-list".to_string();

    // Create and attach
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Cmd,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request(&mut pipe, &Request::Attach { session_id: session_id.clone() });
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    std::thread::sleep(Duration::from_secs(1));

    // Kill the shell
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: b"exit\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Wait for SessionClosed
    let result = wait_for_session_closed(&mut pipe, &session_id, Duration::from_secs(10));
    assert!(result.is_ok(), "SessionClosed not received: {}", result.unwrap_err());

    // ListSessions should show running=false
    let resp = send_request(&mut pipe, &Request::ListSessions);
    match resp {
        Response::SessionList { sessions } => {
            let session = sessions.iter().find(|s| s.id == session_id);
            assert!(
                session.is_some(),
                "Dead session should still be in the session list"
            );
            let session = session.unwrap();
            assert!(
                !session.running,
                "Bug A2: Dead session shows running=true in ListSessions"
            );
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }
}
