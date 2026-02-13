//! Integration tests: Ctrl+C (\x03) must interrupt running processes in PTY sessions.
//!
//! Bug: When running `npm run dev` (Vite) or any long-running process in Godly
//! Terminal, pressing Ctrl+C does nothing. Ctrl+Z shows ^Z in the terminal,
//! proving other control characters reach the PTY — but Ctrl+C fails to send
//! SIGINT/CTRL_C_EVENT to interrupt the running process.
//!
//! These tests verify that writing \x03 to a daemon PTY session actually
//! interrupts the running process, which is the expected behavior on Windows
//! when using ConPTY.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test ctrl_c_interrupt -- --test-threads=1
//!
//! Each test spawns its own daemon with an isolated pipe name, so they do NOT
//! interfere with a running production daemon.

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use godly_protocol::frame;
use godly_protocol::types::ShellType;
use godly_protocol::{DaemonMessage, Event, Request, Response};

use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::namedpipeapi::PeekNamedPipe;
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a unique pipe name for this test to avoid collisions with
/// the production daemon or other test runs.
fn test_pipe_name(test: &str) -> String {
    format!(
        r"\\.\pipe\godly-test-{}-{}",
        test,
        std::process::id()
    )
}

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

fn try_connect_pipe(pipe_name: &str) -> Option<std::fs::File> {
    let wide: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
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

/// Non-blocking check for data on the pipe.
fn pipe_has_data(pipe: &std::fs::File) -> bool {
    let handle = pipe.as_raw_handle();
    let mut bytes_available: u32 = 0;
    let result = unsafe {
        PeekNamedPipe(
            handle as *mut _,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut bytes_available,
            ptr::null_mut(),
        )
    };
    result != 0 && bytes_available > 0
}

/// Read a single DaemonMessage from the pipe (blocking).
fn read_message(pipe: &mut std::fs::File) -> Option<DaemonMessage> {
    frame::read_message(pipe).ok().flatten()
}

/// Send a request and wait for its Response, collecting any Output events into
/// `output` along the way.
fn send_request_collecting_output(
    pipe: &mut std::fs::File,
    req: &Request,
    session_id: &str,
    output: &mut String,
) -> Response {
    frame::write_message(pipe, req).expect("Failed to write request to pipe");
    loop {
        match read_message(pipe) {
            Some(DaemonMessage::Response(r)) => return r,
            Some(DaemonMessage::Event(Event::Output {
                session_id: sid,
                data,
            })) if sid == session_id => {
                output.push_str(&String::from_utf8_lossy(&data));
            }
            Some(DaemonMessage::Event(_)) => continue,
            None => panic!("Pipe closed while waiting for response"),
        }
    }
}

/// Collect terminal output events for up to `timeout`, using non-blocking peeks
/// so we don't block forever when no more data arrives.
fn collect_output(
    pipe: &mut std::fs::File,
    session_id: &str,
    timeout: Duration,
) -> String {
    let mut output = String::new();
    let start = Instant::now();

    while start.elapsed() < timeout {
        if pipe_has_data(pipe) {
            match read_message(pipe) {
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Some(_) => {} // Skip other events/responses
                None => break,
            }
        } else {
            thread::sleep(Duration::from_millis(50));
        }
    }
    output
}

/// Collect output until a predicate matches or timeout expires.
/// Returns (all_collected_output, predicate_matched).
fn collect_output_until(
    pipe: &mut std::fs::File,
    session_id: &str,
    timeout: Duration,
    predicate: impl Fn(&str) -> bool,
) -> (String, bool) {
    let mut output = String::new();
    let start = Instant::now();

    while start.elapsed() < timeout {
        if pipe_has_data(pipe) {
            match read_message(pipe) {
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                    if predicate(&output) {
                        return (output, true);
                    }
                }
                Some(_) => {}
                None => break,
            }
        } else {
            thread::sleep(Duration::from_millis(50));
        }
    }
    let matched = predicate(&output);
    (output, matched)
}

