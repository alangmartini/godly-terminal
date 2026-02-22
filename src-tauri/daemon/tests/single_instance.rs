//! Tests for single-instance daemon enforcement via named mutex.
//!
//! The daemon uses `DaemonLock::try_acquire()` (a Windows named mutex) to ensure
//! only one instance runs per pipe name. The mutex is atomically created by the
//! kernel, eliminating the TOCTOU race that existed with the old pipe-based check.
//!
//! These tests verify:
//! 1. The named mutex blocks a second daemon from starting
//! 2. Concurrent launches produce exactly one running daemon
//! 3. After the lock holder exits, a new daemon can start
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test single_instance -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::process::{Child, Command};
use std::ptr;
use std::thread;
use std::time::Duration;

use godly_protocol::frame;
use godly_protocol::types::ShellType;
use godly_protocol::{DaemonMessage, Request, Response};

use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the daemon binary next to the test binary.
fn daemon_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let deps_dir = exe.parent().unwrap();
    let debug_dir = deps_dir.parent().unwrap();
    let path = debug_dir.join("godly-daemon.exe");
    assert!(
        path.exists(),
        "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
        path
    );
    path
}

/// Spawn a daemon process targeting a specific pipe name.
fn spawn_daemon(pipe_name: &str) -> Child {
    let daemon_path = daemon_binary_path();
    Command::new(&daemon_path)
        .env("GODLY_PIPE_NAME", pipe_name)
        .env("GODLY_NO_DETACH", "1")
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("Failed to spawn daemon")
}

/// Try to open a client connection to a named pipe.
fn try_connect_pipe(pipe_name: &str) -> Option<std::fs::File> {
    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            wide_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        None
    } else {
        Some(unsafe { std::fs::File::from_raw_handle(handle as _) })
    }
}

