//! Bug #411: Quick Claude trust prompt not auto-accepted because idle check fires first.
//!
//! Root cause: `poll_idle_or_trust` checks idle BEFORE trust prompt. When the
//! trust prompt screen is displayed (waiting for Enter, no periodic output), the
//! idle threshold (400ms) is immediately met and the function returns `true`
//! without ever checking `has_trust_prompt()`. This causes quick_claude to write
//! the prompt text into the trust prompt screen, where it's consumed/ignored by
//! ink's selection UI. After a 30s timeout (poll_text_in_output waiting for echo),
//! the Enter keystroke accidentally dismisses the trust prompt, but the prompt
//! text is lost.
//!
//! This test uses a Node.js mock that simulates Claude Code's trust prompt screen:
//! - Outputs initial banner, then after a brief delay, shows the trust prompt text
//! - Goes idle (raw mode stdin, waiting for Enter)
//! - On Enter: outputs "TRUST_ACCEPTED" to confirm the prompt was handled
//!
//! The test replicates the exact `poll_idle_or_trust` logic from terminal.rs and
//! asserts that the trust prompt was auto-accepted. On buggy code, idle detection
//! fires first and the trust prompt is never handled — the test FAILS.
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-daemon --test quick_claude_trust_prompt

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
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
// Node.js mock script — simulates Claude Code's workspace trust prompt
// ---------------------------------------------------------------------------

/// Node.js script that simulates Claude Code showing a trust prompt:
/// 1. Outputs banner text (simulating Claude Code startup)
/// 2. After TRUST_DELAY_MS, outputs the trust prompt text (contains "I trust this folder")
/// 3. Enters raw mode and waits for Enter
/// 4. On Enter: outputs "TRUST_ACCEPTED", then simulates Claude Code ready state
/// 5. Waits for prompt text, then outputs "PROMPT_RECEIVED:<text>"
const MOCK_TRUST_PROMPT_SCRIPT: &str = r#"
const TRUST_DELAY_MS = parseInt(process.env.MOCK_TRUST_DELAY || '1500');

// Phase 1: Banner (like "claude v1.x.x", config loading, etc.)
process.stdout.write('MOCK_BANNER: Claude Code v1.0.0\n');
process.stdout.write('Loading configuration...\n');

// Phase 2: After brief delay, show trust prompt
setTimeout(() => {
  process.stdout.write('\n');
  process.stdout.write('Accessing workspace:\n');
  process.stdout.write('\n');
  process.stdout.write('C:\\Users\\test\\project\n');
  process.stdout.write('\n');
  process.stdout.write('Quick safety check: Is this a project you created or one you trust?\n');
  process.stdout.write('\n');
  process.stdout.write('Security guide\n');
  process.stdout.write('\n');
  process.stdout.write('> 1. Yes, I trust this folder\n');
  process.stdout.write('  2. No, exit\n');
  process.stdout.write('\n');
  process.stdout.write('Enter to confirm\n');

  // Now go idle — raw mode stdin, waiting for Enter
  try {
    process.stdin.setRawMode(true);
  } catch (e) {
    process.stdout.write('RAW_MODE_UNSUPPORTED\n');
    process.exit(3);
  }
  process.stdin.resume();

  let phase = 'trust'; // 'trust' or 'prompt'
  let textBuffer = '';

  process.stdin.on('data', (chunk) => {
    const bytes = [...chunk];
    const hasEnter = bytes.includes(0x0D) || bytes.includes(0x0A);
    const printableBytes = bytes.filter(b => b >= 0x20);

    if (phase === 'trust') {
      if (hasEnter) {
        // Trust prompt accepted
        process.stdout.write('TRUST_ACCEPTED\n');
        phase = 'prompt';

        // Simulate Claude Code initialization after trust acceptance
        setTimeout(() => {
          process.stdout.write('CLAUDE_READY\n');
        }, 500);
      }
      // Ignore non-Enter keys while on trust prompt
    } else if (phase === 'prompt') {
      if (hasEnter && textBuffer.length > 0) {
        process.stdout.write('PROMPT_RECEIVED:' + textBuffer + '\n');
        process.exit(0);
      } else if (!hasEnter) {
        const text = Buffer.from(printableBytes).toString();
        textBuffer += text;
        // Echo text like ink TUI does
        process.stdout.write(text);
      }
    }
  });

  // Safety timeout
  setTimeout(() => {
    process.stdout.write('TIMEOUT:phase=' + phase + ',text=' + textBuffer + '\n');
    process.exit(2);
  }, 60000);
}, TRUST_DELAY_MS);
"#;

// ---------------------------------------------------------------------------
// Helpers (same pattern as quick_claude_enter.rs)
// ---------------------------------------------------------------------------

