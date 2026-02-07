//! Integration tests for daemon session persistence.
//!
//! These tests spawn the actual godly-daemon binary and verify that sessions
//! persist across client reconnections. They also reproduce the bug where the
//! daemon is killed by Windows Job Objects (as happens with `cargo tauri dev`).
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test session_persistence -- --test-threads=1 --nocapture
//!
//! The tests MUST run serially (--test-threads=1) because they share a single
//! named pipe endpoint and kill/restart the daemon between tests.

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use godly_protocol::frame;
use godly_protocol::types::ShellType;
use godly_protocol::{DaemonMessage, Request, Response, PIPE_NAME};

use winapi::um::errhandlingapi::GetLastError;
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::jobapi2::{AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject};
use winapi::um::winnt::{
    JobObjectExtendedLimitInformation, FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ,
    GENERIC_WRITE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// Type alias matching winapi's HANDLE (*mut winapi::ctypes::c_void)
type HANDLE = *mut winapi::ctypes::c_void;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the daemon binary next to the test binary.
/// Integration test binaries live in target/debug/deps/, daemon binary is in target/debug/.
fn daemon_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let deps_dir = exe.parent().unwrap(); // target/debug/deps
    let debug_dir = deps_dir.parent().unwrap(); // target/debug
    let path = debug_dir.join("godly-daemon.exe");
    assert!(
        path.exists(),
        "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
        path
    );
    path
}

/// Try to open a connection to the daemon's named pipe.
/// Returns the File handle if successful, None if the pipe doesn't exist.
fn try_connect_pipe() -> Option<std::fs::File> {
    let pipe_name: Vec<u16> = OsStr::new(PIPE_NAME)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            pipe_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        let err = unsafe { GetLastError() };
        eprintln!("  [test] CreateFileW failed with error: {}", err);
        None
    } else {
        Some(unsafe { std::fs::File::from_raw_handle(handle as _) })
    }
}

