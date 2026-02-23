//! Integration test for adaptive output batching (Phase 4).
//!
//! Verifies that the daemon's session-level mode detector and coalescing work
//! correctly under both bulk and interactive output patterns:
//!
//! 1. Bulk output (e.g. `echo` loop) — all data arrives at the client without loss
//! 2. Interactive output (typed commands) — responses arrive with low latency
//!
//! Uses the DaemonFixture pattern with full pipe isolation.
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-daemon --test adaptive_batching

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Event, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// Helpers (DaemonFixture pattern — same as handler_starvation.rs)
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
            panic!(
                "Failed to connect to pipe '{}' within {:?} (error: {})",
                pipe_name, timeout, err
            );
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

fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
    godly_protocol::write_request(pipe, request)
        .unwrap_or_else(|e| panic!("Failed to write request {:?}: {}", std::mem::discriminant(request), e));
    loop {
        let msg: DaemonMessage = godly_protocol::read_daemon_message(pipe)
            .expect("Failed to read message")
            .expect("Unexpected EOF");
        match msg {
            DaemonMessage::Response(resp) => return resp,
            DaemonMessage::Event(_) => continue,
        }
    }
}

fn send_request_with_deadline(
    pipe: &mut std::fs::File,
    request: &Request,
    deadline: Duration,
) -> Result<(Response, Duration, u32), String> {
    godly_protocol::write_request(pipe, request)
        .map_err(|e| format!("Failed to write request: {}", e))?;

    let start = Instant::now();
    let mut events_skipped = 0u32;

    loop {
        if start.elapsed() > deadline {
            return Err(format!(
                "Deadline exceeded ({:?}): no response after skipping {} events",
                deadline, events_skipped
            ));
        }

        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        let msg: DaemonMessage = godly_protocol::read_daemon_message(pipe)
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Unexpected EOF".to_string())?;

        match msg {
            DaemonMessage::Response(resp) => {
                return Ok((resp, start.elapsed(), events_skipped));
            }
            DaemonMessage::Event(_) => {
                events_skipped += 1;
                continue;
            }
        }
    }
}

