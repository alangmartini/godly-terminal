//! Cold switch latency test: measures ReadRichGrid latency when switching to an
//! idle terminal while other sessions produce heavy output through the same
//! bridge pipe.
//!
//! Bug #373: When switching to a terminal that hasn't been focused for a while,
//! the screen is black for 2-3 seconds. Part of the delay is bridge I/O thread
//! contention: events from active sessions flood the pipe, and the ReadRichGrid
//! request for the idle session queues behind event reads.
//!
//! This test reproduces the real-world scenario:
//!   - 3 sessions attached on one bridge pipe
//!   - 2 sessions produce heavy output (simulating active background terminals)
//!   - 1 session is idle (the terminal being switched to)
//!   - ReadRichGrid latency is measured for the idle session through the bridge
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-daemon --test cold_switch_latency

#![cfg(windows)]

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// DaemonFixture (isolated pipe name + PID-based kill)
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
                "Failed to connect to pipe '{}' within {:?} (err {})",
                pipe_name, timeout, err
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

enum PeekResult {
    Data,
    Empty,
    Error,
}

fn peek_pipe(handle: *mut winapi::ctypes::c_void) -> PeekResult {
    use winapi::um::namedpipeapi::PeekNamedPipe;
    let mut bytes_available: u32 = 0;
    let result = unsafe {
        PeekNamedPipe(
            handle,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut bytes_available,
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        PeekResult::Error
    } else if bytes_available > 0 {
        PeekResult::Data
    } else {
        PeekResult::Empty
    }
}

fn send_request_blocking(pipe: &mut std::fs::File, request: &Request) -> Response {
    godly_protocol::write_request(pipe, request).expect("write");
    loop {
        let msg: DaemonMessage =
            godly_protocol::read_daemon_message(pipe).expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(r) => return r,
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
        let daemon_exe = manifest_dir
            .parent()
            .unwrap()
            .join("target/debug/godly-daemon.exe");
        assert!(
            daemon_exe.exists(),
            "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
            daemon_exe
        );

        let child = Command::new(&daemon_exe)
            .env("GODLY_PIPE_NAME", &pipe_name)
            .env(
                "GODLY_INSTANCE",
                pipe_name.trim_start_matches(r"\\.\pipe\"),
            )
            .env("GODLY_NO_DETACH", "1")
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn");

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
// Simulated Bridge (same pattern as input_latency_full_path.rs)
// ---------------------------------------------------------------------------

struct BridgeRequest {
    request: Request,
    response_tx: mpsc::Sender<Response>,
}

/// Simulated bridge I/O thread: single thread handles ALL pipe I/O.
/// Events from ALL attached sessions interleave on the pipe.
fn bridge_io_loop(
    mut pipe: std::fs::File,
    request_rx: mpsc::Receiver<BridgeRequest>,
    stop: Arc<AtomicBool>,
    event_counter: Arc<AtomicU64>,
) {
    const MAX_EVENTS_BEFORE_CHECK: usize = 8;

    let raw_handle = pipe.as_raw_handle() as *mut winapi::ctypes::c_void;
    let mut pending_responses: VecDeque<mpsc::Sender<Response>> = VecDeque::new();

    while !stop.load(Ordering::Relaxed) {
        // Step 1: Drain events from pipe (up to MAX_EVENTS)
        let mut events_this_round = 0;
        loop {
            if events_this_round >= MAX_EVENTS_BEFORE_CHECK {
                break;
            }

            match peek_pipe(raw_handle) {
                PeekResult::Data => {
                    match godly_protocol::read_daemon_message(&mut pipe) {
                        Ok(Some(DaemonMessage::Event(_))) => {
                            event_counter.fetch_add(1, Ordering::Relaxed);
                            events_this_round += 1;
                        }
                        Ok(Some(DaemonMessage::Response(resp))) => {
                            if let Some(tx) = pending_responses.pop_front() {
                                let _ = tx.send(resp);
                            }
                            break;
                        }
                        Ok(None) => {
                            stop.store(true, Ordering::Relaxed);
                            break;
                        }
                        Err(_) => {
                            stop.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }
                PeekResult::Empty => break,
                PeekResult::Error => {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }

        // Step 2: Check for pending requests
        match request_rx.try_recv() {
            Ok(req) => {
                godly_protocol::write_request(&mut pipe, &req.request).expect("bridge write");
                pending_responses.push_back(req.response_tx);
                continue;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }

        // Step 3: Sleep if idle
        if events_this_round == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// Send a request through the bridge channel and measure round-trip time.
fn bridge_request(
    tx: &mpsc::Sender<BridgeRequest>,
    request: Request,
    timeout: Duration,
) -> Result<(Response, Duration), String> {
    let (resp_tx, resp_rx) = mpsc::channel();
    let start = Instant::now();

    tx.send(BridgeRequest {
        request,
        response_tx: resp_tx,
    })
    .map_err(|e| format!("Channel send failed: {}", e))?;

    let resp = resp_rx
        .recv_timeout(timeout)
        .map_err(|e| format!("Timeout after {:?}: {}", timeout, e))?;

    Ok((resp, start.elapsed()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug #373: Cold switch snapshot latency under multi-session contention.
///
/// Scenario:
///   - 3 sessions on one bridge pipe
///   - 2 sessions produce heavy output (active background terminals)
///   - 1 session is idle (the terminal being switched to)
///   - Measure ReadRichGrid latency for the idle session
///
/// This reproduces the real-world "black screen on tab switch" issue:
/// the bridge I/O thread is busy reading events from active sessions,
/// so the ReadRichGrid request for the idle session queues behind them.
///
/// Target: p95 < 200ms for the cold switch snapshot.
/// Current reality: likely 500ms+ under sustained multi-session output.
#[test]
#[ntest::timeout(120_000)]
fn cold_switch_snapshot_latency_under_contention() {
    let daemon = DaemonFixture::spawn("cold-switch");

    // Setup pipe for session management
    let mut setup_pipe = daemon.connect();

    // Verify daemon is alive
    let resp = send_request_blocking(&mut setup_pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    // Create 3 sessions: 2 active + 1 idle
    let active_ids = ["cold-switch-active-1", "cold-switch-active-2"];
    let idle_id = "cold-switch-idle";

    for &sid in &active_ids {
        let resp = send_request_blocking(
            &mut setup_pipe,
            &Request::CreateSession {
                id: sid.to_string(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 30,
                cols: 120,
                env: None,
            },
        );
        assert!(
            matches!(resp, Response::SessionCreated { .. }),
            "Failed to create session {}: {:?}",
            sid,
            resp
        );
    }

    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::CreateSession {
            id: idle_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "Failed to create idle session: {:?}",
        resp
    );

    // Write some content to the idle session so it has a non-empty grid
    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::Write {
            session_id: idle_id.to_string(),
            data: b"echo Terminal content is here\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));
    std::thread::sleep(Duration::from_secs(1));

    // Connect the bridge pipe and attach to ALL sessions
    let mut bridge_pipe = daemon.connect();

    for &sid in &active_ids {
        godly_protocol::write_request(
            &mut bridge_pipe,
            &Request::Attach {
                session_id: sid.to_string(),
            },
        )
        .expect("attach write");
        loop {
            let msg: DaemonMessage = godly_protocol::read_daemon_message(&mut bridge_pipe)
                .expect("read")
                .expect("EOF");
            match msg {
                DaemonMessage::Response(_) => break,
                DaemonMessage::Event(_) => continue,
            }
        }
    }

    godly_protocol::write_request(
        &mut bridge_pipe,
        &Request::Attach {
            session_id: idle_id.to_string(),
        },
    )
    .expect("attach write");
    loop {
        let msg: DaemonMessage = godly_protocol::read_daemon_message(&mut bridge_pipe)
            .expect("read")
            .expect("EOF");
        match msg {
            DaemonMessage::Response(_) => break,
            DaemonMessage::Event(_) => continue,
        }
    }

    // Wait for shells to settle
    std::thread::sleep(Duration::from_secs(2));

    // Start the bridge I/O thread
    let (req_tx, req_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let event_counter = Arc::new(AtomicU64::new(0));
    let event_counter_clone = event_counter.clone();

    let bridge_thread = std::thread::Builder::new()
        .name("test-bridge".into())
        .spawn(move || bridge_io_loop(bridge_pipe, req_rx, stop_clone, event_counter_clone))
        .expect("spawn bridge");

    // Drain initial events
    std::thread::sleep(Duration::from_millis(500));

    // ── Baseline: ReadRichGrid latency with NO contention ──
    let mut baseline_latencies = Vec::with_capacity(5);
    for i in 0..5 {
        let (resp, lat) = bridge_request(
            &req_tx,
            Request::ReadRichGrid {
                session_id: idle_id.to_string(),
            },
            Duration::from_secs(10),
        )
        .unwrap_or_else(|e| panic!("Baseline ReadRichGrid #{} failed: {}", i, e));
        assert!(matches!(resp, Response::RichGrid { .. }));
        baseline_latencies.push(lat);
    }

    let baseline_avg_ms = baseline_latencies
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / baseline_latencies.len() as f64;
    eprintln!(
        "[cold-switch] Baseline (no contention): avg={:.1}ms",
        baseline_avg_ms
    );

    // ── Start heavy output on active sessions ──
    // ShellType::Windows launches PowerShell, so use PS-compatible syntax.
    for &sid in &active_ids {
        let (resp, _) = bridge_request(
            &req_tx,
            Request::Write {
                session_id: sid.to_string(),
                data: b"1..200000 | ForEach-Object { \"Line ${_}: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\" }\r\n"
                    .to_vec(),
            },
            Duration::from_secs(10),
        )
        .expect("start heavy output");
        assert!(matches!(resp, Response::Ok));
    }

    // Wait for output flood to start. Poll until we see sustained event flow.
    let flood_start = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let events_now = event_counter.load(Ordering::Relaxed);
        if events_now > 50 {
            eprintln!(
                "[cold-switch] Output flood established: {} events in {:.1}s",
                events_now,
                flood_start.elapsed().as_secs_f64()
            );
            break;
        }
        if flood_start.elapsed() > Duration::from_secs(15) {
            eprintln!(
                "[cold-switch] WARNING: Only {} events after 15s, proceeding anyway",
                events_now
            );
            break;
        }
    }

    // ── Cold switch: ReadRichGrid for idle session DURING heavy output ──
    let mut contended_latencies = Vec::with_capacity(10);
    for i in 0..10 {
        let events_at_start = event_counter.load(Ordering::Relaxed);
        let (resp, lat) = bridge_request(
            &req_tx,
            Request::ReadRichGrid {
                session_id: idle_id.to_string(),
            },
            Duration::from_secs(15),
        )
        .unwrap_or_else(|e| panic!("Contended ReadRichGrid #{} failed: {}", i, e));
        assert!(matches!(resp, Response::RichGrid { .. }));

        let events_during = event_counter.load(Ordering::Relaxed) - events_at_start;
        contended_latencies.push(lat);

        eprintln!(
            "[cold-switch] Contended #{}: latency={:.1}ms, events_during={}",
            i,
            lat.as_secs_f64() * 1000.0,
            events_during,
        );

        // Small gap between measurements to avoid measuring back-to-back
        std::thread::sleep(Duration::from_millis(100));
    }

    let contended_avg_ms = contended_latencies
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / contended_latencies.len() as f64;
    let mut sorted: Vec<f64> = contended_latencies
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = sorted[(sorted.len() as f64 * 0.95) as usize];
    let max_ms = sorted.last().copied().unwrap_or(0.0);

    eprintln!(
        "\n[cold-switch] CONTENDED RESULT: avg={:.1}ms  p95={:.1}ms  max={:.1}ms",
        contended_avg_ms, p95_ms, max_ms
    );
    eprintln!(
        "[cold-switch] BASELINE: avg={:.1}ms",
        baseline_avg_ms
    );
    eprintln!(
        "[cold-switch] DEGRADATION: {:.1}x",
        contended_avg_ms / baseline_avg_ms.max(0.1)
    );

    // ── Assertion: cold switch snapshot must be fast enough ──
    // Bug #373: Target is <200ms for the snapshot fetch. If this exceeds 200ms,
    // the user sees a black screen for a noticeable duration. With 2 active
    // sessions flooding the bridge, the contended latency currently exceeds this.
    assert!(
        p95_ms < 200.0,
        "Bug #373: Cold switch snapshot p95 latency is {:.1}ms (target <200ms). \
         The bridge I/O thread is contended by events from {} active sessions, \
         causing ReadRichGrid for the idle session to queue. \
         Baseline (no contention): {:.1}ms. Degradation: {:.1}x.",
        p95_ms,
        active_ids.len(),
        baseline_avg_ms,
        contended_avg_ms / baseline_avg_ms.max(0.1),
    );

    // Cleanup
    stop.store(true, Ordering::Relaxed);
    drop(req_tx);
    let _ = bridge_thread.join();

    for &sid in &active_ids {
        let _ = send_request_blocking(
            &mut setup_pipe,
            &Request::CloseSession {
                session_id: sid.to_string(),
            },
        );
    }
    let _ = send_request_blocking(
        &mut setup_pipe,
        &Request::CloseSession {
            session_id: idle_id.to_string(),
        },
    );
}

/// Verify that the idle session's grid content is correct after heavy output
/// on other sessions — the idle session's VT parser should be unaffected.
#[test]
#[ntest::timeout(60_000)]
fn cold_switch_idle_session_grid_integrity() {
    let daemon = DaemonFixture::spawn("cold-grid-integrity");

    let mut setup_pipe = daemon.connect();

    let resp = send_request_blocking(&mut setup_pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    // Create an active session and an idle session
    let active_id = "integrity-active";
    let idle_id = "integrity-idle";

    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::CreateSession {
            id: active_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::CreateSession {
            id: idle_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Write known content to idle session, then let it settle
    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::Write {
            session_id: idle_id.to_string(),
            data: b"echo IDLE_MARKER_CONTENT\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));
    std::thread::sleep(Duration::from_secs(2));

    // Read baseline grid for idle session
    let baseline_resp = send_request_blocking(
        &mut setup_pipe,
        &Request::ReadRichGrid {
            session_id: idle_id.to_string(),
        },
    );
    let baseline_text = match &baseline_resp {
        Response::RichGrid { grid } => {
            let text: String = grid
                .rows
                .iter()
                .map(|row| {
                    row.cells
                        .iter()
                        .map(|c| c.content.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            text
        }
        other => panic!("Expected RichGrid, got {:?}", other),
    };
    assert!(
        baseline_text.contains("IDLE_MARKER_CONTENT"),
        "Idle session should contain marker text"
    );

    // Now flood the active session with heavy output (PowerShell syntax)
    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::Write {
            session_id: active_id.to_string(),
            data: b"1..50000 | ForEach-Object { \"Active output line ${_}\" }\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Wait for heavy output to run
    std::thread::sleep(Duration::from_secs(3));

    // Read idle session grid AFTER heavy output on other session
    let after_resp = send_request_blocking(
        &mut setup_pipe,
        &Request::ReadRichGrid {
            session_id: idle_id.to_string(),
        },
    );
    let after_text = match &after_resp {
        Response::RichGrid { grid } => {
            let text: String = grid
                .rows
                .iter()
                .map(|row| {
                    row.cells
                        .iter()
                        .map(|c| c.content.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            text
        }
        other => panic!("Expected RichGrid, got {:?}", other),
    };

    // Idle session's grid should still contain its original content
    assert!(
        after_text.contains("IDLE_MARKER_CONTENT"),
        "Bug #373: Idle session grid was corrupted by heavy output on another session. \
         Expected to find 'IDLE_MARKER_CONTENT' in the grid."
    );

    // Cleanup
    for sid in [active_id, idle_id] {
        let _ = send_request_blocking(
            &mut setup_pipe,
            &Request::CloseSession {
                session_id: sid.to_string(),
            },
        );
    }
}