fn wait_for_daemon(pipe_name: &str, timeout: Duration) -> std::fs::File {
    let start = Instant::now();
    loop {
        if let Some(mut file) = try_connect_pipe(pipe_name) {
            if let Ok(()) = frame::write_message(&mut file, &Request::Ping) {
                if let Some(DaemonMessage::Response(Response::Pong)) = read_message(&mut file) {
                    return file;
                }
            }
            drop(file);
        }
        if start.elapsed() > timeout {
            panic!("Daemon did not start within {:?}", timeout);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Launch a daemon with an isolated pipe name. The daemon runs as a child
/// process (GODLY_NO_DETACH=1) so it can be killed cleanly via child.kill().
fn launch_daemon(pipe_name: &str) -> Child {
    let daemon_path = daemon_binary_path();
    Command::new(&daemon_path)
        .env("GODLY_PIPE_NAME", pipe_name)
        .env("GODLY_NO_DETACH", "1")
        .creation_flags(0x00000200) // CREATE_NEW_PROCESS_GROUP
        .spawn()
        .expect("Failed to spawn daemon")
}

/// Kill a daemon child process and wait for it to exit.
fn kill_daemon(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug: Ctrl+C (\x03) doesn't interrupt running processes in PTY sessions.
///
/// When a user runs `npm run dev` (Vite) or any long-running process and
/// presses Ctrl+C, the terminal writes \x03 to the PTY but the process is
/// not interrupted. Ctrl+Z (\x1a) does show ^Z, proving that the data path
/// from frontend to PTY works for other control characters.
///
/// This test creates a daemon PTY session, runs `ping -t localhost` (infinite
/// ping), sends \x03 via a Write request, and asserts that the command is
/// interrupted. On a successful interrupt, we should see one of:
///   - "Control-C" text
///   - Ping statistics summary ("Packets: Sent = ...")
///   - A new PowerShell prompt ("PS ")
///
/// If the test fails, \x03 is not generating CTRL_C_EVENT through the ConPTY,
/// which means processes cannot be interrupted from the terminal.
#[test]
fn test_ctrl_c_interrupts_running_process() {
    eprintln!("\n=== test: Ctrl+C (\\x03) must interrupt running PTY process ===");
    let pipe_name = test_pipe_name("ctrl-c");

    let mut child = launch_daemon(&pipe_name);
    let mut pipe = wait_for_daemon(&pipe_name, Duration::from_secs(10));

    let session_id = "ctrl-c-test";
    let mut output = String::new();

    // Create a PTY session with PowerShell
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
        session_id,
        &mut output,
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    // Attach to session to receive output events
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Attach failed: {:?}",
        resp
    );

    // Wait for PowerShell prompt (indicates shell is ready)
    let (prompt_output, got_prompt) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(10),
        |o| o.contains("PS ") || o.contains("> "),
    );
    output.push_str(&prompt_output);
    eprintln!(
        "  Shell ready: {} (output: {}B)",
        got_prompt,
        output.len()
    );

    // Send a long-running command: `ping -t localhost` runs until interrupted
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"ping -t localhost\r\n".to_vec(),
        },
        session_id,
        &mut output,
    );
    assert!(
        matches!(resp, Response::Ok),
        "Write command failed: {:?}",
        resp
    );

    // Wait for ping to start producing output (at least one reply)
    // Note: match localized output (e.g. "Resposta de" in Portuguese, "Reply from" in English)
    let (ping_output, ping_started) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(10),
        |o| o.contains("Reply from") || o.contains("Pinging") || o.contains("Resposta de") || o.contains("Disparando"),
    );
    output.push_str(&ping_output);
    eprintln!(
        "  Ping started: {} (output: {}B)",
        ping_started,
        output.len()
    );
    assert!(
        ping_started,
        "Ping command did not start. Full output:\n{}",
        output
    );

    // Let ping run for a moment to confirm it's actively producing output
    let extra = collect_output(&mut pipe, session_id, Duration::from_secs(2));
    output.push_str(&extra);

    // Record output length before sending Ctrl+C
    let pre_ctrl_c_len = output.len();

    // Send Ctrl+C (\x03) — this should interrupt the running ping command
    eprintln!("  Sending Ctrl+C (\\x03) to interrupt ping...");
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: vec![0x03], // \x03 = Ctrl+C = ETX
        },
        session_id,
        &mut output,
    );
    assert!(
        matches!(resp, Response::Ok),
        "Write Ctrl+C failed: {:?}",
        resp
    );

    // Collect output after Ctrl+C — look for evidence of interruption
    let (post_output, interrupted) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(8),
        |o| {
            // After Ctrl+C on `ping -t`, Windows shows "Control-C" and
            // ping statistics, then returns to the PS prompt.
            // Note: handle localized output (e.g. Portuguese: "Pacotes:", "Estat")
            o.contains("Control-C")
                || o.contains("^C")
                || o.contains("Packets:")
                || o.contains("Pacotes:")
                || o.contains("Ping statistics")
                || o.contains("Estat")
                || o.contains("PS ")
                || o.contains("Approximate round trip")
        },
    );
    output.push_str(&post_output);

    let new_output = output[pre_ctrl_c_len..].to_string();
    eprintln!(
        "  Output after Ctrl+C ({}B): {:?}",
        new_output.len(),
        &new_output[..new_output.len().min(500)]
    );

    // Cleanup: close session and kill daemon
    let _ = send_request_collecting_output(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    drop(pipe);
    kill_daemon(&mut child);

    // Assert: the process must have been interrupted
    assert!(
        interrupted,
        "\n\nBug: Ctrl+C (\\x03) did NOT interrupt the running process.\n\
         The terminal wrote \\x03 to the PTY via the daemon, but the `ping -t`\n\
         command continued running without interruption.\n\
         \n\
         This reproduces the reported bug: pressing Ctrl+C in Godly Terminal\n\
         does nothing when running long-lived processes like `npm run dev`.\n\
         \n\
         Output after Ctrl+C:\n{:?}\n\
         \n\
         Full session output:\n{:?}\n",
        new_output,
        output
    );
}

