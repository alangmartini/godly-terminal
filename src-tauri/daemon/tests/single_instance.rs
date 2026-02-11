//! Tests for single-instance daemon enforcement.
//!
//! Bug: Multiple godly_daemon.exe processes can run simultaneously when launched
//! concurrently, causing terminal freezes and session isolation. The
//! is_daemon_running() check has a TOCTOU race — it checks the named pipe
//! before any daemon has created one. With PIPE_UNLIMITED_INSTANCES, all
//! launched instances successfully create pipe instances and run in parallel.
//!
//! Symptoms:
//! - Multiple godly_daemon.exe visible in Task Manager
//! - Sessions split across daemons (invisible to each other)
//! - Terminal freeze when client reconnects to a different daemon instance
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test single_instance -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use godly_protocol::frame;
use godly_protocol::types::ShellType;
use godly_protocol::{DaemonMessage, Request, Response};

use winapi::um::errhandlingapi::GetLastError;
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
/// Uses GODLY_NO_DETACH=1 to keep it as a child process for tracking.
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

/// Try to open a connection to the daemon's named pipe.
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
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
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
    let start = Instant::now();
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
    frame::write_message(pipe, req).expect("Failed to write request");
    loop {
        let msg: DaemonMessage = frame::read_message(pipe)
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug: Concurrent daemon launches produce multiple running instances.
///
/// The is_daemon_running() check has a TOCTOU race: it tries to connect to the
/// named pipe, but there's a window between the check and CreateNamedPipeW in
/// server.run(). When N daemons start simultaneously, all pass the check
/// (pipe doesn't exist yet) and all create pipe instances successfully
/// (PIPE_UNLIMITED_INSTANCES allows this).
///
/// Expected: exactly 1 daemon survives, the rest exit with code 0.
/// Actual (buggy): multiple daemons survive and run in parallel.
#[test]
fn test_concurrent_launch_single_instance() {
    // Bug: is_daemon_running() has TOCTOU race allowing multiple daemon instances
    let pipe_name = format!(
        r"\\.\pipe\godly-test-single-instance-{}",
        std::process::id()
    );

    let n = 10; // Launch 10 daemons simultaneously to maximize race hit probability
    let iterations = 3;
    let mut max_running = 0;

    for _iteration in 0..iterations {
        // Launch N daemons at nearly the same instant (tight loop)
        let mut children: Vec<Child> = (0..n).map(|_| spawn_daemon(&pipe_name)).collect();

        // Wait for daemons to start — losers should detect the winner and exit(0)
        thread::sleep(Duration::from_secs(4));

        // Count how many are still running vs exited
        let mut running_count = 0;
        let mut exited_count = 0;
        for child in children.iter_mut() {
            match child.try_wait() {
                Ok(Some(_status)) => exited_count += 1,
                Ok(None) => running_count += 1,
                Err(_) => {}
            }
        }

        if running_count > max_running {
            max_running = running_count;
        }

        // Cleanup
        kill_children(&mut children);
        // Wait for pipe to disappear before next iteration
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            if try_connect_pipe(&pipe_name).is_none() {
                thread::sleep(Duration::from_millis(200));
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    // Single-instance enforcement must ensure only 1 daemon survives.
    // With the TOCTOU bug, multiple daemons run in parallel.
    assert_eq!(
        max_running, 1,
        "Bug: Up to {} daemon processes ran simultaneously (expected 1). \
         The is_daemon_running() TOCTOU race allows concurrent launches to all \
         pass the check before any creates a pipe instance. \
         PIPE_UNLIMITED_INSTANCES then lets all create valid pipe instances. \
         This causes multiple godly_daemon.exe in Task Manager and terminal freezes.",
        max_running
    );
}

/// Bug: Multiple daemons on the same pipe cause session loss on reconnect.
///
/// When two daemons run on the same pipe (due to the TOCTOU race above),
/// each daemon has its own isolated session store. A session created through
/// one daemon's pipe instance is invisible through the other's. Client
/// reconnections (CreateFileW) can land on either daemon, making sessions
/// randomly appear and disappear — the user sees a "frozen terminal."
///
/// This test launches two daemons and verifies that a session created through
/// one connection is always visible through subsequent connections.
///
/// Expected: session always found after reconnect.
/// Actual (buggy): session disappears when reconnect lands on different daemon.
#[test]
fn test_multiple_daemons_cause_session_loss_on_reconnect() {
    // Bug: sessions invisible when client reconnects to a different daemon instance
    let pipe_name = format!(
        r"\\.\pipe\godly-test-session-loss-{}",
        std::process::id()
    );

    // Launch two daemons at the same time to hit the race
    let mut daemon_a = spawn_daemon(&pipe_name);
    let mut daemon_b = spawn_daemon(&pipe_name);

    // Wait for both to initialize
    thread::sleep(Duration::from_secs(3));

    // Check if both are running (needed for this test to be meaningful)
    let a_running = daemon_a
        .try_wait()
        .ok()
        .map_or(false, |status| status.is_none());
    let b_running = daemon_b
        .try_wait()
        .ok()
        .map_or(false, |status| status.is_none());

    if !a_running || !b_running {
        // Single-instance worked for this attempt — one daemon exited.
        // The concurrent launch test covers this failure mode.
        // Try again with more daemons to increase race probability.
        let _ = daemon_a.kill();
        let _ = daemon_a.wait();
        let _ = daemon_b.kill();
        let _ = daemon_b.wait();
        // Wait for pipe cleanup
        thread::sleep(Duration::from_millis(500));

        // Retry with 10 daemons launched simultaneously
        let mut children: Vec<Child> = (0..10).map(|_| spawn_daemon(&pipe_name)).collect();
        thread::sleep(Duration::from_secs(4));

        let running: Vec<usize> = children
            .iter_mut()
            .enumerate()
            .filter_map(|(i, c)| match c.try_wait() {
                Ok(Some(_)) => None,
                _ => Some(i),
            })
            .collect();

        if running.len() < 2 {
            // Race not hit even with 10 — clean up and report
            kill_children(&mut children);
            panic!(
                "Could not reproduce multi-daemon condition (only {} running out of 10). \
                 The TOCTOU race window may be too narrow on this machine. \
                 The concurrent launch test should catch this more reliably.",
                running.len()
            );
        }

        // Use first two running daemons for the session test
        // (we'll just continue with the children array)
        let session_id = format!("session-loss-retry-{}", std::process::id());
        let mut not_found_count = 0;

        // Create a session through one connection
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
        thread::sleep(Duration::from_millis(300));

        // Reconnect multiple times — some should hit a different daemon
        for _ in 0..20 {
            if let Some(mut pipe) = try_connect_pipe(&pipe_name) {
                if let Ok(resp) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        send_request(&mut pipe, &Request::ListSessions)
                    }))
                {
                    if let Response::SessionList { sessions } = resp {
                        if !sessions.iter().any(|s| s.id == session_id) {
                            not_found_count += 1;
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        }

        kill_children(&mut children);

        assert_eq!(
            not_found_count, 0,
            "Bug: Session '{}' was missing in {}/20 reconnect attempts. \
             Multiple daemons on the same pipe have isolated session stores. \
             Client reconnections randomly land on different daemons, making \
             sessions appear/disappear — the user sees a frozen terminal.",
            session_id, not_found_count
        );
        return;
    }

    // Both daemons are running — proceed with session loss test
    let session_id = format!("session-loss-test-{}", std::process::id());

    // Connect to whichever daemon picks up our connection and create a session
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
        // Disconnect
    }
    thread::sleep(Duration::from_millis(300));

    // Reconnect multiple times — at least some should hit the "other" daemon
    // where the session doesn't exist
    let mut found_count = 0;
    let mut not_found_count = 0;
    let reconnect_attempts = 20;

    for _ in 0..reconnect_attempts {
        if let Some(mut pipe) = try_connect_pipe(&pipe_name) {
            if let Ok(resp) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                send_request(&mut pipe, &Request::ListSessions)
            })) {
                match resp {
                    Response::SessionList { sessions } => {
                        if sessions.iter().any(|s| s.id == session_id) {
                            found_count += 1;
                        } else {
                            not_found_count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Cleanup
    let _ = daemon_a.kill();
    let _ = daemon_a.wait();
    let _ = daemon_b.kill();
    let _ = daemon_b.wait();

    // With a single daemon, every reconnection should find the session.
    // With multiple daemons, some reconnections land on the daemon without the session.
    assert_eq!(
        not_found_count, 0,
        "Bug: Session '{}' was missing in {}/{} reconnect attempts ({} found). \
         Multiple daemons on the same pipe have isolated session stores. \
         Client reconnections randomly land on different daemons, making \
         sessions appear/disappear — the user sees a frozen terminal.",
        session_id, not_found_count, reconnect_attempts, found_count
    );
}