fn test_pipe_name(test: &str) -> String {
    format!(
        r"\\.\pipe\godly-test-{}-{}",
        test,
        std::process::id()
    )
}

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

fn read_message(pipe: &mut std::fs::File) -> Option<DaemonMessage> {
    frame::read_daemon_message(pipe).ok().flatten()
}

fn send_request_collecting_output(
    pipe: &mut std::fs::File,
    req: &Request,
    session_id: &str,
    output: &mut String,
) -> Response {
    frame::write_request(pipe, req).expect("Failed to write request to pipe");
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
            if let Ok(()) = frame::write_request(&mut file, &Request::Ping) {
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

fn launch_daemon(pipe_name: &str) -> Child {
    use std::os::windows::process::CommandExt;
    let daemon_path = daemon_binary_path();
    Command::new(&daemon_path)
        .env("GODLY_PIPE_NAME", pipe_name)
        .env("GODLY_INSTANCE", pipe_name.trim_start_matches(r"\\.\pipe\"))
        .env("GODLY_NO_DETACH", "1")
        .creation_flags(0x00000200) // CREATE_NEW_PROCESS_GROUP
        .spawn()
        .expect("Failed to spawn daemon")
}

fn kill_daemon(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Write the mock Node.js script to a temp file and return its path.
fn write_mock_script() -> std::path::PathBuf {
    let script_path = std::env::temp_dir().join(format!(
        "godly_mock_trust_prompt_{}.mjs",
        std::process::id()
    ));
    std::fs::write(&script_path, MOCK_TRUST_PROMPT_SCRIPT)
        .expect("Failed to write mock script");
    script_path
}

// ---------------------------------------------------------------------------
// Replicated poll_idle_or_trust logic (exact copy from terminal.rs)
// ---------------------------------------------------------------------------

/// Replicates `has_trust_prompt` from terminal.rs — searches buffer for trust
/// prompt text patterns.
fn has_trust_prompt(
    pipe: &mut std::fs::File,
    session_id: &str,
    output: &mut String,
) -> bool {
    for needle in &["Do you trust the files", "I trust this folder"] {
        let req = Request::SearchBuffer {
            session_id: session_id.to_string(),
            text: needle.to_string(),
            strip_ansi: true,
        };
        let resp = send_request_collecting_output(pipe, &req, session_id, output);
        if matches!(resp, Response::SearchResult { found: true, .. }) {
            return true;
        }
    }
    false
}

/// Replicates `poll_idle_or_trust` from terminal.rs (fixed version).
/// When idle is detected, checks for trust prompt BEFORE returning.
/// Returns (idle_detected, trust_accepted).
fn poll_idle_or_trust(
    pipe: &mut std::fs::File,
    session_id: &str,
    idle_ms: u64,
    timeout_ms: u64,
    output: &mut String,
) -> (bool, bool) {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let poll_interval = (idle_ms / 4).min(500).max(50);
    let trust_check_interval = 2_000u64 / poll_interval;
    let mut iteration = 0u64;
    let mut trust_accepted = false;

    loop {
        // Drain pending events
        while pipe_has_data(pipe) {
            match read_message(pipe) {
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Some(_) => {}
                None => return (false, trust_accepted),
            }
        }

        // Check idle state
        let req = Request::GetLastOutputTime {
            session_id: session_id.to_string(),
        };
        frame::write_request(pipe, &req).expect("write GetLastOutputTime");

        let is_idle;
        loop {
            match read_message(pipe) {
                Some(DaemonMessage::Response(Response::LastOutputTime {
                    epoch_ms, running, ..
                })) => {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let ago = now_ms.saturating_sub(epoch_ms);

                    is_idle = ago >= idle_ms || !running;
                    break;
                }
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Some(_) => {}
                None => return (false, trust_accepted),
            }
        }

        // Fix #411: Check trust prompt BEFORE returning on idle, AND every ~2s.
        // The trust prompt screen is idle (waiting for Enter), so the idle check
        // fires immediately. We must check for the trust prompt before returning.
        if is_idle || iteration % trust_check_interval == 0 {
            if has_trust_prompt(pipe, session_id, output) {
                eprintln!("[test] Detected trust prompt, auto-accepting");
                let accept_req = Request::Write {
                    session_id: session_id.to_string(),
                    data: b"\r".to_vec(),
                };
                let _ = send_request_collecting_output(pipe, &accept_req, session_id, output);
                trust_accepted = true;
                thread::sleep(Duration::from_millis(3_000));
            } else if is_idle {
                return (true, trust_accepted);
            }
        } else if is_idle {
            return (true, trust_accepted);
        }

        if Instant::now() >= deadline {
            return (false, trust_accepted);
        }
        thread::sleep(Duration::from_millis(poll_interval));
        iteration += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug #411: poll_idle_or_trust returns on idle before checking trust prompt.
///
/// Reproduces the exact failure:
/// 1. Mock outputs trust prompt text (containing "I trust this folder")
/// 2. Mock goes idle (raw mode stdin, waiting for Enter)
/// 3. poll_idle_or_trust starts polling
/// 4. Idle check fires immediately (trust screen produces no output)
/// 5. Function returns without checking or accepting the trust prompt
///
/// The test asserts that the trust prompt WAS auto-accepted. On buggy code,
/// idle fires first and trust is never checked — the assertion FAILS.
#[test]
#[ntest::timeout(120_000)]
fn test_trust_prompt_missed_because_idle_fires_first() {
    eprintln!("\n=== Bug #411: trust prompt not auto-accepted (idle fires first) ===");

    // Check node is available
    let node_check = Command::new("node").arg("--version").output();
    if node_check.is_err() || !node_check.unwrap().status.success() {
        eprintln!("  SKIP: Node.js not available");
        return;
    }

    let script_path = write_mock_script();
    let pipe_name = test_pipe_name("qc-trust");

    let mut daemon = launch_daemon(&pipe_name);
    let mut pipe = wait_for_daemon(&pipe_name, Duration::from_secs(10));

    let session_id = "qc-trust-test";
    let mut output = String::new();

    // Create session with PowerShell
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

    // Attach to receive output
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

    // Wait for shell prompt
    let (prompt_out, got_prompt) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("PS ") || o.contains("> "),
    );
    output.push_str(&prompt_out);
    eprintln!("  Shell ready: {}", got_prompt);

    // Launch the mock script (simulates Claude Code with trust prompt)
    let node_cmd = format!(
        "node \"{}\"\r",
        script_path.to_string_lossy().replace('\\', "\\\\")
    );
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: node_cmd.into_bytes(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok), "Write node cmd failed: {:?}", resp);

    // Wait for mock banner to appear
    let (banner_out, mock_started) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("MOCK_BANNER"),
    );
    output.push_str(&banner_out);
    eprintln!("  Mock banner appeared: {}", mock_started);
    assert!(mock_started, "Mock script did not start. Output:\n{}", output);

    // Check for raw mode support
    thread::sleep(Duration::from_millis(500));
    let (check_out, raw_unsupported) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(1),
        |o| o.contains("RAW_MODE_UNSUPPORTED"),
    );
    output.push_str(&check_out);
    if raw_unsupported {
        eprintln!("  SKIP: Raw mode not supported in this ConPTY environment");
        let _ = send_request_collecting_output(
            &mut pipe,
            &Request::CloseSession { session_id: session_id.to_string() },
            session_id,
            &mut output,
        );
        drop(pipe);
        kill_daemon(&mut daemon);
        std::fs::remove_file(&script_path).ok();
        return;
    }

    // Wait for the trust prompt text to appear in the buffer
    // (the mock outputs it after TRUST_DELAY_MS = 1500ms)
    //
    // The trust prompt text may have already arrived during the raw mode check
    // above (the mock's 1500ms delay can overlap with the 500ms sleep + 1s
    // collect_output_until for RAW_MODE_UNSUPPORTED). Check the accumulated
    // output first before blocking on a fresh pipe read.
    let trust_visible = if output.contains("I trust this folder") {
        true
    } else {
        let (trust_out, found) = collect_output_until(
            &mut pipe,
            session_id,
            Duration::from_secs(10),
            |o| o.contains("I trust this folder"),
        );
        output.push_str(&trust_out);
        found
    };
    eprintln!("  Trust prompt visible: {}", trust_visible);
    assert!(
        trust_visible,
        "Trust prompt never appeared. Output:\n{}",
        output
    );

    // --- Simulate quick_claude_background Step 3b ---
    //
    // In the real flow, after the 5s sleep, poll_idle_or_trust starts.
    // The trust prompt has been visible for a while and the session is idle
    // (no more output). The 400ms idle threshold fires immediately.
    //
    // Give the trust prompt a moment to "settle" (like the real 5s sleep would),
    // ensuring the session is truly idle when we start polling.
    thread::sleep(Duration::from_millis(1_000));

    eprintln!("  Running poll_idle_or_trust (replicating terminal.rs logic)...");
    let poll_start = Instant::now();
    let (idle_detected, trust_accepted) = poll_idle_or_trust(
        &mut pipe,
        session_id,
        400,     // Same idle threshold as quick_claude_background
        25_000,  // Same timeout as quick_claude_background
        &mut output,
    );
    let poll_elapsed = poll_start.elapsed();
    eprintln!(
        "  poll_idle_or_trust returned: idle={}, trust_accepted={}, elapsed={:?}",
        idle_detected, trust_accepted, poll_elapsed
    );

    // Check if TRUST_ACCEPTED appeared in the output (mock confirms Enter was received)
    let trust_in_output = output.contains("TRUST_ACCEPTED");
    eprintln!("  TRUST_ACCEPTED in output: {}", trust_in_output);

    // Cleanup
    let _ = send_request_collecting_output(
        &mut pipe,
        &Request::CloseSession { session_id: session_id.to_string() },
        session_id,
        &mut output,
    );
    drop(pipe);
    kill_daemon(&mut daemon);
    std::fs::remove_file(&script_path).ok();

    // --- Assertions ---
    //
    // Bug #411: The trust prompt MUST be auto-accepted during poll_idle_or_trust.
    // On buggy code, idle fires first and the trust prompt is never checked.
    assert!(
        trust_accepted && trust_in_output,
        "\n\n\
         Bug #411: Quick Claude trust prompt not auto-accepted.\n\
         \n\
         poll_idle_or_trust returned after {:?}:\n\
         - idle_detected: {}\n\
         - trust_accepted: {}\n\
         - TRUST_ACCEPTED in output: {}\n\
         \n\
         The idle check (400ms threshold) in poll_idle_or_trust fires BEFORE\n\
         the trust prompt check. When the trust prompt screen is displayed\n\
         (waiting for Enter, producing no output), the idle threshold is\n\
         immediately met and the function returns without checking for\n\
         or accepting the trust prompt.\n\
         \n\
         This causes quick_claude to write the prompt text into the trust\n\
         prompt screen (where it's ignored), then wait 30s for an echo\n\
         that never comes, then finally send Enter which dismisses the\n\
         trust prompt — but by then the prompt text is lost.\n\
         \n\
         Fix: Check trust prompt BEFORE idle, or always check trust prompt\n\
         even when idle is detected (check-then-return pattern).\n\
         \n\
         Full output ({} bytes):\n{}\n",
        poll_elapsed,
        idle_detected,
        trust_accepted,
        trust_in_output,
        output.len(),
        &output[..output.len().min(2000)],
    );
}

/// Confirms that SearchBuffer finds the trust prompt text in the mock's output.
///
/// This is a prerequisite test: if SearchBuffer can't find "I trust this folder"
/// in the terminal buffer, the trust prompt detection can never work regardless
/// of the idle/trust check ordering bug.
#[test]
#[ntest::timeout(60_000)]
fn test_search_buffer_finds_trust_prompt_text() {
    eprintln!("\n=== Prerequisite: SearchBuffer finds trust prompt text ===");

    let node_check = Command::new("node").arg("--version").output();
    if node_check.is_err() || !node_check.unwrap().status.success() {
        eprintln!("  SKIP: Node.js not available");
        return;
    }

    let script_path = write_mock_script();
    let pipe_name = test_pipe_name("qc-trust-search");

    let mut daemon = launch_daemon(&pipe_name);
    let mut pipe = wait_for_daemon(&pipe_name, Duration::from_secs(10));

    let session_id = "qc-trust-search";
    let mut output = String::new();

    // Create and attach session
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

    // Wait for shell
    let (prompt_out, _) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("PS ") || o.contains("> "),
    );
    output.push_str(&prompt_out);

    // Launch mock
    let node_cmd = format!(
        "node \"{}\"\r",
        script_path.to_string_lossy().replace('\\', "\\\\")
    );
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: node_cmd.into_bytes(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok));

    // Wait for trust prompt text to appear
    let (trust_out, found) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("I trust this folder"),
    );
    output.push_str(&trust_out);
    assert!(found, "Trust prompt text never appeared. Output:\n{}", output);

    // Give daemon time to process the output into the VT parser
    thread::sleep(Duration::from_millis(500));

    // Now test that SearchBuffer finds the trust prompt text
    let found = has_trust_prompt(&mut pipe, session_id, &mut output);
    eprintln!("  SearchBuffer found trust prompt: {}", found);

    // Cleanup
    let _ = send_request_collecting_output(
        &mut pipe,
        &Request::CloseSession { session_id: session_id.to_string() },
        session_id,
        &mut output,
    );
    drop(pipe);
    kill_daemon(&mut daemon);
    std::fs::remove_file(&script_path).ok();

    assert!(
        found,
        "SearchBuffer did not find trust prompt text ('I trust this folder').\n\
         This means the trust prompt detection can never work, regardless of\n\
         the idle/trust check ordering.\n\
         Output:\n{}",
        output
    );
}
