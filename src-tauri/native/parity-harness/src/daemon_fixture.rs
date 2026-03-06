use std::io::{Read, Write};
use std::process::{Child, Command};
use std::time::Duration;

use godly_protocol::{
    read_daemon_message, write_request_with_id, DaemonMessage, Request, Response,
};

/// Isolated daemon fixture for integration tests.
///
/// Spawns a daemon with unique pipe name and instance, kills by PID on drop.
/// Follows the same isolation rules as `daemon/tests/` — never touches
/// the production daemon.
pub struct DaemonFixture {
    child: Option<Child>,
    pipe_name: String,
    reader: Option<Box<dyn Read + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    next_request_id: u32,
}

impl DaemonFixture {
    /// Spawn an isolated daemon and connect to it.
    ///
    /// Uses a unique pipe name derived from the test name to avoid
    /// interference between concurrent tests or with the production daemon.
    #[cfg(windows)]
    pub fn start(test_name: &str) -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;
        use std::os::windows::process::CommandExt;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::{DuplicateHandle, INVALID_HANDLE_VALUE};
        use winapi::um::processthreadsapi::GetCurrentProcess;
        use winapi::um::winnt::{
            DUPLICATE_SAME_ACCESS, FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE,
        };

        let pipe_name = format!(
            r"\\.\pipe\godly-test-parity-{}-{}",
            test_name,
            std::process::id()
        );
        let instance = pipe_name.trim_start_matches(r"\\.\pipe\");

        // Find daemon binary
        let daemon_path = Self::find_daemon_binary()?;

        // Spawn daemon with isolation env vars
        let child = Command::new(&daemon_path)
            .env("GODLY_PIPE_NAME", &pipe_name)
            .env("GODLY_INSTANCE", instance)
            .env("GODLY_NO_DETACH", "1")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

        // Wait for pipe to become available
        let pipe_wide: Vec<u16> = OsStr::new(&pipe_name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));

            let handle = unsafe {
                CreateFileW(
                    pipe_wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };

            if handle != INVALID_HANDLE_VALUE {
                // Duplicate handle for separate reader/writer
                let mut writer_handle = std::ptr::null_mut();
                let dup_result = unsafe {
                    DuplicateHandle(
                        GetCurrentProcess(),
                        handle,
                        GetCurrentProcess(),
                        &mut writer_handle,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    )
                };

                if dup_result == 0 {
                    return Err(format!("DuplicateHandle failed: {}", unsafe {
                        GetLastError()
                    }));
                }

                let reader: Box<dyn Read + Send> =
                    Box::new(unsafe { std::fs::File::from_raw_handle(handle as _) });
                let writer: Box<dyn Write + Send> =
                    Box::new(unsafe { std::fs::File::from_raw_handle(writer_handle as _) });

                return Ok(Self {
                    child: Some(child),
                    pipe_name,
                    reader: Some(reader),
                    writer: Some(writer),
                    next_request_id: 1,
                });
            }
        }

        // Kill the child if we couldn't connect
        let mut child = child;
        let _ = child.kill();
        Err(format!("Timed out waiting for daemon pipe: {}", pipe_name))
    }

    #[cfg(not(windows))]
    pub fn start(_test_name: &str) -> Result<Self, String> {
        Err("DaemonFixture only supported on Windows".to_string())
    }

    /// Send a request and read the response.
    pub fn send_request(&mut self, request: &Request) -> Result<Response, String> {
        let writer = self.writer.as_mut().ok_or("Writer not available")?;
        let reader = self.reader.as_mut().ok_or("Reader not available")?;

        let request_id = self.next_request_id;
        self.next_request_id += 1;

        write_request_with_id(writer, request, Some(request_id))
            .map_err(|e| format!("Failed to write request: {}", e))?;

        // Read messages until we get a response (skip events)
        loop {
            let msg = read_daemon_message(reader)
                .map_err(|e| format!("Failed to read response: {}", e))?
                .ok_or("Unexpected EOF from daemon")?;

            match msg {
                DaemonMessage::Response(response) => return Ok(response),
                DaemonMessage::Event(_) => {
                    // Skip events, keep reading for the response
                    continue;
                }
            }
        }
    }

    /// Get the pipe name for this fixture.
    pub fn pipe_name(&self) -> &str {
        &self.pipe_name
    }

    fn find_daemon_binary() -> Result<std::path::PathBuf, String> {
        // Look in target/debug relative to the workspace root
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_root = std::path::Path::new(manifest_dir)
            .parent() // native/
            .and_then(|p| p.parent()) // src-tauri/
            .ok_or("Cannot find workspace root")?;

        let daemon_name = if cfg!(windows) {
            "godly-daemon.exe"
        } else {
            "godly-daemon"
        };

        // Check target/debug first (most common in tests)
        let debug_path = workspace_root
            .join("target")
            .join("debug")
            .join(daemon_name);
        if debug_path.exists() {
            return Ok(debug_path);
        }

        // Check target/release
        let release_path = workspace_root
            .join("target")
            .join("release")
            .join(daemon_name);
        if release_path.exists() {
            return Ok(release_path);
        }

        Err(format!(
            "Daemon binary not found. Looked in: {:?} and {:?}. Run `cargo build -p godly-daemon` first.",
            debug_path, release_path
        ))
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        // Drop reader/writer first to close pipe handles
        self.reader.take();
        self.writer.take();

        // Kill the daemon by PID (never by name!)
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