/// Bug: Ctrl+C should also work for processes started via `cmd /c` (how npm
/// scripts typically run on Windows). This tests a cmd.exe session instead
/// of PowerShell, since npm often spawns cmd.exe under the hood.
///
/// Uses `ping -t localhost` through cmd.exe and verifies \x03 interrupts it.
#[test]
fn test_ctrl_c_interrupts_cmd_process() {
    eprintln!("\n=== test: Ctrl+C (\\x03) must interrupt cmd.exe process ===");
    let pipe_name = test_pipe_name("ctrl-c-cmd");

    let mut child = launch_daemon(&pipe_name);
    let mut pipe = wait_for_daemon(&pipe_name, Duration::from_secs(10));

    let session_id = "ctrl-c-cmd-test";
    let mut output = String::new();

    // Create session (PowerShell is the default; we'll launch cmd from it)
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for shell prompt
    let (prompt, _) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(10),
        |o| o.contains("PS ") || o.contains("> "),
    );
    output.push_str(&prompt);

    // Launch cmd.exe with an infinite ping (simulates how npm scripts run)
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"cmd /c \"ping -t localhost\"\r\n".to_vec(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok));

    // Wait for ping to start (handle localized output)
    let (ping_out, ping_started) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(10),
        |o| o.contains("Reply from") || o.contains("Pinging") || o.contains("Resposta de") || o.contains("Disparando"),
    );
    output.push_str(&ping_out);
    assert!(ping_started, "Ping via cmd.exe did not start");

    // Let it run briefly
    let extra = collect_output(&mut pipe, session_id, Duration::from_secs(2));
    output.push_str(&extra);

    let pre_ctrl_c_len = output.len();

    // Send Ctrl+C
    eprintln!("  Sending Ctrl+C to cmd.exe ping...");
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: vec![0x03],
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok));

    // Check for interruption
    let (post, interrupted) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(8),
        |o| {
            // Handle localized output (e.g. Portuguese: "Pacotes:", "Estat")
            o.contains("Control-C")
                || o.contains("^C")
                || o.contains("Packets:")
                || o.contains("Pacotes:")
                || o.contains("PS ")
                || o.contains("Ping statistics")
                || o.contains("Estat")
        },
    );
    output.push_str(&post);

    let new_output = output[pre_ctrl_c_len..].to_string();
    eprintln!(
        "  Output after Ctrl+C ({}B): {:?}",
        new_output.len(),
        &new_output[..new_output.len().min(500)]
    );

    // Cleanup
    let _ = send_request_collecting_output(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    drop(pipe);
    kill_daemon(&mut child);

    assert!(
        interrupted,
        "\n\nBug: Ctrl+C (\\x03) did NOT interrupt process running under cmd.exe.\n\
         This is how npm scripts run on Windows (via cmd /c), so this failure\n\
         directly reproduces the reported bug where Ctrl+C does nothing when\n\
         running `npm run dev`.\n\
         \n\
         Output after Ctrl+C:\n{:?}\n",
        new_output
    );
}