/// Wait for the daemon pipe to become available, with timeout.
fn wait_for_daemon(timeout: Duration) -> std::fs::File {
    let start = Instant::now();
    loop {
        if let Some(file) = try_connect_pipe() {
            return file;
        }
        if start.elapsed() > timeout {
            panic!(
                "Daemon did not start within {:?} — pipe never became available",
                timeout
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Send a request on the pipe and wait for the response.
/// Skips any async Event messages that arrive before the response.
fn send_request(pipe: &mut std::fs::File, req: &Request) -> Response {
    frame::write_message(pipe, req).expect("Failed to write request to pipe");
    loop {
        let msg: DaemonMessage = frame::read_message(pipe)
            .expect("Failed to read message from pipe")
            .expect("Unexpected EOF on pipe");
        match msg {
            DaemonMessage::Response(r) => return r,
            DaemonMessage::Event(_) => continue,
        }
    }
}

/// Kill any running godly-daemon process.
fn kill_existing_daemon() {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "godly-daemon.exe"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    // Wait for pipe to disappear
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        if try_connect_pipe().is_none() {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Read the daemon PID file from %APPDATA%.
fn read_pid_file() -> Option<u32> {
    let appdata = std::env::var("APPDATA").ok()?;
    let path = std::path::PathBuf::from(appdata)
        .join("com.godly.terminal")
        .join("godly-daemon.pid");
    let content = std::fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

/// Launch the daemon with the same flags the Tauri app uses (no breakaway).
fn launch_daemon_like_tauri_app() -> std::process::Child {
    let daemon_path = daemon_binary_path();
    Command::new(&daemon_path)
        .creation_flags(
            0x00000008 | // DETACHED_PROCESS
            0x00000200, // CREATE_NEW_PROCESS_GROUP
        )
        .spawn()
        .expect("Failed to spawn daemon")
}

/// Create a Windows Job Object with the given limit flags.
fn create_job_object(limit_flags: u32) -> HANDLE {
    unsafe {
        let job = CreateJobObjectW(ptr::null_mut(), ptr::null());
        assert!(!job.is_null(), "CreateJobObjectW failed: {}", GetLastError());

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = limit_flags;
        let result = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *mut _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        assert!(
            result != 0,
            "SetInformationJobObject failed: {}",
            GetLastError()
        );

        job
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Baseline test: sessions persist when a client disconnects and reconnects
/// to the same daemon (no Job Objects involved).
///
/// This should PASS — it proves the basic daemon reconnection works.
#[test]
fn test_01_sessions_persist_across_client_reconnect() {
    eprintln!("\n=== test_01: basic session persistence across reconnect ===");
    kill_existing_daemon();

    let _child = launch_daemon_like_tauri_app();
    let mut pipe = wait_for_daemon(Duration::from_secs(5));
    let pid1 = read_pid_file().expect("PID file should exist after daemon start");

    // Create a session
    let session_id = "reconnect-test-session".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    // Verify session shows up
    let resp = send_request(&mut pipe, &Request::ListSessions);
    match &resp {
        Response::SessionList { sessions } => {
            assert!(
                sessions.iter().any(|s| s.id == session_id),
                "Session not found in list: {:?}",
                sessions
            );
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }

    // Disconnect (simulates app window close)
    drop(pipe);
    thread::sleep(Duration::from_secs(1));

    // Reconnect
    let mut pipe2 = wait_for_daemon(Duration::from_secs(5));
    let pid2 = read_pid_file().expect("PID file should still exist");

    // Same daemon — PID should not have changed
    assert_eq!(
        pid1, pid2,
        "Daemon PID changed from {} to {}! A new daemon was spawned instead of reusing the existing one.",
        pid1, pid2
    );

    // Session should still exist
    let resp = send_request(&mut pipe2, &Request::ListSessions);
    match &resp {
        Response::SessionList { sessions } => {
            assert!(
                sessions.iter().any(|s| s.id == session_id),
                "Session '{}' lost after reconnect! Sessions: {:?}",
                session_id,
                sessions
            );
            eprintln!(
                "  OK: Session '{}' still present after reconnect (PID {})",
                session_id, pid2
            );
        }
        other => panic!("Expected SessionList, got {:?}", other),
    }

    // Cleanup
    let _ = send_request(
        &mut pipe2,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
    drop(pipe2);
    kill_existing_daemon();
}

/// BUG REPRODUCTION: The daemon is killed when a Windows Job Object with
/// JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE is closed.
///
/// This reproduces what happens with `cargo tauri dev`:
///   1. cargo/tauri-cli create a Job Object for their process tree
///   2. The Tauri app spawns the daemon (which inherits the job)
///   3. User closes the Tauri window -> app exits
///   4. cargo/tauri-cli exit -> Job Object is closed -> daemon is KILLED
///   5. Next `cargo tauri dev` -> no daemon found -> new daemon starts
///   6. Old sessions are lost
///
/// The test assigns the daemon to a KILL_ON_JOB_CLOSE Job Object (simulating
/// job inheritance from the cargo process tree), then closes the job handle
/// (simulating cargo/tauri-cli exiting). The daemon should survive, but
/// currently it doesn't.
///
/// This test should FAIL until the daemon can escape Job Objects.
#[test]
fn test_02_daemon_survives_job_object_closure() {
    let iterations = 3;
    let mut killed_count = 0;

    eprintln!(
        "\n=== test_02: daemon vs Job Object ({} iterations) ===",
        iterations
    );

    for i in 0..iterations {
        eprintln!("--- Iteration {}/{} ---", i + 1, iterations);
        kill_existing_daemon();

        // Create a Job Object with KILL_ON_JOB_CLOSE (simulates cargo/tauri-cli)
        let job = create_job_object(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE);

        // Launch daemon with the same flags the Tauri app uses
        let child = launch_daemon_like_tauri_app();

        // Assign daemon to the Job Object (simulates job inheritance from parent).
        // In the real scenario, the daemon inherits the job automatically because
        // it's spawned by a process that's already in the job. Here we assign
        // explicitly after spawn to achieve the same effect without putting the
        // test process itself in the job.
        unsafe {
            let result = AssignProcessToJobObject(job, child.as_raw_handle() as _);
            assert!(
                result != 0,
                "AssignProcessToJobObject failed: {}",
                GetLastError()
            );
        }

        // Wait for daemon to start
        let mut pipe = wait_for_daemon(Duration::from_secs(5));
        let pid = read_pid_file().expect("PID file should exist");
        eprintln!("  Daemon started with PID {}", pid);

        // Create a session with a running process
        let session_id = format!("job-test-session-{}", i);
        let resp = send_request(
            &mut pipe,
            &Request::CreateSession {
                id: session_id.clone(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
            },
        );
        assert!(
            matches!(resp, Response::SessionCreated { .. }),
            "CreateSession failed: {:?}",
            resp
        );

        // Disconnect client (simulates user closing the app window)
        drop(pipe);
        thread::sleep(Duration::from_millis(500));

        // Close the Job Object handle — simulates cargo/tauri-cli exiting.
        // With JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, this kills all processes
        // in the job (including the daemon).
        eprintln!("  Closing Job Object handle (simulating cargo exit)...");
        unsafe {
            CloseHandle(job);
        }
        thread::sleep(Duration::from_secs(2));

        // Try to reconnect to the daemon
        let survived = try_connect_pipe().is_some();
        if survived {
            eprintln!("  Daemon SURVIVED Job Object closure (PID {})", pid);
        } else {
            eprintln!("  Daemon KILLED by Job Object closure (PID {})", pid);
            killed_count += 1;
        }

        // Clean up for next iteration
        kill_existing_daemon();
    }

    // The daemon SHOULD survive — if it doesn't, the bug is reproduced
    assert_eq!(
        killed_count, 0,
        "\n\nBUG REPRODUCED: Daemon was killed by Job Object closure in {}/{} iterations.\n\
         When `cargo tauri dev` exits, it closes its Job Object, killing the daemon.\n\
         All sessions are lost because the daemon doesn't escape the Job Object.\n\
         \n\
         Root cause: the daemon is spawned with DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,\n\
         but these flags do NOT remove the process from its parent's Job Object.\n\
         CREATE_BREAKAWAY_FROM_JOB would fix this, but it requires the Job Object to\n\
         have JOB_OBJECT_LIMIT_BREAKAWAY_OK set — cargo's job does not allow this.\n\
         \n\
         Fix: launch the daemon via an intermediate process that is NOT in the job,\n\
         or use a Windows Service, or use a scheduled task to start the daemon.\n",
        killed_count, iterations
    );
}
