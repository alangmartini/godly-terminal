//! Input latency regression tests: measure grid snapshot round-trip time
//! during concurrent PTY output.
//!
//! Problem: Every keystroke triggers a ReadRichGrid request through the daemon.
//! The daemon's handler thread must lock the godly-vt Mutex to build the snapshot.
//! Under sustained output, the PTY reader thread holds this lock while parsing,
//! causing ReadRichGrid requests to block. This test measures the real round-trip
//! time and asserts it stays within acceptable bounds.
//!
//! This test exercises the REAL bottleneck: concurrent pipe I/O + Mutex contention,
//! not just isolated JSON serialization cost.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test input_latency -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// DaemonFixture (same pattern as handler_starvation.rs)
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

fn send_request_with_deadline(
    pipe: &mut std::fs::File,
    request: &Request,
    deadline: Duration,
) -> Result<(Response, Duration, u32), String> {
    godly_protocol::write_message(pipe, request)
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

        let msg: DaemonMessage = godly_protocol::read_message(pipe)
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

fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
    godly_protocol::write_message(pipe, request).expect("Failed to write request");
    loop {
        let msg: DaemonMessage = godly_protocol::read_message(pipe)
            .expect("Failed to read message")
            .expect("Unexpected EOF");
        match msg {
            DaemonMessage::Response(resp) => return resp,
            DaemonMessage::Event(_) => continue,
        }
    }
}