/// Verify that \x03 byte survives protocol JSON serialization round-trip.
///
/// If the control character is stripped or corrupted during JSON serialization
/// between the Tauri app and the daemon, Ctrl+C would never reach the PTY.
#[test]
fn test_ctrl_c_byte_survives_protocol_serialization() {
    use std::io::Cursor;

    // Bug: \x03 byte in Write request could be lost during JSON serialization
    let request = Request::Write {
        session_id: "test-session".to_string(),
        data: vec![0x03], // \x03 = Ctrl+C = ETX
    };

    // Serialize to wire format (length-prefixed JSON)
    let mut buf = Vec::new();
    frame::write_message(&mut buf, &request).expect("serialize");

    // Deserialize back
    let mut cursor = Cursor::new(buf);
    let deserialized: Request = frame::read_message(&mut cursor)
        .expect("deserialize")
        .expect("should not be None");

    // Verify the \x03 byte survived intact
    match deserialized {
        Request::Write { session_id, data } => {
            assert_eq!(session_id, "test-session");
            assert_eq!(
                data,
                vec![0x03],
                "Bug: \\x03 byte was corrupted during protocol JSON serialization. Got: {:?}",
                data
            );
        }
        other => panic!("Expected Write request, got {:?}", other),
    }
}

/// Verify that ALL common terminal control characters survive serialization.
/// Tests \x03 (Ctrl+C), \x04 (Ctrl+D), \x16 (Ctrl+V), \x1a (Ctrl+Z).
#[test]
fn test_all_control_characters_survive_serialization() {
    use std::io::Cursor;

    let control_chars: &[(u8, &str)] = &[
        (0x03, "Ctrl+C (SIGINT)"),
        (0x04, "Ctrl+D (EOF)"),
        (0x16, "Ctrl+V (literal-next)"),
        (0x1a, "Ctrl+Z (SIGTSTP)"),
    ];

    for &(byte, name) in control_chars {
        let request = Request::Write {
            session_id: "test".to_string(),
            data: vec![byte],
        };

        let mut buf = Vec::new();
        frame::write_message(&mut buf, &request).expect("serialize");

        let mut cursor = Cursor::new(buf);
        let deserialized: Request = frame::read_message(&mut cursor)
            .expect("deserialize")
            .expect("not None");

        match deserialized {
            Request::Write { data, .. } => {
                assert_eq!(
                    data,
                    vec![byte],
                    "Bug: {} (0x{:02x}) was corrupted during serialization. Got: {:?}",
                    name,
                    byte,
                    data
                );
            }
            other => panic!("Expected Write, got {:?}", other),
        }
    }
}

/// Verify that \x03 embedded in larger payloads (mixed with printable text)
/// also survives serialization — mimics a scenario where Ctrl+C is sent
/// while other characters are buffered.
#[test]
fn test_ctrl_c_in_mixed_payload_survives_serialization() {
    use std::io::Cursor;

    let data = b"hello\x03world".to_vec();
    let request = Request::Write {
        session_id: "test".to_string(),
        data: data.clone(),
    };

    let mut buf = Vec::new();
    frame::write_message(&mut buf, &request).expect("serialize");

    let mut cursor = Cursor::new(buf);
    let deserialized: Request = frame::read_message(&mut cursor)
        .expect("deserialize")
        .expect("not None");

    match deserialized {
        Request::Write {
            data: deser_data, ..
        } => {
            assert_eq!(
                deser_data, data,
                "Bug: \\x03 in mixed payload was corrupted. Got: {:?}",
                deser_data
            );
        }
        other => panic!("Expected Write, got {:?}", other),
    }
}
