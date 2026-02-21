//! Arrow-up history navigation latency regression test.
//!
//! Bug #149 (regression): Pressing Up arrow to navigate command history feels laggy.
//! This test reproduces the exact user scenario: type several commands to build
//! shell history, then press Up arrow repeatedly and measure the round-trip
//! latency for each keystroke through the full bridge-simulated pipeline.
//!
//! Arrow-up is more expensive than regular character echo because:
//!   1. Shell history lookup involves file I/O (PSReadLine history file)
//!   2. Shell redraws the entire prompt line with the previous command
//!   3. VT output includes cursor movement + line clear + text replacement
//!   4. More bytes flow through the pipe per keystroke
//!   5. More rows are dirty in the grid snapshot
//!
//! The test simulates the real bridge architecture:
//!   [Command thread] --channel--> [Bridge I/O thread] --pipe--> [Daemon]
//!
//! The bridge I/O thread is single-threaded, exactly like the real app.
//! Under any concurrent output, requests queue behind event reads.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test arrow_up_history_latency -- --test-threads=1

#![cfg(windows)]

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// DaemonFixture (same isolation pattern as other daemon tests)
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

/// Drain all pending events from the pipe without blocking.
fn drain_events(pipe: &mut std::fs::File) -> u32 {
    let handle = pipe.as_raw_handle();
    let mut drained = 0u32;
    loop {
        let mut bytes_available: u32 = 0;
        let result = unsafe {
            use winapi::um::namedpipeapi::PeekNamedPipe;
            PeekNamedPipe(
                handle as *mut _,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut bytes_available,
                std::ptr::null_mut(),
            )
        };
        if result == 0 || bytes_available == 0 {
            break;
        }
        let msg: DaemonMessage =
            godly_protocol::read_daemon_message(pipe).expect("drain read").expect("drain EOF");
        match msg {
            DaemonMessage::Event(_) => drained += 1,
            DaemonMessage::Response(_) => break,
        }
    }
    drained
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

        let status = Command::new("cargo")
            .args(["build", "-p", "godly-daemon"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("Failed to run cargo build");
        assert!(status.success(), "cargo build failed");

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir
            .parent()
            .unwrap()
            .join("target")
            .join("debug");
        let daemon_exe = target_dir.join("godly-daemon.exe");
        assert!(
            daemon_exe.exists(),
            "Daemon binary not found at {:?}",
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
// Bridge simulation (replicates real bridge I/O thread)
// ---------------------------------------------------------------------------

struct BridgeRequest {
    request: Request,
    response_tx: mpsc::Sender<Response>,
}

/// Simulated bridge I/O thread: single thread handles ALL pipe I/O.
/// Replicates the real bridge's exact behavior:
///   1. Service ALL pending requests first (high priority)
///   2. After each request write, read pipe immediately for the response
///   3. Read up to MAX_EVENTS events from the pipe (non-blocking via PeekNamedPipe)
///   4. Sleep/yield when idle
fn bridge_io_loop(
    mut pipe: std::fs::File,
    request_rx: mpsc::Receiver<BridgeRequest>,
    stop: Arc<AtomicBool>,
    event_counter: Arc<AtomicU64>,
) {
    const MAX_EVENTS_BEFORE_CHECK: usize = 2;

    let raw_handle = pipe.as_raw_handle() as *mut winapi::ctypes::c_void;
    let mut pending_responses: VecDeque<mpsc::Sender<Response>> = VecDeque::new();

    while !stop.load(Ordering::Relaxed) {
        let mut did_work = false;

        // Step 1: Service ALL pending requests (high priority)
        loop {
            match request_rx.try_recv() {
                Ok(req) => {
                    godly_protocol::write_request(&mut pipe, &req.request).expect("bridge write");
                    pending_responses.push_back(req.response_tx);
                    did_work = true;

                    // Read pipe immediately for the response
                    loop {
                        match peek_pipe(raw_handle) {
                            PeekResult::Data => {
                                match godly_protocol::read_daemon_message(&mut pipe) {
                                    Ok(Some(DaemonMessage::Event(_))) => {
                                        event_counter.fetch_add(1, Ordering::Relaxed);
                                        continue; // Keep reading until we get the response
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
                            PeekResult::Empty => {
                                std::thread::yield_now();
                                continue;
                            }
                            PeekResult::Error => {
                                stop.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                    }

                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    continue; // Check for more queued requests
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }

        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Step 2: Read up to MAX_EVENTS_BEFORE_CHECK events from the pipe
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
                            did_work = true;
                        }
                        Ok(Some(DaemonMessage::Response(resp))) => {
                            if let Some(tx) = pending_responses.pop_front() {
                                let _ = tx.send(resp);
                            }
                            did_work = true;
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

        if !did_work {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

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
// Helper: type a command and wait for shell to process it
// ---------------------------------------------------------------------------

fn type_command_and_wait(
    tx: &mpsc::Sender<BridgeRequest>,
    session_id: &str,
    command: &[u8],
    settle_ms: u64,
) {
    // Type each character with a small gap (simulates real typing speed)
    for &ch in command {
        let _ = bridge_request(
            tx,
            Request::Write {
                session_id: session_id.to_string(),
                data: vec![ch],
            },
            Duration::from_secs(5),
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // Press Enter
    let _ = bridge_request(
        tx,
        Request::Write {
            session_id: session_id.to_string(),
            data: b"\r".to_vec(),
        },
        Duration::from_secs(5),
    );
    // Wait for command to execute and shell to settle
    std::thread::sleep(Duration::from_millis(settle_ms));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug #149 (regression): Arrow-up history navigation latency.
///
/// Reproduces the exact user scenario:
/// 1. Open a terminal session
/// 2. Type several commands to build shell history
/// 3. Press Up arrow repeatedly to navigate history
/// 4. Measure the round-trip: Write(ESC[A) → ReadRichGrid
///
/// The full pipeline is simulated: [Command thread] → [Bridge I/O thread] → [Daemon]
///
/// Arrow-up produces more complex output than single-char echo because the
/// shell redraws the entire prompt line. This means more bytes through the pipe,
/// more VT parsing in the daemon, and more dirty rows in the grid snapshot.
///
/// Target: p95 < 100ms in release mode. Debug builds add ~10x serde overhead,
/// but the architecture bottleneck (bridge I/O contention) is build-independent.
#[test]
#[ntest::timeout(120_000)]
fn arrow_up_history_latency_through_bridge() {
    let daemon = DaemonFixture::spawn("arrow-up-bridge");
    let pipe = daemon.connect();

    let mut setup_pipe = daemon.connect();
    let session_id = "arrow-up-bridge".to_string();

    let resp = send_request_blocking(&mut setup_pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach on the bridge pipe
    let mut bridge_pipe = pipe;
    godly_protocol::write_request(
        &mut bridge_pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    )
    .expect("attach write");
    loop {
        let msg: DaemonMessage =
            godly_protocol::read_daemon_message(&mut bridge_pipe).expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(_) => break,
            DaemonMessage::Event(_) => continue,
        }
    }

    // Wait for shell to fully initialize
    std::thread::sleep(Duration::from_secs(3));

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

    // Wait for bridge to start and drain initial events
    std::thread::sleep(Duration::from_millis(500));

    // Step 1: Build shell history by typing several commands
    // Each command produces output and adds to PSReadLine's history buffer.
    type_command_and_wait(&req_tx, &session_id, b"echo first-command", 1500);
    type_command_and_wait(&req_tx, &session_id, b"echo second-command", 1500);
    type_command_and_wait(&req_tx, &session_id, b"echo third-command", 1500);
    type_command_and_wait(&req_tx, &session_id, b"echo fourth-command", 1500);

    // Wait for everything to fully settle — shell prompt displayed, no pending output
    std::thread::sleep(Duration::from_secs(2));

    // Take a baseline grid snapshot to clear any dirty state
    let _ = bridge_request(
        &req_tx,
        Request::ReadRichGrid {
            session_id: session_id.clone(),
        },
        Duration::from_secs(5),
    );

    // Step 2: Measure arrow-up latency
    // ESC[A is the ANSI escape sequence for Up arrow
    let up_arrow = b"\x1b[A";
    let num_presses = 10;
    let mut round_trips = Vec::with_capacity(num_presses);

    for i in 0..num_presses {
        let events_before = event_counter.load(Ordering::Relaxed);
        let keystroke_start = Instant::now();

        // Write the Up arrow escape sequence through the bridge
        let (resp, write_lat) = bridge_request(
            &req_tx,
            Request::Write {
                session_id: session_id.clone(),
                data: up_arrow.to_vec(),
            },
            Duration::from_secs(10),
        )
        .unwrap_or_else(|e| panic!("Write Up arrow #{} failed: {}", i, e));
        assert!(matches!(resp, Response::Ok));

        // Wait for shell to process the escape sequence and produce output.
        // Shell history navigation involves:
        //   - PSReadLine processes ESC[A
        //   - Looks up previous command in history
        //   - Clears current input line (CSI K or similar)
        //   - Moves cursor to start of line
        //   - Writes the previous command text
        // This produces several VT sequences flowing back through the daemon.
        std::thread::sleep(Duration::from_millis(20));

        // Request grid snapshot through the bridge
        let (resp, snapshot_lat) = bridge_request(
            &req_tx,
            Request::ReadRichGrid {
                session_id: session_id.clone(),
            },
            Duration::from_secs(10),
        )
        .unwrap_or_else(|e| panic!("ReadRichGrid #{} failed: {}", i, e));
        assert!(matches!(resp, Response::RichGrid { .. }));

        let total = keystroke_start.elapsed();
        let events_during = event_counter.load(Ordering::Relaxed) - events_before;
        round_trips.push(total);

        eprintln!(
            "[arrow-up] #{}: total={:.1}ms (write={:.1}ms, snapshot={:.1}ms) events={}",
            i,
            total.as_secs_f64() * 1000.0,
            write_lat.as_secs_f64() * 1000.0,
            snapshot_lat.as_secs_f64() * 1000.0,
            events_during,
        );
    }

    let avg_ms =
        round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / round_trips.len() as f64;
    let mut sorted: Vec<f64> = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50_ms = sorted[sorted.len() / 2];
    let p95_idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
    let p95_ms = sorted[p95_idx];
    let max_ms = sorted.last().copied().unwrap_or(0.0);

    eprintln!(
        "\n[arrow-up] RESULT: avg={:.1}ms  p50={:.1}ms  p95={:.1}ms  max={:.1}ms\n",
        avg_ms, p50_ms, p95_ms, max_ms
    );

    // Cleanup bridge
    stop.store(true, Ordering::Relaxed);
    drop(req_tx);
    let _ = bridge_thread.join();

    // Cleanup session
    let _ = send_request_blocking(
        &mut setup_pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );

    // Bug #149: Arrow-up history navigation should feel instantaneous.
    // The p95 round-trip through the bridge (Write + ReadRichGrid) must stay
    // under 100ms for input to feel responsive. This threshold accounts for:
    //   - Pipe round-trip (~1ms release, ~5ms debug)
    //   - JSON serialization of grid snapshot (~1ms release, ~10ms debug)
    //   - Bridge I/O thread scheduling (~1-5ms)
    //   - Shell history processing (~5-20ms)
    //   - 20ms intentional wait for shell echo
    //
    // In practice, the bridge architecture (single I/O thread, event/request
    // contention) adds significant overhead. If this test fails, the bridge
    // is adding too much latency for arrow-up to feel responsive.
    //
    // Note: the real app adds Tauri invoke() dispatch + JS event loop on top
    // of this, so the actual user-perceived latency is HIGHER than what this
    // test measures.
    //
    // CI VMs have variable performance, so we use relaxed thresholds there.
    let threshold = if std::env::var("CI").is_ok() { 250.0 } else { 100.0 };
    assert!(
        p95_ms < threshold,
        "Bug #149: Arrow-up history latency p95={:.1}ms exceeds {:.0}ms target.\n\
         avg={:.1}ms  p50={:.1}ms  max={:.1}ms\n\
         The bridge I/O architecture is adding too much latency for arrow-up \n\
         to feel responsive. See issue #149 remaining work items for fix candidates.",
        p95_ms, threshold, avg_ms, p50_ms, max_ms
    );
}

/// Daemon-only arrow-up latency (no bridge simulation).
/// Establishes the daemon-side baseline: pipe round-trip + Mutex lock + serialize.
/// If THIS test has high latency, the problem is in the daemon. If this is fast
/// but the bridge test is slow, the problem is in the bridge I/O architecture.
#[test]
#[ntest::timeout(120_000)]
fn arrow_up_daemon_only_latency() {
    let daemon = DaemonFixture::spawn("arrow-up-daemon");
    let mut pipe = daemon.connect();

    let resp = send_request_blocking(&mut pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let session_id = "arrow-up-daemon".to_string();
    let resp = send_request_blocking(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request_blocking(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for shell to initialize
    std::thread::sleep(Duration::from_secs(3));
    drain_events(&mut pipe);

    // Build history: type commands directly through the pipe
    let commands: &[&[u8]] = &[
        b"echo history-one\r",
        b"echo history-two\r",
        b"echo history-three\r",
        b"echo history-four\r",
    ];
    for cmd in commands {
        let resp = send_request_blocking(
            &mut pipe,
            &Request::Write {
                session_id: session_id.clone(),
                data: cmd.to_vec(),
            },
        );
        assert!(matches!(resp, Response::Ok));
        std::thread::sleep(Duration::from_secs(1));
        drain_events(&mut pipe);
    }

    // Wait for shell to fully settle
    std::thread::sleep(Duration::from_secs(2));
    drain_events(&mut pipe);

    // Clear dirty state with a baseline snapshot
    let resp = send_request_blocking(
        &mut pipe,
        &Request::ReadRichGrid {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::RichGrid { .. }));

    // Measure arrow-up round-trip: Write(ESC[A) → wait for echo → ReadRichGrid
    let up_arrow = b"\x1b[A";
    let num_presses = 10;
    let mut round_trips = Vec::with_capacity(num_presses);

    for i in 0..num_presses {
        let start = Instant::now();

        // Send Up arrow
        godly_protocol::write_request(
            &mut pipe,
            &Request::Write {
                session_id: session_id.clone(),
                data: up_arrow.to_vec(),
            },
        )
        .expect("write up arrow");

        // Read Write response (skip events)
        loop {
            let msg = godly_protocol::read_daemon_message(&mut pipe)
                .expect("read")
                .expect("EOF");
            match msg {
                DaemonMessage::Response(Response::Ok) => break,
                DaemonMessage::Response(r) => panic!("Unexpected response: {:?}", r),
                DaemonMessage::Event(_) => continue,
            }
        }

        // Wait for shell to process and produce output
        std::thread::sleep(Duration::from_millis(20));

        // Read grid snapshot
        godly_protocol::write_request(
            &mut pipe,
            &Request::ReadRichGrid {
                session_id: session_id.clone(),
            },
        )
        .expect("write grid request");

        let mut events_skipped = 0u32;
        loop {
            let msg = godly_protocol::read_daemon_message(&mut pipe)
                .expect("read")
                .expect("EOF");
            match msg {
                DaemonMessage::Response(Response::RichGrid { .. }) => break,
                DaemonMessage::Response(r) => panic!("Unexpected response: {:?}", r),
                DaemonMessage::Event(_) => {
                    events_skipped += 1;
                    continue;
                }
            }
        }

        let elapsed = start.elapsed();
        round_trips.push(elapsed);

        eprintln!(
            "[arrow-up-direct] #{}: {:.1}ms (skipped {} events)",
            i,
            elapsed.as_secs_f64() * 1000.0,
            events_skipped
        );
    }

    let avg_ms =
        round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / round_trips.len() as f64;
    let mut sorted: Vec<f64> = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50_ms = sorted[sorted.len() / 2];
    let p95_idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
    let p95_ms = sorted[p95_idx];
    let max_ms = sorted.last().copied().unwrap_or(0.0);

    eprintln!(
        "\n[arrow-up-direct] RESULT: avg={:.1}ms  p50={:.1}ms  p95={:.1}ms  max={:.1}ms\n",
        avg_ms, p50_ms, p95_ms, max_ms
    );

    // Cleanup
    let _ = send_request_blocking(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );

    // Bug #149: Daemon-side arrow-up should complete well under 100ms.
    // This test isolates the daemon from the bridge. If THIS fails, the
    // daemon's Mutex contention or JSON serialization is the bottleneck.
    // If this passes but the bridge test fails, the bridge architecture is
    // the bottleneck.
    let threshold = if std::env::var("CI").is_ok() { 250.0 } else { 100.0 };
    assert!(
        p95_ms < threshold,
        "Bug #149: Daemon-only arrow-up p95={:.1}ms exceeds {:.0}ms target.\n\
         avg={:.1}ms  p50={:.1}ms  max={:.1}ms\n\
         The daemon is too slow for responsive arrow-up history navigation.",
        p95_ms, threshold, avg_ms, p50_ms, max_ms
    );
}

/// Arrow-up with multi-session contention (the real-world bottleneck).
///
/// Bug #149: In the real app, the user has multiple terminal tabs. Events from
/// ALL sessions flow through a single bridge I/O thread. When one terminal is
/// producing output (e.g., a build running in another tab), arrow-up requests
/// for the active terminal get queued behind event reads from the other session.
///
/// This test creates TWO sessions on the same pipe:
///   - Session A: produces heavy continuous output (simulates a build/log tail)
///   - Session B: user presses Up arrow for history navigation
///
/// The bridge I/O thread must read events from Session A AND service requests
/// for Session B through the same pipe, creating head-of-line blocking.
#[test]
#[ntest::timeout(180_000)]
fn arrow_up_during_multi_session_contention() {
    let daemon = DaemonFixture::spawn("arrow-up-contention");

    // Setup pipe for creating sessions
    let mut setup_pipe = daemon.connect();

    let resp = send_request_blocking(&mut setup_pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    // Create Session A: the noisy session (heavy output)
    let session_a = "contention-noisy".to_string();
    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::CreateSession {
            id: session_a.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Create Session B: the user's active session (arrow-up testing)
    let session_b = "contention-active".to_string();
    let resp = send_request_blocking(
        &mut setup_pipe,
        &Request::CreateSession {
            id: session_b.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 30,
            cols: 120,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Bridge pipe: attach to BOTH sessions (real bridge attaches all terminals)
    let mut bridge_pipe = daemon.connect();

    // Attach Session A
    godly_protocol::write_request(
        &mut bridge_pipe,
        &Request::Attach {
            session_id: session_a.clone(),
        },
    )
    .expect("attach A");
    loop {
        let msg: DaemonMessage =
            godly_protocol::read_daemon_message(&mut bridge_pipe).expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(_) => break,
            DaemonMessage::Event(_) => continue,
        }
    }

    // Attach Session B
    godly_protocol::write_request(
        &mut bridge_pipe,
        &Request::Attach {
            session_id: session_b.clone(),
        },
    )
    .expect("attach B");
    loop {
        let msg: DaemonMessage =
            godly_protocol::read_daemon_message(&mut bridge_pipe).expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(_) => break,
            DaemonMessage::Event(_) => continue,
        }
    }

    // Wait for both shells to initialize
    std::thread::sleep(Duration::from_secs(3));

    // Start bridge I/O thread
    let (req_tx, req_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let event_counter = Arc::new(AtomicU64::new(0));
    let event_counter_clone = event_counter.clone();

    let _bridge_thread = std::thread::Builder::new()
        .name("test-bridge".into())
        .spawn(move || bridge_io_loop(bridge_pipe, req_rx, stop_clone, event_counter_clone))
        .expect("spawn bridge");

    std::thread::sleep(Duration::from_millis(500));

    // Build history on Session B FIRST (before starting heavy output).
    // This ensures the shell is idle and history is populated before contention starts.
    type_command_and_wait(&req_tx, &session_b, b"echo active-cmd-one", 1500);
    type_command_and_wait(&req_tx, &session_b, b"echo active-cmd-two", 1500);
    type_command_and_wait(&req_tx, &session_b, b"echo active-cmd-three", 1500);

    // Take a baseline snapshot to clear dirty state
    let _ = bridge_request(
        &req_tx,
        Request::ReadRichGrid {
            session_id: session_b.clone(),
        },
        Duration::from_secs(5),
    );

    std::thread::sleep(Duration::from_secs(1));

    // Start heavy CONTINUOUS output on Session A AFTER history is built.
    // Uses PowerShell's while loop to produce output indefinitely until the
    // session is closed. Each iteration outputs ~100 bytes and sleeps briefly
    // to produce a sustained stream (not a burst that finishes quickly).
    // The 1..10000000 range ensures output runs for the entire test duration.
    let (resp, _) = bridge_request(
        &req_tx,
        Request::Write {
            session_id: session_a.clone(),
            data: b"1..10000000 | ForEach-Object { Write-Output (\"Line $_ \" + ('A' * 80)) }\r\n"
                .to_vec(),
        },
        Duration::from_secs(10),
    )
    .expect("start heavy output on session A");
    assert!(matches!(resp, Response::Ok));

    // Wait for output events to start flowing through the pipe
    std::thread::sleep(Duration::from_secs(3));
    let events_before = event_counter.load(Ordering::Relaxed);
    eprintln!(
        "[contention] Events from Session A before typing: {}",
        events_before
    );
    assert!(
        events_before > 0,
        "Session A output events should be flowing through the pipe"
    );

    // Verify output is STILL flowing (not a burst that already finished)
    let check_start = event_counter.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_secs(1));
    let check_end = event_counter.load(Ordering::Relaxed);
    let events_per_sec = check_end - check_start;
    eprintln!(
        "[contention] Session A output rate: {} events/sec",
        events_per_sec
    );
    assert!(
        events_per_sec > 0,
        "Session A output must be actively flowing during arrow-up test"
    );

    // Now press Up arrow on Session B while Session A floods events
    let up_arrow = b"\x1b[A";
    let num_presses = 8;
    let mut round_trips = Vec::with_capacity(num_presses);

    for i in 0..num_presses {
        let events_at_start = event_counter.load(Ordering::Relaxed);
        let keystroke_start = Instant::now();

        // Write Up arrow to Session B through the contended bridge
        let (resp, write_lat) = bridge_request(
            &req_tx,
            Request::Write {
                session_id: session_b.clone(),
                data: up_arrow.to_vec(),
            },
            Duration::from_secs(15),
        )
        .unwrap_or_else(|e| panic!("Write Up arrow #{} failed: {}", i, e));
        assert!(matches!(resp, Response::Ok));

        // Wait for shell to process history navigation
        std::thread::sleep(Duration::from_millis(20));

        // Request grid snapshot for Session B through the contended bridge
        let (resp, snapshot_lat) = bridge_request(
            &req_tx,
            Request::ReadRichGrid {
                session_id: session_b.clone(),
            },
            Duration::from_secs(15),
        )
        .unwrap_or_else(|e| panic!("ReadRichGrid #{} failed: {}", i, e));
        assert!(matches!(resp, Response::RichGrid { .. }));

        let total = keystroke_start.elapsed();
        let events_during = event_counter.load(Ordering::Relaxed) - events_at_start;
        round_trips.push(total);

        eprintln!(
            "[contention] #{}: total={:.1}ms (write={:.1}ms, snapshot={:.1}ms) events_during={}",
            i,
            total.as_secs_f64() * 1000.0,
            write_lat.as_secs_f64() * 1000.0,
            snapshot_lat.as_secs_f64() * 1000.0,
            events_during,
        );
    }

    let total_events = event_counter.load(Ordering::Relaxed);
    let avg_ms =
        round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / round_trips.len() as f64;
    let mut sorted: Vec<f64> = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50_ms = sorted[sorted.len() / 2];
    let p95_idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
    let p95_ms = sorted[p95_idx];
    let max_ms = sorted.last().copied().unwrap_or(0.0);

    eprintln!(
        "\n[contention] RESULT: avg={:.1}ms  p50={:.1}ms  p95={:.1}ms  max={:.1}ms  total_events={}\n",
        avg_ms, p50_ms, p95_ms, max_ms, total_events
    );

    // Stop bridge thread first — dropping req_tx disconnects the channel,
    // and stop flag causes the bridge loop to exit on next iteration.
    // The daemon process will be killed by DaemonFixture::Drop.
    stop.store(true, Ordering::Relaxed);
    drop(req_tx);
    // Don't join() — bridge thread may be stuck reading events from the pipe.
    // DaemonFixture::Drop kills the daemon, which breaks the pipe, which unblocks
    // the bridge thread.

    // Bug #149: Arrow-up must stay responsive even when other tabs produce output.
    //
    // The bridge I/O thread reads up to 8 events (MAX_EVENTS_BEFORE_REQUEST_CHECK)
    // per loop iteration before checking for pending requests. Under heavy output
    // from Session A, each loop iteration reads 8 events (each ~40ms to deserialize
    // in debug mode), adding hundreds of milliseconds before the arrow-up request
    // from Session B gets serviced.
    //
    // This is the primary bottleneck users experience: input lag scales with the
    // output rate of OTHER terminals, not just the active one.
    //
    // Target: p95 < 150ms. In practice, multi-session contention pushes latency
    // well above this because the bridge must drain events before servicing requests.
    // The real app adds Tauri + JS overhead on top (~30-60ms), so the daemon+bridge
    // portion must stay well under the perceptible threshold.
    //
    // CI VMs have high variance under contention — use much more relaxed threshold.
    let threshold = if std::env::var("CI").is_ok() { 2000.0 } else { 150.0 };
    assert!(
        p95_ms < threshold,
        "Bug #149: Arrow-up during multi-session contention p95={:.1}ms exceeds {:.0}ms.\n\
         avg={:.1}ms  p50={:.1}ms  max={:.1}ms  total_events={}\n\
         Bridge I/O thread contention from other sessions causes arrow-up lag.\n\
         The single-threaded bridge architecture cannot handle concurrent output \n\
         and input without head-of-line blocking. See issue #149 remaining work.",
        p95_ms, threshold, avg_ms, p50_ms, max_ms, total_events
    );
}