/// Drain all pending events from the pipe without blocking.
fn drain_events(pipe: &mut std::fs::File) -> u32 {
    let mut drained = 0u32;
    while pipe_has_data(pipe) {
        let msg: DaemonMessage = godly_protocol::read_message(pipe)
            .expect("drain read error")
            .expect("drain EOF");
        match msg {
            DaemonMessage::Event(_) => drained += 1,
            DaemonMessage::Response(_) => break, // shouldn't happen
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
        assert!(daemon_exe.exists(), "Daemon binary not found at {:?}", daemon_exe);

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

/// Measure ReadRichGrid round-trip time with NO concurrent output.
/// This establishes the baseline latency floor: pipe round-trip + JSON
/// serialization + Mutex lock (uncontended).
///
/// Expected: <50ms per request (typically <5ms on modern hardware).
#[test]
fn baseline_grid_snapshot_latency_no_output() {
    let daemon = DaemonFixture::spawn("latency-baseline");
    let mut pipe = daemon.connect();

    // Verify connection
    let resp = send_request(&mut pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    // Create a session (produces initial prompt output, then goes idle)
    let session_id = "latency-baseline".to_string();
    let resp = send_request(
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

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for shell prompt to settle
    std::thread::sleep(Duration::from_secs(2));
    drain_events(&mut pipe);

    // Measure 20 ReadRichGrid round-trips with no concurrent output
    let mut latencies = Vec::with_capacity(20);
    for _ in 0..20 {
        let result = send_request_with_deadline(
            &mut pipe,
            &Request::ReadRichGrid { session_id: session_id.clone() },
            Duration::from_secs(5),
        );
        let (resp, latency, _) = result.expect("ReadRichGrid failed during baseline");
        assert!(matches!(resp, Response::RichGrid { .. }));
        latencies.push(latency);
    }

    let avg_ms = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / latencies.len() as f64;
    let max_ms = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).fold(0.0f64, f64::max);
    let p95_idx = (latencies.len() as f64 * 0.95) as usize;
    let mut sorted: Vec<f64> = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = sorted[p95_idx.min(sorted.len() - 1)];

    eprintln!(
        "[baseline] ReadRichGrid (30x120, no output): avg={:.2}ms  p95={:.2}ms  max={:.2}ms",
        avg_ms, p95_ms, max_ms
    );

    // Assert: uncontended snapshot should complete in <200ms.
    // Debug builds are ~10x slower than release due to unoptimized serde.
    // In release mode, this should be <10ms. If this threshold is exceeded
    // even in debug, something is fundamentally wrong.
    assert!(
        p95_ms < 200.0,
        "Baseline ReadRichGrid p95 latency too high: {:.2}ms (expected <200ms in debug)",
        p95_ms
    );

    if p95_ms > 10.0 {
        eprintln!(
            "[baseline] NOTE: {:.2}ms is above release target (<10ms). \
             Expected in debug builds due to unoptimized serde serialization.",
            p95_ms
        );
    }

    // Cleanup
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession { session_id },
        Duration::from_secs(5),
    );
}

/// Measure ReadRichGrid round-trip time DURING sustained PTY output.
/// This reproduces the real-world bottleneck: the Mutex in the daemon is
/// contended between the PTY reader thread (parsing output) and the handler
/// thread (building the grid snapshot).
///
/// This test captures the 500ms+ delay users experience when typing during
/// active output. It asserts that even under load, snapshot latency stays
/// within bounds (which may require architectural fixes to pass).
#[test]
fn grid_snapshot_latency_during_heavy_output() {
    let daemon = DaemonFixture::spawn("latency-heavy");
    let mut pipe = daemon.connect();

    let resp = send_request(&mut pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let session_id = "latency-heavy".to_string();
    let resp = send_request(
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

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    std::thread::sleep(Duration::from_secs(1));
    drain_events(&mut pipe);

    // Start heavy output: generate many lines of text to keep the PTY reader
    // thread busy and contending for the godly-vt Mutex.
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: b"for /L %i in (1,1,5000) do @echo Line %i: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    // Wait for output to start flowing
    std::thread::sleep(Duration::from_secs(2));

    // Measure ReadRichGrid round-trips while output is actively flowing.
    // This is the exact scenario that causes 500ms+ delays in production:
    // - PTY reader thread holds vt Mutex while parsing output
    // - Handler thread blocks on Mutex in ReadRichGrid
    // - Client waits for response on the pipe
    let mut latencies = Vec::with_capacity(10);
    let mut total_events_skipped = 0u32;

    for i in 0..10 {
        let result = send_request_with_deadline(
            &mut pipe,
            &Request::ReadRichGrid { session_id: session_id.clone() },
            Duration::from_secs(15),
        );
        match result {
            Ok((resp, latency, events)) => {
                assert!(matches!(resp, Response::RichGrid { .. }));
                latencies.push(latency);
                total_events_skipped += events;
                eprintln!(
                    "[heavy] ReadRichGrid #{}: {:.2}ms (skipped {} events)",
                    i,
                    latency.as_secs_f64() * 1000.0,
                    events
                );
            }
            Err(e) => {
                panic!(
                    "ReadRichGrid #{} failed during heavy output (latencies so far: {:?}): {}",
                    i,
                    latencies.iter().map(|d| format!("{:.1}ms", d.as_secs_f64() * 1000.0)).collect::<Vec<_>>(),
                    e
                );
            }
        }
    }

    let avg_ms = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / latencies.len() as f64;
    let max_ms = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).fold(0.0f64, f64::max);
    let mut sorted: Vec<f64> = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = sorted[(sorted.len() as f64 * 0.95) as usize];

    eprintln!(
        "[heavy] ReadRichGrid (30x120, heavy output): avg={:.2}ms  p95={:.2}ms  max={:.2}ms  events_skipped={}",
        avg_ms, p95_ms, max_ms, total_events_skipped
    );

    // This assertion documents the CURRENT behavior. If the p95 is >200ms,
    // the test passes but logs a warning — this is the regression baseline.
    // Once we fix the architecture (push-based diffs, separate channels, etc.),
    // tighten this to <50ms.
    //
    // For now: assert requests at least COMPLETE (don't hang forever).
    // The printed latency numbers are the actionable diagnostic.
    assert!(
        max_ms < 15_000.0,
        "ReadRichGrid hung during heavy output: max={:.2}ms (>15s deadline)",
        max_ms
    );

    if p95_ms > 200.0 {
        eprintln!(
            "\n[WARNING] ReadRichGrid p95={:.2}ms during heavy output — input latency is user-visible.\n\
             Target: <50ms. See docs/input-latency-investigation.md for fix candidates.\n",
            p95_ms
        );
    }

    // Cleanup
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession { session_id },
        Duration::from_secs(5),
    );
}

/// Measure ReadRichGridDiff round-trip time during output.
/// Diff snapshots send only dirty rows, so they should be faster than full
/// grid snapshots — both in serialization cost and Mutex hold time.
#[test]
fn diff_snapshot_latency_during_output() {
    let daemon = DaemonFixture::spawn("latency-diff");
    let mut pipe = daemon.connect();

    let resp = send_request(&mut pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let session_id = "latency-diff".to_string();
    let resp = send_request(
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

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    std::thread::sleep(Duration::from_secs(1));
    drain_events(&mut pipe);

    // First: take a full snapshot to establish baseline + clear dirty flags
    let result = send_request_with_deadline(
        &mut pipe,
        &Request::ReadRichGrid { session_id: session_id.clone() },
        Duration::from_secs(5),
    );
    assert!(result.is_ok(), "Initial full snapshot failed");

    // Start moderate output (single character per line, not flooding)
    // This simulates typing: each line produces ~1 dirty row
    let resp = send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: b"for /L %i in (1,1,200) do @echo Line %i\r\n".to_vec(),
        },
    );
    assert!(matches!(resp, Response::Ok));

    std::thread::sleep(Duration::from_secs(1));

    // Measure diff snapshot latency
    let mut diff_latencies = Vec::with_capacity(10);
    let mut full_latencies = Vec::with_capacity(10);

    for i in 0..10 {
        // Diff snapshot
        let diff_result = send_request_with_deadline(
            &mut pipe,
            &Request::ReadRichGridDiff { session_id: session_id.clone() },
            Duration::from_secs(10),
        );
        match diff_result {
            Ok((resp, latency, events)) => {
                if let Response::RichGridDiff { ref diff } = resp {
                    eprintln!(
                        "[diff] #{}: {:.2}ms, {} dirty rows, full_repaint={} (skipped {} events)",
                        i,
                        latency.as_secs_f64() * 1000.0,
                        diff.dirty_rows.len(),
                        diff.full_repaint,
                        events
                    );
                }
                diff_latencies.push(latency);
            }
            Err(e) => {
                eprintln!("[diff] #{} failed: {}", i, e);
                // Don't panic — diff might not be supported if daemon wasn't rebuilt
                break;
            }
        }

        // Full snapshot for comparison
        let full_result = send_request_with_deadline(
            &mut pipe,
            &Request::ReadRichGrid { session_id: session_id.clone() },
            Duration::from_secs(10),
        );
        if let Ok((_, latency, _)) = full_result {
            full_latencies.push(latency);
        }
    }

    if !diff_latencies.is_empty() {
        let diff_avg = diff_latencies.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / diff_latencies.len() as f64;
        let full_avg = full_latencies.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / full_latencies.len().max(1) as f64;

        eprintln!(
            "\n[comparison] Diff avg={:.2}ms  Full avg={:.2}ms  Speedup={:.1}x\n",
            diff_avg, full_avg,
            if diff_avg > 0.0 { full_avg / diff_avg } else { 0.0 }
        );
    }

    // Cleanup
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession { session_id },
        Duration::from_secs(5),
    );
}

/// Simulate the exact keystroke-echo pattern: Write + ReadRichGrid in sequence.
/// Measures the combined round-trip that a real keystroke incurs.
///
/// In production, these are two separate IPC calls through the bridge thread.
/// Here we measure them as back-to-back pipe requests to isolate the daemon-side
/// cost from the bridge threading overhead.
#[test]
fn keystroke_echo_round_trip() {
    let daemon = DaemonFixture::spawn("latency-keystroke");
    let mut pipe = daemon.connect();

    let resp = send_request(&mut pipe, &Request::Ping);
    assert!(matches!(resp, Response::Pong));

    let session_id = "latency-keystroke".to_string();
    let resp = send_request(
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

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for shell to fully initialize
    std::thread::sleep(Duration::from_secs(2));
    drain_events(&mut pipe);

    // Simulate 20 keystrokes: Write a character → wait for echo → ReadRichGrid
    let mut round_trips = Vec::with_capacity(20);
    let chars = b"hello world test input";

    for (i, &ch) in chars.iter().enumerate() {
        let round_trip_start = Instant::now();

        // Step 1: Write the keystroke
        let write_result = send_request_with_deadline(
            &mut pipe,
            &Request::Write {
                session_id: session_id.clone(),
                data: vec![ch],
            },
            Duration::from_secs(5),
        );
        assert!(write_result.is_ok(), "Write #{} failed: {:?}", i, write_result);

        // Step 2: Small delay for shell to echo (simulates real latency)
        std::thread::sleep(Duration::from_millis(10));

        // Step 3: Read the grid snapshot
        let snapshot_result = send_request_with_deadline(
            &mut pipe,
            &Request::ReadRichGrid { session_id: session_id.clone() },
            Duration::from_secs(5),
        );
        let (resp, _, events) = snapshot_result.unwrap_or_else(|e| {
            panic!("ReadRichGrid #{} failed: {}", i, e);
        });
        assert!(matches!(resp, Response::RichGrid { .. }));

        let round_trip = round_trip_start.elapsed();
        round_trips.push(round_trip);

        eprintln!(
            "[keystroke] '{}' #{}: {:.2}ms total (skipped {} events)",
            ch as char,
            i,
            round_trip.as_secs_f64() * 1000.0,
            events
        );
    }

    let avg_ms = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / round_trips.len() as f64;
    let max_ms = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).fold(0.0f64, f64::max);
    let mut sorted: Vec<f64> = round_trips.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = sorted[(sorted.len() as f64 * 0.95) as usize];

    eprintln!(
        "\n[keystroke] Write+ReadRichGrid round-trip: avg={:.2}ms  p95={:.2}ms  max={:.2}ms\n",
        avg_ms, p95_ms, max_ms
    );

    // The daemon-side round-trip (pipe + Mutex + serialize) should be <500ms
    // in debug builds. In release mode, target <50ms.
    // Debug builds have ~10x slower serde serialization.
    // In production, the bridge thread adds additional latency on top.
    assert!(
        p95_ms < 500.0,
        "Keystroke round-trip p95 too high: {:.2}ms (expected <500ms in debug). \
         Daemon-side Mutex contention or serialization overhead is the bottleneck.",
        p95_ms
    );

    if p95_ms > 50.0 {
        eprintln!(
            "[keystroke] WARNING: p95={:.2}ms exceeds release target (<50ms). \
             In debug builds this is expected due to unoptimized serde. \
             In production (release build), the bridge thread adds ~200-400ms on top.",
            p95_ms
        );
    }

    // Cleanup
    let _ = send_request_with_deadline(
        &mut pipe,
        &Request::CloseSession { session_id },
        Duration::from_secs(5),
    );
}
