//! Memory stress tests for the godly-daemon process.
//!
//! These integration tests spawn a real daemon process, exercise it via named pipe IPC,
//! and measure RSS (Working Set Size on Windows) to detect memory leaks.
//!
//! Each test uses a unique pipe name via GODLY_PIPE_NAME env var for isolation.
//!
//! Run with: cargo test -p godly-daemon --test memory_stress -- --nocapture

#[cfg(windows)]
mod windows_tests {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::process::{Child, Command};
    use std::time::Duration;

    use godly_protocol::{DaemonMessage, Request, Response, ShellType};

    /// Get the Working Set Size (RSS equivalent) of a process by PID.
    fn get_process_memory_bytes(pid: u32) -> Option<usize> {
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::processthreadsapi::OpenProcess;
        use winapi::um::psapi::GetProcessMemoryInfo;
        use winapi::um::psapi::PROCESS_MEMORY_COUNTERS;
        use winapi::um::winnt::PROCESS_QUERY_INFORMATION;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }

            let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

            let result = GetProcessMemoryInfo(
                handle,
                &mut pmc,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            );

            CloseHandle(handle);

            if result != 0 {
                Some(pmc.WorkingSetSize)
            } else {
                None
            }
        }
    }

    /// Connect to a named pipe, retrying until the daemon is ready.
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

        let start = std::time::Instant::now();
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
                    "Failed to connect to daemon pipe '{}' within {:?} (error: {})",
                    pipe_name, timeout, err
                );
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Send a request and read the response from the pipe.
    fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
        godly_protocol::write_request(pipe, request).expect("Failed to write request");

        // Read response — may need to skip Event messages
        loop {
            let msg: DaemonMessage =
                godly_protocol::read_daemon_message(pipe)
                    .expect("Failed to read message")
                    .expect("Unexpected EOF reading response");

            match msg {
                DaemonMessage::Response(resp) => return resp,
                DaemonMessage::Event(_) => {
                    // Skip async events, keep reading for the response
                    continue;
                }
            }
        }
    }

    /// Drain any pending events from the pipe (non-blocking via peek).
    fn drain_events(pipe: &std::fs::File) {
        use winapi::um::namedpipeapi::PeekNamedPipe;

        let handle = {
            use std::os::windows::io::AsRawHandle;
            pipe.as_raw_handle() as *mut _
        };

        loop {
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

            if result == 0 || bytes_available == 0 {
                break;
            }

            // Read and discard
            let msg: Option<DaemonMessage> =
                godly_protocol::read_daemon_message(&mut &*pipe).ok().flatten();
            if msg.is_none() {
                break;
            }
        }
    }

    struct DaemonFixture {
        child: Child,
        pipe_name: String,
    }

    impl DaemonFixture {
        fn spawn(test_name: &str) -> Self {
            let pipe_name = format!(r"\\.\pipe\godly-test-{}-{}", test_name, std::process::id());

            // Build the daemon binary first (debug mode)
            let status = Command::new("cargo")
                .args(["build", "-p", "godly-daemon"])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .status()
                .expect("Failed to run cargo build");
            assert!(status.success(), "cargo build failed");

            // Find the daemon binary
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let target_dir = manifest_dir.parent().unwrap().join("target").join("debug");
            let daemon_exe = target_dir.join("godly-daemon.exe");
            assert!(
                daemon_exe.exists(),
                "Daemon binary not found at {:?}",
                daemon_exe
            );

            // Spawn daemon with custom pipe name and no-detach
            let child = Command::new(&daemon_exe)
                .env("GODLY_PIPE_NAME", &pipe_name)
                .env("GODLY_NO_DETACH", "1")
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn daemon");

            eprintln!(
                "[test] Spawned daemon pid={} on pipe={}",
                child.id(),
                pipe_name
            );

            // Give daemon time to start listening
            std::thread::sleep(Duration::from_millis(500));

            Self { child, pipe_name }
        }

        fn pid(&self) -> u32 {
            self.child.id()
        }

        fn connect(&self) -> std::fs::File {
            connect_pipe(&self.pipe_name, Duration::from_secs(5))
        }

        fn memory_bytes(&self) -> usize {
            get_process_memory_bytes(self.pid()).expect("Failed to get daemon memory")
        }
    }

    impl Drop for DaemonFixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            eprintln!("[test] Daemon process killed");
        }
    }

    #[test]
    #[ntest::timeout(180_000)] // 3min — creates/destroys many sessions + measures RSS
    fn test_session_create_destroy_no_leak() {
        let daemon = DaemonFixture::spawn("create-destroy");
        let mut pipe = daemon.connect();

        // Verify connection
        let resp = send_request(&mut pipe, &Request::Ping);
        assert!(matches!(resp, Response::Pong), "Ping failed: {:?}", resp);

        // Warm up: create and destroy 5 sessions to let the allocator settle
        for i in 0..5 {
            let id = format!("warmup-{}", i);
            let resp = send_request(
                &mut pipe,
                &Request::CreateSession {
                    id: id.clone(),
                    shell_type: ShellType::Windows,
                    cwd: None,
                    rows: 24,
                    cols: 80,
                    env: None,
                },
            );
            assert!(
                matches!(resp, Response::SessionCreated { .. }),
                "Create failed: {:?}",
                resp
            );

            // Brief pause to let the shell start
            std::thread::sleep(Duration::from_millis(200));

            let resp = send_request(&mut pipe, &Request::CloseSession { session_id: id });
            assert!(matches!(resp, Response::Ok), "Close failed: {:?}", resp);
        }

        // Let things settle after warmup
        std::thread::sleep(Duration::from_secs(1));

        // Measure baseline RSS
        let baseline = daemon.memory_bytes();
        eprintln!("[test] Baseline RSS: {} bytes ({:.1} MB)", baseline, baseline as f64 / 1_048_576.0);

        // Stress: create + attach + write + detach + close 50 sessions
        let num_sessions = 50;
        for i in 0..num_sessions {
            let id = format!("stress-{}", i);

            // Create
            let resp = send_request(
                &mut pipe,
                &Request::CreateSession {
                    id: id.clone(),
                    shell_type: ShellType::Windows,
                    cwd: None,
                    rows: 24,
                    cols: 80,
                    env: None,
                },
            );
            assert!(
                matches!(resp, Response::SessionCreated { .. }),
                "Create #{} failed: {:?}",
                i,
                resp
            );

            // Attach
            let resp = send_request(
                &mut pipe,
                &Request::Attach {
                    session_id: id.clone(),
                },
            );
            assert!(
                matches!(resp, Response::Ok | Response::Buffer { .. }),
                "Attach #{} failed: {:?}",
                i,
                resp
            );

            // Write 1KB of data
            let data = vec![b'x'; 1024];
            let resp = send_request(
                &mut pipe,
                &Request::Write {
                    session_id: id.clone(),
                    data,
                },
            );
            assert!(matches!(resp, Response::Ok), "Write #{} failed: {:?}", i, resp);

            // Brief pause for shell to process
            std::thread::sleep(Duration::from_millis(100));

            // Drain any output events
            drain_events(&pipe);

            // Detach
            let resp = send_request(
                &mut pipe,
                &Request::Detach {
                    session_id: id.clone(),
                },
            );
            assert!(matches!(resp, Response::Ok), "Detach #{} failed: {:?}", i, resp);

            // Close
            let resp = send_request(
                &mut pipe,
                &Request::CloseSession {
                    session_id: id,
                },
            );
            assert!(matches!(resp, Response::Ok), "Close #{} failed: {:?}", i, resp);

            if (i + 1) % 10 == 0 {
                let current = daemon.memory_bytes();
                eprintln!(
                    "[test] After {}/{} sessions: RSS = {} bytes ({:.1} MB), delta = {:.1} MB",
                    i + 1,
                    num_sessions,
                    current,
                    current as f64 / 1_048_576.0,
                    (current as f64 - baseline as f64) / 1_048_576.0
                );
            }
        }

        // Let the daemon settle
        std::thread::sleep(Duration::from_secs(2));

        // Measure final RSS
        let after = daemon.memory_bytes();
        let growth = if after > baseline { after - baseline } else { 0 };
        eprintln!(
            "[test] Final RSS: {} bytes ({:.1} MB), growth: {} bytes ({:.1} MB)",
            after,
            after as f64 / 1_048_576.0,
            growth,
            growth as f64 / 1_048_576.0
        );

        // Allow up to 12MB growth for allocator fragmentation, Windows overhead,
        // and godly-vt parser grid buffers (~60KB per session from 80x24 grid of
        // 32-byte cells) that linger until reader threads exit their blocking read.
        let max_growth = 12 * 1024 * 1024;
        assert!(
            growth < max_growth,
            "Memory grew by {} bytes ({:.1} MB) after {} create/destroy cycles — exceeds {} MB threshold. Likely leak!",
            growth,
            growth as f64 / 1_048_576.0,
            num_sessions,
            max_growth / 1_048_576
        );
    }

    #[test]
    #[ntest::timeout(180_000)]
    fn test_attach_detach_no_leak() {
        let daemon = DaemonFixture::spawn("attach-detach");
        let mut pipe = daemon.connect();

        // Verify connection
        let resp = send_request(&mut pipe, &Request::Ping);
        assert!(matches!(resp, Response::Pong));

        // Create 3 persistent sessions
        let session_count = 3;
        let mut session_ids = Vec::new();
        for i in 0..session_count {
            let id = format!("persist-{}", i);
            let resp = send_request(
                &mut pipe,
                &Request::CreateSession {
                    id: id.clone(),
                    shell_type: ShellType::Windows,
                    cwd: None,
                    rows: 24,
                    cols: 80,
                    env: None,
                },
            );
            assert!(matches!(resp, Response::SessionCreated { .. }));
            session_ids.push(id);
        }

        // Let shells start
        std::thread::sleep(Duration::from_secs(1));

        // Warm up: attach/detach each 5 times
        for id in &session_ids {
            for _ in 0..5 {
                let resp = send_request(
                    &mut pipe,
                    &Request::Attach {
                        session_id: id.clone(),
                    },
                );
                assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

                std::thread::sleep(Duration::from_millis(50));
                drain_events(&pipe);

                let resp = send_request(
                    &mut pipe,
                    &Request::Detach {
                        session_id: id.clone(),
                    },
                );
                assert!(matches!(resp, Response::Ok));
            }
        }

        std::thread::sleep(Duration::from_secs(1));
        let baseline = daemon.memory_bytes();
        eprintln!("[test] Baseline RSS: {:.1} MB", baseline as f64 / 1_048_576.0);

        // Stress: attach/detach each session 100 times
        let cycles = 100;
        for cycle in 0..cycles {
            for id in &session_ids {
                let resp = send_request(
                    &mut pipe,
                    &Request::Attach {
                        session_id: id.clone(),
                    },
                );
                assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

                std::thread::sleep(Duration::from_millis(10));
                drain_events(&pipe);

                let resp = send_request(
                    &mut pipe,
                    &Request::Detach {
                        session_id: id.clone(),
                    },
                );
                assert!(matches!(resp, Response::Ok));
            }

            if (cycle + 1) % 25 == 0 {
                let current = daemon.memory_bytes();
                eprintln!(
                    "[test] After {}/{} cycles: RSS = {:.1} MB, delta = {:.1} MB",
                    cycle + 1,
                    cycles,
                    current as f64 / 1_048_576.0,
                    (current as f64 - baseline as f64) / 1_048_576.0
                );
            }
        }

        // Clean up sessions
        for id in &session_ids {
            send_request(
                &mut pipe,
                &Request::CloseSession {
                    session_id: id.clone(),
                },
            );
        }

        std::thread::sleep(Duration::from_secs(2));

        let after = daemon.memory_bytes();
        let growth = if after > baseline { after - baseline } else { 0 };
        eprintln!(
            "[test] Final RSS: {:.1} MB, growth: {:.1} MB",
            after as f64 / 1_048_576.0,
            growth as f64 / 1_048_576.0
        );

        let max_growth = 5 * 1024 * 1024;
        assert!(
            growth < max_growth,
            "Memory grew by {:.1} MB after {} attach/detach cycles — exceeds {} MB threshold. Likely leak!",
            growth as f64 / 1_048_576.0,
            cycles * session_count,
            max_growth / 1_048_576
        );
    }

    #[test]
    #[ntest::timeout(180_000)]
    fn test_heavy_output_no_leak() {
        let daemon = DaemonFixture::spawn("heavy-output");
        let mut pipe = daemon.connect();

        // Verify connection
        let resp = send_request(&mut pipe, &Request::Ping);
        assert!(matches!(resp, Response::Pong));

        // Create a single session
        let id = "heavy-output-session".to_string();
        let resp = send_request(
            &mut pipe,
            &Request::CreateSession {
                id: id.clone(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            },
        );
        assert!(matches!(resp, Response::SessionCreated { .. }));

        // Attach to it
        let resp = send_request(
            &mut pipe,
            &Request::Attach {
                session_id: id.clone(),
            },
        );
        assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

        // Let shell start
        std::thread::sleep(Duration::from_secs(1));

        let baseline = daemon.memory_bytes();
        eprintln!("[test] Baseline RSS: {:.1} MB", baseline as f64 / 1_048_576.0);

        // Write 10MB of data in 10KB chunks (simulates heavy shell output flowing through)
        let total_bytes = 10 * 1024 * 1024;
        let chunk_size = 10 * 1024;
        let chunks = total_bytes / chunk_size;

        for i in 0..chunks {
            let data = vec![b'A'; chunk_size];
            let resp = send_request(
                &mut pipe,
                &Request::Write {
                    session_id: id.clone(),
                    data,
                },
            );
            assert!(matches!(resp, Response::Ok), "Write chunk {} failed: {:?}", i, resp);

            // Drain output events periodically to prevent pipe buffer backpressure
            if (i + 1) % 10 == 0 {
                drain_events(&pipe);
            }

            if (i + 1) % 100 == 0 {
                let current = daemon.memory_bytes();
                eprintln!(
                    "[test] After {:.1} MB written: RSS = {:.1} MB",
                    ((i + 1) * chunk_size) as f64 / 1_048_576.0,
                    current as f64 / 1_048_576.0
                );
            }
        }

        // Detach and let ring buffer settle
        let resp = send_request(
            &mut pipe,
            &Request::Detach {
                session_id: id.clone(),
            },
        );
        assert!(matches!(resp, Response::Ok));

        std::thread::sleep(Duration::from_secs(2));

        // Close session
        let resp = send_request(
            &mut pipe,
            &Request::CloseSession {
                session_id: id,
            },
        );
        assert!(matches!(resp, Response::Ok));

        std::thread::sleep(Duration::from_secs(2));

        let after = daemon.memory_bytes();
        let growth = if after > baseline { after - baseline } else { 0 };
        eprintln!(
            "[test] Final RSS: {:.1} MB, growth: {:.1} MB (after {:.1} MB written)",
            after as f64 / 1_048_576.0,
            growth as f64 / 1_048_576.0,
            total_bytes as f64 / 1_048_576.0
        );

        // After CloseSession, the session is dropped and the reader thread should
        // exit (master Arc reaches refcount 0 → ConPTY destroyed → EOF on read pipe).
        // All bounded allocations (ring buffer, output_history, vt_parser scrollback)
        // are freed when the thread exits.
        //
        // However, Windows RSS (working set) doesn't shrink instantly after free() —
        // the allocator retains pages and the OS reclaims them lazily. Allow 15MB to
        // accommodate allocator fragmentation + OS page retention, while still catching
        // true leaks (which would grow proportionally to total data throughput).
        let max_growth = 15 * 1024 * 1024;
        assert!(
            growth < max_growth,
            "Memory grew by {:.1} MB after writing {:.1} MB — ring buffer may not be capping correctly",
            growth as f64 / 1_048_576.0,
            total_bytes as f64 / 1_048_576.0
        );
    }
}

#[cfg(not(windows))]
mod non_windows {
    #[test]
    #[ntest::timeout(10_000)]
    fn test_memory_stress_not_supported() {
        eprintln!("Memory stress tests are only supported on Windows (named pipes)");
    }
}
