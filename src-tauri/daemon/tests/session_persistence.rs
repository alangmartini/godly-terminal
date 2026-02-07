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
use std::os::windows::io::FromRawHandle;
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
use winapi::um::jobapi2::{CreateJobObjectW, SetInformationJobObject};
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

/// Wait for the daemon pipe to become available and verify it's responsive.
/// Sends a Ping request to confirm the daemon is fully initialized.
fn wait_for_daemon(timeout: Duration) -> std::fs::File {
    let start = Instant::now();
    loop {
        if let Some(mut file) = try_connect_pipe() {
            // Verify the daemon is responsive with a Ping
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                send_request(&mut file, &Request::Ping)
            })) {
                Ok(Response::Pong) => return file,
                Ok(other) => {
                    eprintln!("  [test] Unexpected ping response: {:?}, retrying...", other);
                }
                Err(_) => {
                    eprintln!("  [test] Ping failed (daemon not ready), retrying...");
                }
            }
            drop(file);
            thread::sleep(Duration::from_millis(200));
            continue;
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

/// Kill any running godly-daemon process and wait for full cleanup.
fn kill_existing_daemon() {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "godly-daemon.exe"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    // Wait for pipe to disappear
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if try_connect_pipe().is_none() {
            // Pipe is gone — wait a bit more for OS to fully reclaim pipe resources
            thread::sleep(Duration::from_millis(500));
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

/// Launch the daemon via WMI (Win32_Process.Create) using PowerShell CIM.
/// The process is created by the WMI provider host (wmiprvse.exe), NOT as a
/// child of the calling process, so it does not inherit any Job Object membership.
fn launch_daemon_via_wmi() {
    let daemon_path = daemon_binary_path();
    let daemon_str = daemon_path.to_string_lossy();

    let ps_command = format!(
        "$r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create \
         -Arguments @{{CommandLine='{}'}}; \
         if ($r.ReturnValue -ne 0) {{ throw \"WMI Create failed: $($r.ReturnValue)\" }}; \
         Write-Output \"PID=$($r.ProcessId)\"",
        daemon_str
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_command])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .expect("Failed to run PowerShell");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!("  [wmi] stdout: {}", stdout.trim());
    if !stderr.is_empty() {
        eprintln!("  [wmi] stderr: {}", stderr.trim());
    }
    assert!(
        output.status.success(),
        "WMI launch failed (exit: {}, stderr: {})",
        output.status,
        stderr
    );
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

/// Simulates the actual `cargo tauri dev` restart workflow:
///   1. Daemon A is running with sessions
///   2. Client disconnects (app window closed)
///   3. A SECOND daemon binary is launched (simulating `connect_or_launch`)
///   4. Does daemon B detect daemon A and exit? Or does it start and take over?
///
/// Verification is by SESSION EXISTENCE, not PID file (which is unreliable
/// because `taskkill /F` skips cleanup, leaving stale PID files).
///
/// Also checks whether the daemon binary can be overwritten, which tells us
/// if the daemon process is truly alive (on Windows, running .exe files are locked).
#[test]
fn test_03_second_daemon_detects_first() {
    let iterations = 5;
    let mut session_lost_count = 0;

    eprintln!(
        "\n=== test_03: second daemon detects running first ({} iterations) ===",
        iterations
    );

    for i in 0..iterations {
        eprintln!("--- Iteration {}/{} ---", i + 1, iterations);
        kill_existing_daemon();
        // Clean up stale PID file from previous taskkill
        if let Some(appdata) = std::env::var("APPDATA").ok() {
            let pid_path = std::path::PathBuf::from(&appdata)
                .join("com.godly.terminal")
                .join("godly-daemon.pid");
            let _ = std::fs::remove_file(&pid_path);
        }

        // Start daemon A
        let _child_a = launch_daemon_like_tauri_app();
        let mut pipe = wait_for_daemon(Duration::from_secs(5));
        eprintln!("  Daemon A running (PID file: {:?})", read_pid_file());

        // Create a uniquely-named session (our "marker" for daemon A)
        let session_id = format!("marker-session-iter{}-{}", i, std::process::id());
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

        // Disconnect (simulates app window close)
        // Do NOT call try_connect_pipe() for diagnostics — it consumes a pipe
        // instance and creates a race window.
        drop(pipe);
        thread::sleep(Duration::from_secs(2));

        // Check if daemon binary is locked (proves the process is alive)
        let daemon_path = daemon_binary_path();
        let binary_locked = std::fs::OpenOptions::new()
            .write(true)
            .open(&daemon_path)
            .is_err();
        eprintln!("  Daemon binary locked (process alive): {}", binary_locked);

        // Start daemon B (exactly as the Tauri app would via launch_daemon)
        eprintln!("  Starting daemon B...");
        let _child_b = launch_daemon_like_tauri_app();
        thread::sleep(Duration::from_secs(3));

        // Connect to whatever daemon is available and check for our marker session
        let mut pipe2 = wait_for_daemon(Duration::from_secs(5));
        let resp = send_request(&mut pipe2, &Request::ListSessions);
        match &resp {
            Response::SessionList { sessions } => {
                let found = sessions.iter().any(|s| s.id == session_id);
                let session_names: Vec<&str> =
                    sessions.iter().map(|s| s.id.as_str()).collect();
                if found {
                    eprintln!(
                        "  OK: Marker session '{}' found — daemon A is still serving",
                        session_id
                    );
                } else {
                    eprintln!(
                        "  LOST: Marker session '{}' missing! Sessions: {:?}",
                        session_id, session_names
                    );
                    eprintln!(
                        "  PID file now: {:?} (daemon B took over, sessions lost)",
                        read_pid_file()
                    );
                    session_lost_count += 1;
                }
            }
            other => panic!("Expected SessionList, got {:?}", other),
        }

        // Clean up session
        let _ = send_request(
            &mut pipe2,
            &Request::CloseSession {
                session_id: session_id.clone(),
            },
        );
        drop(pipe2);
        kill_existing_daemon();
    }

    assert_eq!(
        session_lost_count, 0,
        "\n\nBUG: Sessions were lost in {}/{} iterations.\n\
         A second daemon launched and took over while the first was still running.\n\
         This causes session loss when `cargo tauri dev` is restarted.\n",
        session_lost_count, iterations
    );
}

/// FIX VERIFICATION: Daemon launched via WMI survives Job Object closure.
///
/// This tests the fix for test_02: instead of spawning the daemon directly
/// (which inherits the Job Object), we use WMI's Win32_Process.Create(),
/// which runs from the WMI service (wmiprvse.exe) — outside any Job Object.
///
/// Steps:
///   1. Create a KILL_ON_JOB_CLOSE Job Object
///   2. Launch the daemon via WMI (NOT as a child of this process)
///   3. Create a session to prove the daemon is working
///   4. Close the Job Object handle (this would kill job members)
///   5. Verify the daemon and session survived
///
/// This should PASS — proving WMI launch is the correct escape mechanism.
#[test]
fn test_04_wmi_launch_escapes_job_object() {
    let iterations = 3;
    let mut killed_count = 0;

    eprintln!(
        "\n=== test_04: WMI-launched daemon vs Job Object ({} iterations) ===",
        iterations
    );

    for i in 0..iterations {
        eprintln!("--- Iteration {}/{} ---", i + 1, iterations);
        kill_existing_daemon();

        // Create a Job Object with KILL_ON_JOB_CLOSE — same as cargo/tauri-cli
        let job = create_job_object(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE);

        // Launch daemon via WMI — this is the FIX being tested.
        // The daemon is created by wmiprvse.exe, not by us, so it's NOT in our job.
        eprintln!("  Launching daemon via WMI...");
        launch_daemon_via_wmi();

        // Wait for daemon to start and create a session
        let mut pipe = wait_for_daemon(Duration::from_secs(10));
        let pid = read_pid_file().expect("PID file should exist");
        eprintln!("  Daemon started with PID {}", pid);

        let session_id = format!("wmi-test-session-{}", i);
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

        // Disconnect client
        drop(pipe);
        thread::sleep(Duration::from_millis(500));

        // Close the Job Object handle — this kills all processes IN the job.
        // The WMI-launched daemon should NOT be in the job.
        eprintln!("  Closing Job Object handle...");
        unsafe {
            CloseHandle(job);
        }
        thread::sleep(Duration::from_secs(2));

        // Try to reconnect and verify the session is still alive
        match try_connect_pipe() {
            Some(mut pipe2) => {
                let resp = send_request(&mut pipe2, &Request::ListSessions);
                match &resp {
                    Response::SessionList { sessions } => {
                        let found = sessions.iter().any(|s| s.id == session_id);
                        if found {
                            eprintln!(
                                "  OK: Daemon survived, session '{}' intact (PID {})",
                                session_id, pid
                            );
                        } else {
                            eprintln!(
                                "  PARTIAL: Daemon alive but session '{}' missing!",
                                session_id
                            );
                            killed_count += 1;
                        }
                    }
                    other => {
                        eprintln!("  ERROR: Unexpected response: {:?}", other);
                        killed_count += 1;
                    }
                }
                let _ = send_request(
                    &mut pipe2,
                    &Request::CloseSession {
                        session_id: session_id.clone(),
                    },
                );
            }
            None => {
                eprintln!("  KILLED: Daemon did not survive Job Object closure (PID {})", pid);
                killed_count += 1;
            }
        }

        kill_existing_daemon();
    }

    assert_eq!(
        killed_count, 0,
        "\n\nFIX FAILED: WMI-launched daemon was killed/broken in {}/{} iterations.\n\
         The WMI escape mechanism did not work as expected.\n",
        killed_count, iterations
    );
}
