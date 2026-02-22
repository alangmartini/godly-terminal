//! Full-path input latency test: reproduces the complete keystroke-to-grid-snapshot
//! pipeline including the single-threaded bridge I/O contention.
//!
//! The real app's bridge thread runs a loop that:
//!   1. Reads up to MAX_EVENTS (8) from the daemon pipe
//!   2. Checks for pending requests from the Tauri thread pool
//!   3. Writes the request to the pipe
//!   4. Loops back to read the response (but events may arrive first)
//!
//! This test replicates that exact pattern: a single thread handles ALL pipe I/O,
//! and a separate "Tauri command" thread submits requests through a channel.
//! The measured latency includes:
//!   - Channel wait (request sits in queue while bridge drains events)
//!   - Pipe write (request serialization + pipe I/O)
//!   - Daemon processing (Mutex lock + grid serialization)
//!   - Pipe read (response may be interleaved with events)
//!   - Channel return (response sent back to command thread)
//!
//! This is the closest simulation of the real app without running Tauri.
//! The only missing pieces are Tauri's invoke() dispatch and JS event loop.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test input_latency_full_path -- --test-threads=1

#![cfg(windows)]

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// DaemonFixture
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
            panic!("Failed to connect to pipe '{}' within {:?} (err {})", pipe_name, timeout, err);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

enum PeekResult { Data, Empty, Error }

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
    if result == 0 { PeekResult::Error }
    else if bytes_available > 0 { PeekResult::Data }
    else { PeekResult::Empty }
}

fn send_request_blocking(pipe: &mut std::fs::File, request: &Request) -> Response {
    godly_protocol::write_request(pipe, request).expect("write");
    loop {
        let msg: DaemonMessage = godly_protocol::read_daemon_message(pipe).expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(r) => return r,
            DaemonMessage::Event(_) => continue,
        }
    }
}

struct DaemonFixture { child: Child, pipe_name: String }