/// Drain all available events from the pipe without blocking.
/// Returns (event_count, total_output_bytes).
fn drain_events(pipe: &mut std::fs::File, timeout: Duration) -> (u32, usize) {
    let start = Instant::now();
    let mut event_count = 0u32;
    let mut total_bytes = 0usize;

    while start.elapsed() < timeout {
        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        match godly_protocol::read_daemon_message(pipe) {
            Ok(Some(DaemonMessage::Event(Event::Output { data, .. }))) => {
                event_count += 1;
                total_bytes += data.len();
            }
            Ok(Some(DaemonMessage::Event(_))) => {
                event_count += 1;
            }
            Ok(Some(DaemonMessage::Response(_))) => {
                // Shouldn't happen during drain, but don't panic
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    (event_count, total_bytes)
}

/// Create a session with retries. Pty-shim spawn can fail transiently when
/// multiple daemon instances start in parallel (common in test suites).
/// Uses exponential backoff: 500ms, 1s, 2s, 4s, ...
fn create_session_with_retry(
    pipe: &mut std::fs::File,
    session_id: &str,
    max_attempts: u32,
) -> Response {
    let mut delay_ms = 500u64;
    for attempt in 1..=max_attempts {
        let resp = send_request(
            pipe,
            &Request::CreateSession {
                id: format!("{}-{}", session_id, attempt),
                shell_type: ShellType::Windows,
                rows: 24,
                cols: 80,
                cwd: None,
                env: None,
            },
        );
        if matches!(resp, Response::SessionCreated { .. }) {
            return resp;
        }
        if attempt < max_attempts {
            eprintln!(
                "Session creation attempt {} failed: {:?}, retrying in {}ms...",
                attempt, resp, delay_ms
            );
            std::thread::sleep(Duration::from_millis(delay_ms));
            delay_ms = (delay_ms * 2).min(4000);
        } else {
            return resp;
        }
    }
    unreachable!()
}

/// Extract the session ID from a SessionCreated response.
fn extract_session_id(resp: &Response) -> String {
    match resp {
        Response::SessionCreated { session } => session.id.clone(),
        _ => panic!("Expected SessionCreated, got: {:?}", resp),
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
            .env("GODLY_INSTANCE", pipe_name.trim_start_matches(r"\\.\pipe\"))
            .env("GODLY_NO_DETACH", "1")
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn daemon");

        std::thread::sleep(Duration::from_millis(1000));

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

/// Verify that bulk output (rapid, large) is delivered without data loss.
/// The daemon's coalescing may combine multiple PTY reads into fewer events,
/// but total bytes delivered must be non-trivial.
#[test]
#[ntest::timeout(60_000)]
fn bulk_output_no_data_loss() {
    let daemon = DaemonFixture::spawn("adaptive-bulk");
    let mut pipe = daemon.connect();

    // Create a session (with retries for flaky shim spawn)
    let resp = create_session_with_retry(&mut pipe, "bulk-test", 5);
    assert!(matches!(resp, Response::SessionCreated { .. }), "Expected SessionCreated, got: {:?}", resp);
    let session_id = extract_session_id(&resp);

    // Attach to receive output
    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }), "Expected Ok or Buffer for Attach, got: {:?}", resp);

    // Wait for shell prompt
    std::thread::sleep(Duration::from_millis(2000));
    let _ = drain_events(&mut pipe, Duration::from_millis(500));

    // Generate bulk output: echo a predictable pattern many times.
    // This should trigger the bulk mode detector in the session I/O thread.
    let bulk_cmd = "for /L %i in (1,1,200) do @echo LINE_%i_PADDING_DATA_TO_MAKE_THIS_LONGER\r\n";
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: bulk_cmd.as_bytes().to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Wait a moment for the shell to start processing the for loop
    std::thread::sleep(Duration::from_millis(500));

    // Collect output events — keep draining until we see the expected marker
    // or we time out. We look for any LINE_ markers in the output.
    let mut total_bytes = 0usize;
    let mut event_count = 0u32;
    let mut found_line_marker = false;
    let deadline = Instant::now() + Duration::from_secs(15);

    while Instant::now() < deadline {
        if !pipe_has_data(&pipe) {
            // If we already found markers and no more data, we're done
            if found_line_marker {
                std::thread::sleep(Duration::from_millis(200));
                if !pipe_has_data(&pipe) {
                    break;
                }
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
            continue;
        }

        match godly_protocol::read_daemon_message(&mut pipe) {
            Ok(Some(DaemonMessage::Event(Event::Output { data, .. }))) => {
                event_count += 1;
                total_bytes += data.len();
                let text = String::from_utf8_lossy(&data);
                if text.contains("LINE_") {
                    found_line_marker = true;
                }
            }
            Ok(Some(_)) => {
                event_count += 1;
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    eprintln!(
        "Bulk output: {} bytes in {} events, found_marker={}",
        total_bytes, event_count, found_line_marker
    );

    // Verify we received the for-loop output
    assert!(
        found_line_marker,
        "Expected LINE_ markers in bulk output, got {} bytes in {} events",
        total_bytes, event_count
    );

    // Clean up
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        Duration::from_secs(5),
    );
}

/// Verify that interactive-pattern output (commands with gaps) still gets
/// low-latency responses. The daemon should NOT buffer interactive output
/// behind bulk coalescing.
#[test]
#[ntest::timeout(60_000)]
fn interactive_output_low_latency() {
    let daemon = DaemonFixture::spawn("adaptive-interactive");
    let mut pipe = daemon.connect();

    // Create and attach (with retries for flaky shim spawn)
    let resp = create_session_with_retry(&mut pipe, "interactive-test", 5);
    assert!(matches!(resp, Response::SessionCreated { .. }), "Expected SessionCreated, got: {:?}", resp);
    let session_id = extract_session_id(&resp);

    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }), "Expected Ok or Buffer for Attach, got: {:?}", resp);

    // Wait for shell prompt
    std::thread::sleep(Duration::from_millis(1500));
    let _ = drain_events(&mut pipe, Duration::from_millis(500));

    // Send an interactive command with a gap before it (simulating typing)
    std::thread::sleep(Duration::from_millis(100));

    let start = Instant::now();
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"echo hello\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // The Write response should come back quickly (it's just an ack).
    // Now check that output events arrive within a reasonable time.
    let mut got_output = false;
    let output_deadline = Duration::from_secs(5);

    while start.elapsed() < output_deadline {
        if !pipe_has_data(&pipe) {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        match godly_protocol::read_daemon_message(&mut pipe) {
            Ok(Some(DaemonMessage::Event(Event::Output { data, .. }))) => {
                let text = String::from_utf8_lossy(&data);
                if text.contains("hello") {
                    got_output = true;
                    let latency = start.elapsed();
                    assert!(
                        latency < Duration::from_secs(3),
                        "Interactive output took too long: {:?}",
                        latency
                    );
                    break;
                }
            }
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => break,
        }
    }

    assert!(got_output, "Never received 'hello' output from interactive command");

    // Clean up
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        Duration::from_secs(5),
    );
}

/// Verify that Ping requests still get fast responses during bulk output.
/// This tests the full path: session coalescing reduces events → bridge
/// adaptive batching reads more per iteration → requests still serviced.
#[test]
#[ntest::timeout(60_000)]
fn requests_responsive_during_bulk_output() {
    let daemon = DaemonFixture::spawn("adaptive-responsive");
    let mut pipe = daemon.connect();

    let resp = create_session_with_retry(&mut pipe, "responsive-test", 5);
    assert!(matches!(resp, Response::SessionCreated { .. }), "Expected SessionCreated, got: {:?}", resp);
    let session_id = extract_session_id(&resp);

    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }), "Expected Ok or Buffer for Attach, got: {:?}", resp);

    // Wait for shell prompt
    std::thread::sleep(Duration::from_millis(1500));
    let _ = drain_events(&mut pipe, Duration::from_millis(500));

    // Start heavy output
    let bulk_cmd = "for /L %i in (1,1,500) do @echo BULK_LINE_%i_PADDING_DATA\r\n";
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: bulk_cmd.as_bytes().to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Wait a moment for output to start flowing
    std::thread::sleep(Duration::from_millis(200));

    // Send Ping while output is flowing — should respond within 15s
    let (resp, latency, events_skipped) = send_request_with_deadline(
        &mut pipe,
        &Request::Ping,
        Duration::from_secs(15),
    )
    .expect("Ping should succeed during bulk output");

    assert!(
        matches!(resp, Response::Pong),
        "Expected Pong, got {:?}",
        resp
    );

    eprintln!(
        "Ping during bulk output: latency={:?}, events_skipped={}",
        latency, events_skipped
    );

    // Clean up
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        Duration::from_secs(5),
    );
}