/// Wait for a pipe to become available, verifying with a Ping.
fn wait_for_pipe(pipe_name: &str, timeout: Duration) -> std::fs::File {
    let start = std::time::Instant::now();
    loop {
        if let Some(mut file) = try_connect_pipe(pipe_name) {
            if let Ok(Response::Pong) = std::panic::catch_unwind(
                std::panic::AssertUnwindSafe(|| send_request(&mut file, &Request::Ping)),
            ) {
                return file;
            }
            drop(file);
        }
        if start.elapsed() > timeout {
            panic!(
                "Pipe '{}' did not become available within {:?}",
                pipe_name, timeout
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Send a request and read the response, skipping async Event messages.
fn send_request(pipe: &mut std::fs::File, req: &Request) -> Response {
    frame::write_request(pipe, req).expect("Failed to write request");
    loop {
        let msg: DaemonMessage = frame::read_daemon_message(pipe)
            .expect("Failed to read message")
            .expect("Unexpected EOF on pipe");
        match msg {
            DaemonMessage::Response(r) => return r,
            DaemonMessage::Event(_) => continue,
        }
    }
}

/// Kill all remaining child processes and wait for cleanup.
fn kill_children(children: &mut [Child]) {
    for child in children.iter_mut() {
        let _ = child.kill();
    }
    for child in children.iter_mut() {
        let _ = child.wait();
    }
}

/// Wait for a pipe to fully disappear.
fn wait_for_pipe_gone(pipe_name: &str, timeout: Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if try_connect_pipe(pipe_name).is_none() {
            thread::sleep(Duration::from_millis(200));
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Named mutex prevents a second daemon from starting on the same pipe.
///
/// The DaemonLock uses CreateMutexW with a name derived from the pipe name.
/// A second daemon calling DaemonLock::try_acquire() gets ERROR_ALREADY_EXISTS
/// and exits immediately.
///
/// Bug fix verification: previously, is_daemon_running() checked the pipe,
/// which has a TOCTOU race. The named mutex is atomic and race-free.
#[test]
#[ntest::timeout(60_000)] // 1min — spawns multiple daemons + mutex contention
fn test_named_mutex_blocks_second_daemon() {
    // Fix: DaemonLock (named mutex) prevents multiple daemon instances
    let pipe_name = format!(
        r"\\.\pipe\godly-test-mutex-block-{}",
        std::process::id()
    );

    // Start daemon A and verify it's running
    let mut daemon_a = spawn_daemon(&pipe_name);
    let mut pipe = wait_for_pipe(&pipe_name, Duration::from_secs(5));
    assert!(
        matches!(send_request(&mut pipe, &Request::Ping), Response::Pong),
        "Daemon A failed to respond to Ping"
    );
    drop(pipe);

    // Start daemon B on the same pipe — it should detect the mutex and exit
    let mut daemon_b = spawn_daemon(&pipe_name);
    thread::sleep(Duration::from_secs(3));

    let b_exited = daemon_b
        .try_wait()
        .ok()
        .map_or(false, |status| status.is_some());

    // Cleanup
    let _ = daemon_b.kill();
    let _ = daemon_b.wait();
    let _ = daemon_a.kill();
    let _ = daemon_a.wait();

    assert!(
        b_exited,
        "Daemon B should have exited after detecting daemon A's mutex lock, \
         but it's still running. The named mutex singleton enforcement is broken."
    );
}

/// Concurrent daemon launches produce exactly one running instance.
///
/// When N daemon processes start simultaneously, the named mutex ensures
/// exactly one acquires the lock. The rest detect ERROR_ALREADY_EXISTS and
/// exit with code 0.
///
/// Bug fix verification: with the old pipe-based check, concurrent launches
/// could all pass the TOCTOU window and coexist. The named mutex is atomic.
#[test]
#[ntest::timeout(60_000)]
fn test_concurrent_launch_single_instance() {
    // Fix: named mutex ensures exactly 1 daemon from concurrent launches
    let pipe_name = format!(
        r"\\.\pipe\godly-test-concurrent-{}",
        std::process::id()
    );

    let n = 10;
    let iterations = 3;
    let mut max_running: usize = 0;

    for _ in 0..iterations {
        // Launch N daemons at nearly the same instant
        let mut children: Vec<Child> = (0..n).map(|_| spawn_daemon(&pipe_name)).collect();
        thread::sleep(Duration::from_secs(4));

        let mut running_count: usize = 0;
        for child in children.iter_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {} // exited — good
                Ok(None) => running_count += 1,
                Err(_) => {}
            }
        }

        if running_count > max_running {
            max_running = running_count;
        }

        kill_children(&mut children);
        wait_for_pipe_gone(&pipe_name, Duration::from_secs(3));
    }

    assert_eq!(
        max_running, 1,
        "Expected exactly 1 daemon from {} concurrent launches, but {} were running. \
         The named mutex should prevent multiple instances atomically.",
        n, max_running
    );
}

/// After the lock holder exits, a new daemon can start and serve sessions.
///
/// The named mutex is automatically released when the process exits (even on
/// crash), so there are no stale locks. A new daemon should be able to acquire
/// the lock and become the singleton.
#[test]
#[ntest::timeout(60_000)]
fn test_new_daemon_starts_after_lock_holder_exits() {
    // Fix: mutex is auto-released on exit, no stale locks
    let pipe_name = format!(
        r"\\.\pipe\godly-test-reacquire-{}",
        std::process::id()
    );

    // Start daemon A
    let mut daemon_a = spawn_daemon(&pipe_name);
    let mut pipe = wait_for_pipe(&pipe_name, Duration::from_secs(5));
    assert!(matches!(send_request(&mut pipe, &Request::Ping), Response::Pong));

    // Create a session on daemon A
    let session_id = format!("reacquire-test-{}", std::process::id());
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
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
    drop(pipe);

    // Kill daemon A (simulates crash)
    let _ = daemon_a.kill();
    let _ = daemon_a.wait();
    wait_for_pipe_gone(&pipe_name, Duration::from_secs(10));

    // Start daemon B — should acquire the lock since A released it on exit
    let mut daemon_b = spawn_daemon(&pipe_name);
    let mut pipe_b = wait_for_pipe(&pipe_name, Duration::from_secs(15));
    assert!(
        matches!(send_request(&mut pipe_b, &Request::Ping), Response::Pong),
        "Daemon B should be responsive after daemon A exited"
    );

    // Daemon B should have an empty session store (A's sessions are gone)
    let resp = send_request(&mut pipe_b, &Request::ListSessions);
    match &resp {
        Response::SessionList { sessions } => {
            assert!(
                !sessions.iter().any(|s| s.id == session_id),
                "Session from daemon A should not exist in daemon B"
            );
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }

    // Cleanup
    drop(pipe_b);
    let _ = daemon_b.kill();
    let _ = daemon_b.wait();
}

/// Sessions persist when only one daemon runs (no session isolation bug).
///
/// With the mutex fix, there is always exactly one daemon, so all client
/// connections go to the same daemon. Sessions created by one client are
/// always visible to subsequent connections.
#[test]
#[ntest::timeout(60_000)]
fn test_sessions_visible_across_reconnect_with_single_daemon() {
    // Fix: single daemon means no session isolation
    let pipe_name = format!(
        r"\\.\pipe\godly-test-reconnect-{}",
        std::process::id()
    );

    let mut daemon = spawn_daemon(&pipe_name);
    let session_id = format!("reconnect-test-{}", std::process::id());

    // Create session via first connection
    {
        let mut pipe = wait_for_pipe(&pipe_name, Duration::from_secs(5));
        let resp = send_request(
            &mut pipe,
            &Request::CreateSession {
                id: session_id.clone(),
                shell_type: ShellType::Windows,
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
    }

    // Reconnect multiple times — session should ALWAYS be visible
    thread::sleep(Duration::from_millis(500));
    let mut not_found_count = 0;
    let reconnect_attempts = 10;

    for _ in 0..reconnect_attempts {
        if let Some(mut pipe) = try_connect_pipe(&pipe_name) {
            if let Ok(resp) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                send_request(&mut pipe, &Request::ListSessions)
            })) {
                if let Response::SessionList { sessions } = resp {
                    if !sessions.iter().any(|s| s.id == session_id) {
                        not_found_count += 1;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    // Cleanup
    let _ = daemon.kill();
    let _ = daemon.wait();

    assert_eq!(
        not_found_count, 0,
        "Session '{}' was missing in {}/{} reconnect attempts. \
         With a single daemon (enforced by named mutex), sessions should \
         always be visible across reconnections.",
        session_id, not_found_count, reconnect_attempts
    );
}