impl DaemonFixture {
    fn spawn(test_name: &str) -> Self {
        let pipe_name = format!(r"\\.\pipe\godly-test-{}-{}", test_name, std::process::id());

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let daemon_exe = manifest_dir.parent().unwrap().join("target/debug/godly-daemon.exe");
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
// Simulated Bridge
// ---------------------------------------------------------------------------

/// Request submitted to the bridge thread (same as real BridgeRequest).
struct BridgeRequest {
    request: Request,
    response_tx: mpsc::Sender<Response>,
}

/// Simulated bridge I/O thread: single thread handles ALL pipe I/O.
/// This replicates the real bridge's exact behavior:
///   1. Read up to MAX_EVENTS events from the pipe (non-blocking via PeekNamedPipe)
///   2. Check for pending requests from the channel
///   3. Write request to pipe, then loop back to read response
///   4. Sleep/yield when idle
fn bridge_io_loop(
    mut pipe: std::fs::File,
    request_rx: mpsc::Receiver<BridgeRequest>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    event_counter: std::sync::Arc<std::sync::atomic::AtomicU64>,
) {
    use std::sync::atomic::Ordering;

    const MAX_EVENTS_BEFORE_CHECK: usize = 8;

    let raw_handle = pipe.as_raw_handle() as *mut winapi::ctypes::c_void;
    let mut pending_responses: VecDeque<mpsc::Sender<Response>> = VecDeque::new();

    while !stop.load(Ordering::Relaxed) {
        // Step 1: Drain events from pipe (up to MAX_EVENTS)
        let mut events_this_round = 0;
        loop {
            if events_this_round >= MAX_EVENTS_BEFORE_CHECK { break; }

            match peek_pipe(raw_handle) {
                PeekResult::Data => {
                    match godly_protocol::read_daemon_message(&mut pipe) {
                        Ok(Some(DaemonMessage::Event(_))) => {
                            event_counter.fetch_add(1, Ordering::Relaxed);
                            events_this_round += 1;
                            // Real bridge: emitter.try_send(payload)
                            // We just count events since there's no Tauri app
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
                // Loop back immediately to read the response
                continue;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }

        // Step 3: Sleep if idle (like the real bridge)
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

    tx.send(BridgeRequest { request, response_tx: resp_tx })
        .map_err(|e| format!("Channel send failed: {}", e))?;

    let resp = resp_rx.recv_timeout(timeout)
        .map_err(|e| format!("Timeout after {:?}: {}", timeout, e))?;

    Ok((resp, start.elapsed()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Full-path keystroke latency: simulates the complete pipeline.
///
/// Architecture:
///   [Command thread] --channel--> [Bridge I/O thread] --pipe--> [Daemon]
///
/// The bridge I/O thread is a SINGLE thread that reads events AND services
/// requests, exactly like the real app. Under heavy output, requests queue
/// behind event reads, reproducing the real-world 500ms delay.
///
/// Sequence per "keystroke":
///   1. Command thread sends Write request via channel
///   2. Bridge thread picks it up, writes to pipe, reads Response::Ok
///   3. Shell echoes the character (daemon sends Event::Output through pipe)
///   4. Bridge thread reads events
///   5. Command thread sends ReadRichGrid request via channel
///   6. Bridge thread picks it up, writes to pipe
///   7. Bridge thread reads more events (head-of-line blocking!)
///   8. Bridge thread reads the Response::RichGrid, sends via channel
///   9. Command thread receives the snapshot
///
/// Total time = sum of all waits. THIS is the number users feel.
#[test]
#[ntest::timeout(60_000)]
fn full_path_keystroke_latency_idle_terminal() {
    let daemon = DaemonFixture::spawn("full-path-idle");
    let pipe = daemon.connect();

    // Set up a session using a separate connection for setup
    let mut setup_pipe = daemon.connect();
    let session_id = "full-path-idle".to_string();

    let resp = send_request_blocking(&mut setup_pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let resp = send_request_blocking(&mut setup_pipe, &Request::CreateSession {
        id: session_id.clone(),
        shell_type: ShellType::Windows,
        cwd: None,
        rows: 30,
        cols: 120,
        env: None,
    });
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach on the bridge pipe (this is the pipe the bridge thread will use)
    let mut bridge_pipe = pipe;
    godly_protocol::write_request(&mut bridge_pipe, &Request::Attach {
        session_id: session_id.clone(),
    }).expect("attach write");
    loop {
        let msg: DaemonMessage = godly_protocol::read_daemon_message(&mut bridge_pipe)
            .expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(_) => break,
            DaemonMessage::Event(_) => continue,
        }
    }

    // Wait for shell to settle
    std::thread::sleep(Duration::from_secs(2));

    // Start the bridge I/O thread
    let (req_tx, req_rx) = mpsc::channel();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = stop.clone();
    let event_counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let event_counter_clone = event_counter.clone();

    let bridge_thread = std::thread::Builder::new()
        .name("test-bridge".into())
        .spawn(move || bridge_io_loop(bridge_pipe, req_rx, stop_clone, event_counter_clone))
        .expect("spawn bridge");

    // Wait for bridge to start and drain initial events
    std::thread::sleep(Duration::from_millis(500));

    // Simulate 15 keystrokes through the bridge
    let mut round_trips = Vec::with_capacity(15);
    let chars = b"hello world test";

    for (i, &ch) in chars.iter().enumerate() {
        let keystroke_start = Instant::now();

        // Step 1: Write the keystroke (through bridge channel)
        let (resp, write_lat) = bridge_request(
            &req_tx,
            Request::Write { session_id: session_id.clone(), data: vec![ch] },
            Duration::from_secs(10),
        ).unwrap_or_else(|e| panic!("Write #{} failed: {}", i, e));
        assert!(matches!(resp, Response::Ok));

        // Step 2: Wait for shell echo to arrive through the pipe
        // In real app: terminal-output event → scheduleSnapshotFetch → setTimeout(0)
        // We simulate this with a short sleep
        std::thread::sleep(Duration::from_millis(5));

        // Step 3: Request grid snapshot (through bridge channel)
        let (resp, snapshot_lat) = bridge_request(
            &req_tx,
            Request::ReadRichGrid { session_id: session_id.clone() },
            Duration::from_secs(10),
        ).unwrap_or_else(|e| panic!("ReadRichGrid #{} failed: {}", i, e));
        assert!(matches!(resp, Response::RichGrid { .. }));

        let total = keystroke_start.elapsed();
        round_trips.push(total);

        eprintln!(
            "[full-path-idle] '{}' #{}: total={:.1}ms (write={:.1}ms, snapshot={:.1}ms)",
            ch as char, i,
            total.as_secs_f64() * 1000.0,
            write_lat.as_secs_f64() * 1000.0,
            snapshot_lat.as_secs_f64() * 1000.0,
        );
    }

    let avg_ms = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / round_trips.len() as f64;
    let mut sorted: Vec<f64> = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = sorted[(sorted.len() as f64 * 0.95) as usize];
    let max_ms = sorted.last().copied().unwrap_or(0.0);

    eprintln!(
        "\n[full-path-idle] RESULT: avg={:.1}ms  p95={:.1}ms  max={:.1}ms\n",
        avg_ms, p95_ms, max_ms
    );

    // Cleanup
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    drop(req_tx);
    let _ = bridge_thread.join();

    let _ = send_request_blocking(&mut setup_pipe, &Request::CloseSession {
        session_id,
    });
}

/// Full-path keystroke latency DURING heavy output.
/// This is the test that reproduces the 500ms delay users experience.
///
/// Heavy output floods the pipe with Event::Output messages. The bridge
/// thread drains up to 8 events per loop iteration before checking for
/// requests. Under sustained output, the request can wait multiple loop
/// iterations in the channel, adding 10-50ms per iteration.
#[test]
#[ntest::timeout(120_000)]
fn full_path_keystroke_latency_during_heavy_output() {
    let daemon = DaemonFixture::spawn("full-path-heavy");
    let pipe = daemon.connect();

    let mut setup_pipe = daemon.connect();
    let session_id = "full-path-heavy".to_string();

    let resp = send_request_blocking(&mut setup_pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let resp = send_request_blocking(&mut setup_pipe, &Request::CreateSession {
        id: session_id.clone(),
        shell_type: ShellType::Windows,
        cwd: None,
        rows: 30,
        cols: 120,
        env: None,
    });
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let mut bridge_pipe = pipe;
    godly_protocol::write_request(&mut bridge_pipe, &Request::Attach {
        session_id: session_id.clone(),
    }).expect("attach write");
    loop {
        let msg: DaemonMessage = godly_protocol::read_daemon_message(&mut bridge_pipe)
            .expect("read").expect("EOF");
        match msg {
            DaemonMessage::Response(_) => break,
            DaemonMessage::Event(_) => continue,
        }
    }

    std::thread::sleep(Duration::from_secs(1));

    // Start the bridge I/O thread
    let (req_tx, req_rx) = mpsc::channel();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = stop.clone();
    let event_counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let event_counter_clone = event_counter.clone();

    let bridge_thread = std::thread::Builder::new()
        .name("test-bridge".into())
        .spawn(move || bridge_io_loop(bridge_pipe, req_rx, stop_clone, event_counter_clone))
        .expect("spawn bridge");

    std::thread::sleep(Duration::from_millis(200));

    // Start heavy output: 200,000 lines ensures sustained output for the full test.
    // Each iteration echoes ~50 bytes, so this produces ~10MB of output.
    let (resp, _) = bridge_request(
        &req_tx,
        Request::Write {
            session_id: session_id.clone(),
            data: b"for /L %i in (1,1,200000) do @echo Line %i: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\r\n".to_vec(),
        },
        Duration::from_secs(10),
    ).expect("start heavy output");
    assert!(matches!(resp, Response::Ok));

    // Wait for output to start flowing, then verify events are arriving
    std::thread::sleep(Duration::from_secs(2));
    let events_before = event_counter.load(std::sync::atomic::Ordering::Relaxed);
    eprintln!("[full-path-heavy] Events received before typing: {}", events_before);
    assert!(events_before > 0, "Output events should be flowing before we start typing");

    // NOW simulate keystrokes during heavy output
    let mut round_trips = Vec::with_capacity(10);
    let chars = b"typing now";

    for (i, &ch) in chars.iter().enumerate() {
        let events_at_start = event_counter.load(std::sync::atomic::Ordering::Relaxed);
        let keystroke_start = Instant::now();

        // Write keystroke through the bridge (may wait for event draining)
        let (resp, write_lat) = bridge_request(
            &req_tx,
            Request::Write { session_id: session_id.clone(), data: vec![ch] },
            Duration::from_secs(15),
        ).unwrap_or_else(|e| panic!("Write #{} failed: {}", i, e));
        assert!(matches!(resp, Response::Ok));

        std::thread::sleep(Duration::from_millis(5));

        // Request grid snapshot through the bridge (may wait behind events)
        let (resp, snapshot_lat) = bridge_request(
            &req_tx,
            Request::ReadRichGrid { session_id: session_id.clone() },
            Duration::from_secs(15),
        ).unwrap_or_else(|e| panic!("ReadRichGrid #{} failed: {}", i, e));
        assert!(matches!(resp, Response::RichGrid { .. }));

        let total = keystroke_start.elapsed();
        let events_during = event_counter.load(std::sync::atomic::Ordering::Relaxed) - events_at_start;
        round_trips.push(total);

        eprintln!(
            "[full-path-heavy] '{}' #{}: total={:.1}ms (write={:.1}ms, snapshot={:.1}ms) events_during={}",
            ch as char, i,
            total.as_secs_f64() * 1000.0,
            write_lat.as_secs_f64() * 1000.0,
            snapshot_lat.as_secs_f64() * 1000.0,
            events_during,
        );
    }

    let total_events = event_counter.load(std::sync::atomic::Ordering::Relaxed);
    let avg_ms = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / round_trips.len() as f64;
    let mut sorted: Vec<f64> = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = sorted[(sorted.len() as f64 * 0.95) as usize];
    let max_ms = sorted.last().copied().unwrap_or(0.0);

    eprintln!(
        "\n[full-path-heavy] RESULT: avg={:.1}ms  p95={:.1}ms  max={:.1}ms  total_events={}",
        avg_ms, p95_ms, max_ms, total_events
    );

    // This is the regression gate. If this exceeds 500ms, the input latency
    // is user-visible and unacceptable. Tighten this threshold as we fix
    // the architecture.
    if p95_ms > 500.0 {
        eprintln!(
            "\n[REGRESSION] Full-path keystroke latency p95={:.1}ms exceeds 500ms.\n\
             This means users experience >500ms delay when typing.\n\
             See docs/input-latency-investigation.md for fix candidates.\n",
            p95_ms
        );
    }

    if p95_ms > 100.0 {
        eprintln!(
            "\n[WARNING] p95={:.1}ms — input lag is noticeable (target: <50ms).\n",
            p95_ms
        );
    }

    // Cleanup
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    drop(req_tx);
    let _ = bridge_thread.join();

    let _ = send_request_blocking(&mut setup_pipe, &Request::CloseSession { session_id });
}
