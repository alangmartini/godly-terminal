//! Handler starvation test: verify the daemon responds to requests during heavy output.
//!
//! Bug: Under heavy output (e.g. Claude CLI streaming a long response), the daemon's
//! handler thread blocked on `output_tx.lock()` in `is_attached()`/`info()` because
//! the reader thread held that lock in a tight loop. Since the handler is sequential,
//! ALL terminals froze — no Write, Resize, Ping, or Attach could be processed.
//! The bridge detected the stall and reconnected, but the new handler also blocked
//! on `session.attach()` → `output_tx.lock()`, causing unbounded client accumulation.
//!
//! This test creates a session with heavy continuous output, then sends requests
//! while output is flowing and verifies they complete within a deadline. Without the
//! fix, the handler blocks indefinitely and requests never get responses.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test handler_starvation -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::io::AsRawHandle;
use std::os::windows::ffi::OsStrExt;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// Helpers (same DaemonFixture pattern as memory_stress.rs)
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

/// Send a request and wait for the response with a hard deadline.
/// Uses PeekNamedPipe to poll for data, so it can time out instead of hanging.
///
/// Returns Err(description) if the deadline expires without receiving a response.
/// On success, returns (response, wall_time, events_skipped).
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
                "Deadline exceeded ({:?}): no response received after skipping {} events",
                deadline, events_skipped
            ));
        }

        // Non-blocking check: is there data to read?
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

/// Send a request and read the response (blocking, no deadline). For setup only.
fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
    godly_protocol::write_request(pipe, request).expect("Failed to write request");
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

/// Maximum time any single request should take to get a response.
/// With the old bug, requests hang indefinitely (handler blocked on output_tx.lock).
/// With the fix, responses should arrive within seconds even under heavy output.
/// 15s is generous — accounts for pipe buffer draining + event reading overhead.
const REQUEST_DEADLINE: Duration = Duration::from_secs(15);

/// Bug: handler blocked on output_tx.lock() in info() during ListSessions,
/// causing all terminals to freeze under heavy output. Fix: AtomicBool for
/// is_attached(), try_lock_for in attach(), yield_now in reader thread.
///
/// This test creates a session producing continuous heavy output, then sends
/// Ping, ListSessions, and Write requests while output is flowing. Each request
/// must receive a response within REQUEST_DEADLINE. Without the fix, the handler
/// blocks forever on output_tx.lock() and no response is ever produced.
#[test]
#[ntest::timeout(120_000)] // 2min — spawns daemon + heavy PTY output + IPC round-trips
fn test_requests_complete_during_heavy_output() {
    let daemon = DaemonFixture::spawn("handler-starvation");
    let mut pipe = daemon.connect();

    // Verify connection
    let resp = send_request(&mut pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong), "Initial ping failed");

    // Create a session that will produce heavy output
    let heavy_id = "heavy-output".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: heavy_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "Create heavy session failed: {:?}",
        resp
    );

    // Attach to the heavy session (starts output forwarding)
    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: heavy_id.clone(),
        },
    );
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Attach failed: {:?}",
        resp
    );

    // Wait for shell to start
    std::thread::sleep(Duration::from_secs(1));

    // Start heavy continuous output — PowerShell generating 4KB lines
    // Bug trigger: reader thread holds output_tx in a tight read→lock→send→unlock loop
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: heavy_id.clone(),
            data: b"1..100000 | ForEach-Object { Write-Output ('A' * 4096) }\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok), "Write cmd failed: {:?}", resp);

    // Wait for heavy output to start flowing through the daemon
    std::thread::sleep(Duration::from_secs(3));

    // --- Test 1: Ping during heavy output ---
    let result = send_request_with_deadline(&mut pipe, &Request::Ping, REQUEST_DEADLINE);
    let (resp, latency, events) = result.unwrap_or_else(|e| {
        panic!(
            "HANDLER STARVATION: Ping got no response within {:?} — {}",
            REQUEST_DEADLINE, e
        )
    });
    assert!(matches!(resp, Response::Pong), "Ping response wrong: {:?}", resp);
    eprintln!("[test] Ping: {:?} (skipped {} events)", latency, events);

    // --- Test 2: ListSessions during heavy output (primary starvation vector) ---
    // This was the exact code path that caused the freeze: ListSessions → info() →
    // is_attached() → output_tx.lock() contended with reader thread
    let result =
        send_request_with_deadline(&mut pipe, &Request::ListSessions, REQUEST_DEADLINE);
    let (resp, latency, events) = result.unwrap_or_else(|e| {
        panic!(
            "HANDLER STARVATION: ListSessions got no response within {:?} — {}",
            REQUEST_DEADLINE, e
        )
    });
    assert!(
        matches!(resp, Response::SessionList { .. }),
        "ListSessions response wrong: {:?}",
        resp
    );
    eprintln!(
        "[test] ListSessions: {:?} (skipped {} events)",
        latency, events
    );

    // --- Test 3: Detach during heavy output ---
    // detach() still uses plain .lock() on output_tx — contends with reader thread
    let result = send_request_with_deadline(
        &mut pipe,
        &Request::Detach {
            session_id: heavy_id.clone(),
        },
        REQUEST_DEADLINE,
    );
    let (resp, latency, events) = result.unwrap_or_else(|e| {
        panic!(
            "HANDLER STARVATION: Detach got no response within {:?} — {}",
            REQUEST_DEADLINE, e
        )
    });
    assert!(matches!(resp, Response::Ok), "Detach failed: {:?}", resp);
    eprintln!("[test] Detach: {:?} (skipped {} events)", latency, events);

    // --- Test 4: Re-attach during heavy output ---
    // attach() uses try_lock_for(2s) — should not block indefinitely
    let result = send_request_with_deadline(
        &mut pipe,
        &Request::Attach {
            session_id: heavy_id.clone(),
        },
        REQUEST_DEADLINE,
    );
    let (resp, latency, events) = result.unwrap_or_else(|e| {
        panic!(
            "HANDLER STARVATION: Re-attach got no response within {:?} — {}",
            REQUEST_DEADLINE, e
        )
    });
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Re-attach failed: {:?}",
        resp
    );
    eprintln!(
        "[test] Re-attach: {:?} (skipped {} events)",
        latency, events
    );

    // --- Test 5: Second client during heavy output ---
    // Simulates bridge reconnection: a NEW client connects while first handles
    // heavy output. Without the fix, the new handler also blocks on session locks.
    let mut pipe2 = daemon.connect();
    let result =
        send_request_with_deadline(&mut pipe2, &Request::ListSessions, Duration::from_secs(5));
    let (resp, latency, _) = result.unwrap_or_else(|e| {
        panic!(
            "HANDLER STARVATION: Second client ListSessions failed within 5s — {}",
            e
        )
    });
    assert!(
        matches!(resp, Response::SessionList { .. }),
        "Second client response wrong: {:?}",
        resp
    );
    eprintln!("[test] Second client ListSessions: {:?}", latency);

    // Cleanup
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession {
            session_id: heavy_id,
        },
        Duration::from_secs(5),
    );
}
