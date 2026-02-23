//! Pause/Resume test: verify that paused sessions suppress output events
//! while keeping the VT parser up-to-date, and that resume restores streaming.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test pause_resume -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Event, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// Helpers (DaemonFixture pattern — see handler_starvation.rs)
// ---------------------------------------------------------------------------

fn connect_pipe(pipe_name: &str, timeout: Duration) -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
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
            panic!("Failed to connect to pipe within {:?} (error: {})", timeout, err);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

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

/// Send a request and read the response (blocking). For setup/control only.
fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
    godly_protocol::write_request(pipe, request).expect("Failed to write request");
    loop {
        let msg = godly_protocol::read_daemon_message(pipe)
            .expect("Failed to read message")
            .expect("Unexpected EOF");
        match msg {
            DaemonMessage::Response(resp) => return resp,
            DaemonMessage::Event(_) => continue,
        }
    }
}

/// Create a session with retries (shim pipe can be flaky under parallel test runs).
fn create_session_with_retry(
    pipe: &mut std::fs::File,
    session_id: &str,
    max_attempts: u32,
) -> Response {
    for attempt in 1..=max_attempts {
        let resp = send_request(
            pipe,
            &Request::CreateSession {
                id: session_id.to_string(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            },
        );
        match &resp {
            Response::SessionCreated { .. } => return resp,
            Response::Error { message } if attempt < max_attempts => {
                eprintln!("CreateSession attempt {} failed: {}, retrying...", attempt, message);
                std::thread::sleep(Duration::from_millis(500 * u64::from(attempt)));
            }
            _ => return resp,
        }
    }
    unreachable!()
}

/// Drain all pending events from the pipe, returning the count.
fn drain_events(pipe: &mut std::fs::File, timeout: Duration) -> u32 {
    let start = Instant::now();
    let mut count = 0u32;
    while start.elapsed() < timeout {
        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        match godly_protocol::read_daemon_message(pipe) {
            Ok(Some(DaemonMessage::Event(_))) => count += 1,
            _ => break,
        }
    }
    count
}

/// Count Output/GridDiff events for a specific session during a window.
fn count_session_events(pipe: &mut std::fs::File, session_id: &str, window: Duration) -> u32 {
    let start = Instant::now();
    let mut count = 0u32;
    while start.elapsed() < window {
        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        match godly_protocol::read_daemon_message(pipe) {
            Ok(Some(DaemonMessage::Event(Event::Output { session_id: sid, .. })))
                if sid == session_id =>
            {
                count += 1;
            }
            Ok(Some(DaemonMessage::Event(Event::GridDiff { session_id: sid, .. })))
                if sid == session_id =>
            {
                count += 1;
            }
            Ok(Some(DaemonMessage::Event(Event::Bell { session_id: sid })))
                if sid == session_id =>
            {
                count += 1;
            }
            Ok(Some(_)) => {} // other events/responses
            _ => break,
        }
    }
    count
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
        let target_dir = manifest_dir.parent().unwrap().join("target").join("debug");
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

/// Verify that PauseSession suppresses Output/GridDiff events and
/// ResumeSession restores them. The VT parser stays current in both states.
#[test]
#[ntest::timeout(60_000)]
fn test_pause_suppresses_output_resume_restores() {
    let fixture = DaemonFixture::spawn("pause_resume");
    let mut pipe = fixture.connect();

    // Create a session that produces continuous output
    let session_id = "pause-test-session";
    let resp = create_session_with_retry(&mut pipe, session_id, 3);
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach to receive events
    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.to_string(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Send a command that produces output
    let _ = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"echo BEFORE_PAUSE\r\n".to_vec(),
        },
    );

    // Wait for some events
    std::thread::sleep(Duration::from_millis(500));
    let _events_before = drain_events(&mut pipe, Duration::from_millis(500));

    // Pause the session
    let resp = send_request(
        &mut pipe,
        &Request::PauseSession {
            session_id: session_id.to_string(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Drain any in-flight events from before the pause took effect
    std::thread::sleep(Duration::from_millis(200));
    drain_events(&mut pipe, Duration::from_millis(200));

    // Write while paused — should produce NO output events
    let _ = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"echo DURING_PAUSE\r\n".to_vec(),
        },
    );

    // Count events during the paused window
    let events_while_paused = count_session_events(
        &mut pipe,
        session_id,
        Duration::from_secs(2),
    );

    // Should have zero output events while paused
    assert_eq!(
        events_while_paused, 0,
        "Expected 0 output events while paused, got {}",
        events_while_paused
    );

    // The VT parser should still have the content — verify via ReadRichGrid
    let resp = send_request(
        &mut pipe,
        &Request::ReadRichGrid {
            session_id: session_id.to_string(),
        },
    );
    match &resp {
        Response::RichGrid { grid } => {
            // Grid should have content (the parser processed the "echo DURING_PAUSE" output)
            let has_content = grid.rows.iter().any(|row| {
                row.cells.iter().any(|cell| !cell.content.trim().is_empty())
            });
            assert!(has_content, "VT parser should have content even while paused");
        }
        other => panic!("Expected RichGrid, got {:?}", other),
    }

    // Resume the session
    let resp = send_request(
        &mut pipe,
        &Request::ResumeSession {
            session_id: session_id.to_string(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Write after resume — should produce output events again
    let _ = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"echo AFTER_RESUME\r\n".to_vec(),
        },
    );

    let events_after_resume = count_session_events(
        &mut pipe,
        session_id,
        Duration::from_secs(2),
    );

    assert!(
        events_after_resume > 0,
        "Expected output events after resume, got 0"
    );

    // ListSessions should report paused=false
    let resp = send_request(&mut pipe, &Request::ListSessions);
    match resp {
        Response::SessionList { sessions } => {
            let info = sessions.iter().find(|s| s.id == session_id).unwrap();
            assert!(!info.paused, "Session should not be paused after ResumeSession");
            assert!(info.scrollback_rows > 0 || info.scrollback_memory_bytes == 0,
                "Session info should include scrollback stats");
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }

    // Clean up
    let _ = send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
    );
}

/// Verify ListSessions reports paused=true for a paused session.
#[test]
#[ntest::timeout(60_000)]
fn test_list_sessions_reports_paused_state() {
    let fixture = DaemonFixture::spawn("pause_list");
    let mut pipe = fixture.connect();

    let session_id = "pause-list-session";
    let resp = create_session_with_retry(&mut pipe, session_id, 3);
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach so pause has effect
    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.to_string(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Before pause: paused=false
    let resp = send_request(&mut pipe, &Request::ListSessions);
    match &resp {
        Response::SessionList { sessions } => {
            let info = sessions.iter().find(|s| s.id == session_id).unwrap();
            assert!(!info.paused);
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }

    // Pause
    let resp = send_request(
        &mut pipe,
        &Request::PauseSession {
            session_id: session_id.to_string(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // After pause: paused=true
    let resp = send_request(&mut pipe, &Request::ListSessions);
    match &resp {
        Response::SessionList { sessions } => {
            let info = sessions.iter().find(|s| s.id == session_id).unwrap();
            assert!(info.paused, "Session should be reported as paused");
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }

    // Clean up
    let _ = send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
    );
}
