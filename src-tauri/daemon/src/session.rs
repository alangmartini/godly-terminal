use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use godly_protocol::types::ShellType;

use crate::debug_log::daemon_log;

const RING_BUFFER_SIZE: usize = 1024 * 1024; // 1MB ring buffer

/// A daemon-managed PTY session that survives app disconnections.
pub struct DaemonSession {
    pub id: String,
    pub shell_type: ShellType,
    pub cwd: Option<String>,
    pub created_at: u64,
    pub rows: u16,
    pub cols: u16,
    #[cfg(windows)]
    pub pid: u32,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    running: Arc<AtomicBool>,
    /// Ring buffer accumulates output when no client is attached
    ring_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Channel sender for live output to an attached client
    output_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<Vec<u8>>>>>,
    /// Lock-free attachment flag — readable without locking output_tx.
    /// Updated in attach()/detach()/close() and by the reader thread on send failure.
    is_attached_flag: Arc<AtomicBool>,
}

impl DaemonSession {
    pub fn new(
        id: String,
        shell_type: ShellType,
        cwd: Option<String>,
        rows: u16,
        cols: u16,
        env: Option<HashMap<String, String>>,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| format!("Failed to open pty: {}", e))?;

        let mut cmd = match &shell_type {
            ShellType::Windows => {
                let mut cmd = CommandBuilder::new("powershell.exe");
                cmd.arg("-NoLogo");
                if let Some(dir) = &cwd {
                    cmd.cwd(dir);
                }
                cmd
            }
            ShellType::Wsl { distribution } => {
                let mut cmd = CommandBuilder::new("wsl.exe");
                if let Some(distro) = distribution {
                    cmd.args(["-d", distro]);
                }
                if let Some(dir) = &cwd {
                    let wsl_path = windows_to_wsl_path(dir);
                    cmd.args(["--cd", &wsl_path]);
                }
                cmd
            }
        };

        // Inject environment variables into the PTY session
        if let Some(env_vars) = &env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        #[cfg(windows)]
        let pid = child.process_id().unwrap_or(0);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get writer: {}", e))?;

        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let running = Arc::new(AtomicBool::new(true));
        let ring_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_SIZE)));
        let output_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<Vec<u8>>>>> =
            Arc::new(Mutex::new(None));
        let is_attached_flag = Arc::new(AtomicBool::new(false));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Spawn reader thread that routes output to either the attached client or ring buffer
        let reader_master = master.clone();
        let reader_running = running.clone();
        let reader_ring = ring_buffer.clone();
        let reader_tx = output_tx.clone();
        let session_id = id.clone();
        let reader_attached = is_attached_flag.clone();

        thread::spawn(move || {
            let mut reader = {
                let master = reader_master.lock();
                match master.try_clone_reader() {
                    Ok(r) => r,
                    Err(e) => {
                        daemon_log!("Session {} reader: failed to clone reader: {}", session_id, e);
                        return;
                    }
                }
            };

            daemon_log!("Session {} reader thread started", session_id);

            let mut buf = [0u8; 65536];
            let mut total_bytes: u64 = 0;
            let mut total_reads: u64 = 0;
            let mut channel_send_failures: u64 = 0;
            let mut last_stats = Instant::now();

            while reader_running.load(Ordering::Relaxed) {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        daemon_log!("Session {} reader: EOF (process exited)", session_id);
                        break;
                    }
                    Ok(n) => {
                        total_bytes += n as u64;
                        total_reads += 1;

                        let data = buf[..n].to_vec();
                        let lock_start = Instant::now();
                        let tx_guard = reader_tx.lock();
                        let lock_elapsed = lock_start.elapsed();

                        if lock_elapsed.as_millis() > 50 {
                            daemon_log!(
                                "Session {} reader: SLOW LOCK on output_tx: {:.1}ms",
                                session_id,
                                lock_elapsed.as_secs_f64() * 1000.0
                            );
                        }

                        if let Some(tx) = tx_guard.as_ref() {
                            // Client attached: try to send live output (bounded channel applies backpressure)
                            match tx.try_send(data) {
                                Ok(()) => {
                                    drop(tx_guard);
                                    // Yield to let handler threads acquire output_tx if waiting
                                    thread::yield_now();
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Full(data)) => {
                                    // Bug A1 fix: block until channel has capacity instead of
                                    // falling back to ring buffer. Previously, data written to
                                    // the ring buffer during transient fullness was never
                                    // replayed to the attached client, causing missing output.
                                    // blocking_send provides true backpressure — the PTY read
                                    // pauses naturally because this thread is blocked.
                                    let tx_clone = tx.clone();
                                    drop(tx_guard);
                                    let bp_start = Instant::now();
                                    match tx_clone.blocking_send(data) {
                                        Ok(()) => {
                                            let bp_elapsed = bp_start.elapsed();
                                            if bp_elapsed.as_millis() > 50 {
                                                daemon_log!(
                                                    "Session {} reader: backpressure {:.1}ms (channel was full)",
                                                    session_id,
                                                    bp_elapsed.as_secs_f64() * 1000.0
                                                );
                                            }
                                            thread::yield_now();
                                        }
                                        Err(send_err) => {
                                            // Channel closed during backpressure — client disconnected
                                            channel_send_failures += 1;
                                            daemon_log!(
                                                "Session {} reader: channel closed during backpressure (disconnect #{})",
                                                session_id,
                                                channel_send_failures
                                            );
                                            reader_attached.store(false, Ordering::Relaxed);
                                            *reader_tx.lock() = None;
                                            let mut ring = reader_ring.lock();
                                            append_to_ring(&mut ring, &send_err.0);
                                        }
                                    }
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                    // Client disconnected, switch to ring buffer
                                    channel_send_failures += 1;
                                    daemon_log!(
                                        "Session {} reader: channel send failed (client disconnect #{})",
                                        session_id,
                                        channel_send_failures
                                    );
                                    drop(tx_guard);
                                    reader_attached.store(false, Ordering::Relaxed);
                                    *reader_tx.lock() = None;
                                    // Store this chunk in ring buffer
                                    let mut ring = reader_ring.lock();
                                    append_to_ring(&mut ring, &buf[..n]);
                                }
                            }
                        } else {
                            // No client attached: buffer output
                            drop(tx_guard);
                            let mut ring = reader_ring.lock();
                            append_to_ring(&mut ring, &buf[..n]);
                        }

                        // Periodic stats
                        if last_stats.elapsed().as_secs() > 30 {
                            let ring_size = reader_ring.lock().len();
                            let attached = reader_tx.lock().is_some();
                            daemon_log!(
                                "Session {} reader stats: reads={}, bytes={}, send_failures={}, ring_buf={:.0}KB, attached={}",
                                session_id,
                                total_reads,
                                total_bytes,
                                channel_send_failures,
                                ring_size as f64 / 1024.0,
                                attached
                            );
                            last_stats = Instant::now();
                        }
                    }
                    Err(e) => {
                        daemon_log!("Session {} reader: read error: {}", session_id, e);
                        break;
                    }
                }
            }

            // PTY exited — mark session as dead and close output channel.
        // Setting output_tx = None causes rx.recv() in the forwarding task to
        // return None, which triggers sending SessionClosed to the client.
        reader_running.store(false, Ordering::Relaxed);
        reader_attached.store(false, Ordering::Relaxed);
        *reader_tx.lock() = None;

            daemon_log!(
                "Session {} reader thread exited: reads={}, bytes={}, send_failures={}",
                session_id,
                total_reads,
                total_bytes,
                channel_send_failures
            );
            eprintln!("[daemon] Session {} reader thread exited", session_id);
        });

        // Keep child handle alive
        thread::spawn(move || {
            let _ = child;
        });

        Ok(Self {
            id,
            shell_type,
            cwd,
            created_at: now,
            rows,
            cols,
            #[cfg(windows)]
            pid,
            master,
            writer,
            running,
            ring_buffer,
            output_tx,
            is_attached_flag,
        })
    }

    /// Attach a client to this session.
    /// Returns (buffered_data, receiver_for_live_output).
    ///
    /// Uses `try_lock_for` with timeouts to avoid blocking the handler indefinitely
    /// when the reader thread holds ring_buffer or output_tx under heavy output.
    pub fn attach(&self) -> (Vec<u8>, tokio::sync::mpsc::Receiver<Vec<u8>>) {
        let (tx, rx) = tokio::sync::mpsc::channel(64);

        // Drain ring buffer as initial replay — timeout to avoid blocking handler
        let buffered: Vec<u8> = match self.ring_buffer.try_lock_for(Duration::from_secs(2)) {
            Some(mut ring) => ring.drain(..).collect(),
            None => {
                daemon_log!(
                    "WARN: ring_buffer lock timeout in attach for session {}, returning empty buffer",
                    self.id
                );
                Vec::new()
            }
        };

        // Set the output sender for live streaming — timeout to avoid blocking handler
        match self.output_tx.try_lock_for(Duration::from_secs(2)) {
            Some(mut guard) => *guard = Some(tx),
            None => {
                daemon_log!(
                    "WARN: output_tx lock timeout in attach for session {}",
                    self.id
                );
            }
        };

        self.is_attached_flag.store(true, Ordering::Relaxed);

        (buffered, rx)
    }

    /// Detach the current client. Output will accumulate in the ring buffer.
    pub fn detach(&self) {
        self.is_attached_flag.store(false, Ordering::Relaxed);
        *self.output_tx.lock() = None;
    }

    /// Check if a client is currently attached (lock-free).
    ///
    /// Reads an AtomicBool instead of locking output_tx. This prevents the
    /// handler from blocking on ListSessions/info() when the reader thread
    /// holds output_tx in a tight loop under heavy output.
    pub fn is_attached(&self) -> bool {
        self.is_attached_flag.load(Ordering::Relaxed)
    }

    /// Write data to the PTY.
    ///
    /// On Windows, raw `\x03` (Ctrl+C) written to ConPTY's input pipe does NOT
    /// generate `CTRL_C_EVENT` for child processes. ConPTY only generates console
    /// control events from real keyboard input, not from pipe-written data.
    /// `GenerateConsoleCtrlEvent` also doesn't work with pseudoconsoles.
    ///
    /// To interrupt a running process, we detect `\x03` and terminate child
    /// processes of the shell, leaving the shell itself alive so the user gets
    /// a fresh prompt.
    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        #[cfg(windows)]
        if data.contains(&0x03) {
            match terminate_child_processes(self.pid) {
                Ok(count) => {
                    if count > 0 {
                        daemon_log!(
                            "Session {} Ctrl+C: terminated {} child process(es) of shell pid {}",
                            self.id, count, self.pid
                        );
                    }
                }
                Err(e) => {
                    daemon_log!("Session {} Ctrl+C failed: {}", self.id, e);
                }
            }
            // Also write \x03 to the PTY — while ConPTY won't generate
            // CTRL_C_EVENT, some shells (like PSReadLine) may read it from
            // the input buffer and cancel the current line.
            let mut writer = self.writer.lock();
            writer
                .write_all(data)
                .map_err(|e| format!("Failed to write to pty: {}", e))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush pty: {}", e))?;
            return Ok(());
        }

        let mut writer = self.writer.lock();
        writer
            .write_all(data)
            .map_err(|e| format!("Failed to write to pty: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush pty: {}", e))?;
        Ok(())
    }

    /// Resize the PTY
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        let master = self.master.lock();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize pty: {}", e))
    }

    /// Check if the session is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the running flag for external monitoring (e.g., forwarding task).
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// Close the session
    pub fn close(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.is_attached_flag.store(false, Ordering::Relaxed);
        // Drop the output channel to notify attached clients
        *self.output_tx.lock() = None;
    }

    /// Get the session info for protocol messages
    pub fn info(&self) -> godly_protocol::SessionInfo {
        godly_protocol::SessionInfo {
            id: self.id.clone(),
            shell_type: self.shell_type.clone(),
            #[cfg(windows)]
            pid: self.pid,
            #[cfg(not(windows))]
            pid: 0,
            rows: self.rows,
            cols: self.cols,
            cwd: self.cwd.clone(),
            created_at: self.created_at,
            attached: self.is_attached(),
            running: self.is_running(),
        }
    }

    /// Get the current ring buffer size in bytes (for diagnostics and testing).
    #[cfg(test)]
    pub fn ring_buffer_len(&self) -> usize {
        self.ring_buffer.lock().len()
    }

    #[cfg(windows)]
    #[allow(dead_code)]
    pub fn get_pid(&self) -> u32 {
        self.pid
    }

    #[allow(dead_code)]
    pub fn get_shell_type(&self) -> &ShellType {
        &self.shell_type
    }
}

/// Terminate child processes of the given shell PID to simulate Ctrl+C.
///
/// ConPTY does not translate raw `\x03` written to its input pipe into
/// `CTRL_C_EVENT`, and `GenerateConsoleCtrlEvent` doesn't work with
/// pseudoconsoles either. As a workaround, we enumerate child processes
/// of the shell and terminate them, leaving the shell alive so it returns
/// to the prompt.
///
/// Returns the number of processes terminated.
#[cfg(windows)]
fn terminate_child_processes(shell_pid: u32) -> Result<u32, String> {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
    use winapi::um::tlhelp32::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
    };
    use winapi::um::winnt::PROCESS_TERMINATE;

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == winapi::um::handleapi::INVALID_HANDLE_VALUE {
        return Err("CreateToolhelp32Snapshot failed".to_string());
    }

    let mut entry: PROCESSENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

    // Collect all PIDs that are descendants of shell_pid
    let mut all_pids: Vec<(u32, u32)> = Vec::new(); // (pid, parent_pid)
    unsafe {
        if Process32First(snapshot, &mut entry) != 0 {
            loop {
                all_pids.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32Next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }

    // Find direct children of shell_pid (and their descendants)
    let mut targets: Vec<u32> = Vec::new();
    let mut queue: Vec<u32> = vec![shell_pid];
    while let Some(parent) = queue.pop() {
        for &(pid, ppid) in &all_pids {
            if ppid == parent && pid != shell_pid {
                targets.push(pid);
                queue.push(pid); // Also find grandchildren
            }
        }
    }

    if targets.is_empty() {
        return Ok(0);
    }

    // Terminate in reverse order (deepest children first)
    targets.reverse();
    let mut terminated = 0u32;
    for pid in &targets {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, *pid);
            if !handle.is_null() {
                // Exit code 0xC000013A = STATUS_CONTROL_C_EXIT (same as real Ctrl+C)
                if TerminateProcess(handle, 0xC000013Au32) != 0 {
                    terminated += 1;
                }
                CloseHandle(handle);
            }
        }
    }

    Ok(terminated)
}

/// Append data to ring buffer, evicting oldest data if necessary
fn append_to_ring(ring: &mut VecDeque<u8>, data: &[u8]) {
    // If data alone exceeds buffer, only keep the tail
    if data.len() >= RING_BUFFER_SIZE {
        ring.clear();
        ring.extend(&data[data.len() - RING_BUFFER_SIZE..]);
        return;
    }

    let needed = ring.len() + data.len();
    if needed > RING_BUFFER_SIZE {
        let to_remove = needed - RING_BUFFER_SIZE;
        ring.drain(..to_remove);
    }
    ring.extend(data);
}

/// Convert a Windows path to WSL path format (duplicated from utils for daemon independence)
fn windows_to_wsl_path(path: &str) -> String {
    let path = path.replace('\\', "/");

    // Handle WSL UNC paths: //wsl.localhost/<distro>/... or //wsl$/<distro>/...
    // These must be converted to native Linux paths by stripping the prefix and distro name.
    if path.starts_with("//wsl.localhost/") || path.starts_with("//wsl$/") {
        let after_host = if path.starts_with("//wsl.localhost/") {
            &path["//wsl.localhost/".len()..]
        } else {
            &path["//wsl$/".len()..]
        };
        // Skip the distro name (first path segment)
        return match after_host.find('/') {
            Some(idx) => {
                let linux_path = &after_host[idx..];
                if linux_path == "/" { "/".to_string() } else { linux_path.to_string() }
            }
            None => "/".to_string(),
        };
    }

    // Check for drive letter pattern: C:/...
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        let drive = path.as_bytes()[0].to_ascii_lowercase() as char;
        let rest = &path[2..];
        return format!("/mnt/{}{}", drive, rest);
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_to_ring_basic() {
        let mut ring = VecDeque::new();
        append_to_ring(&mut ring, b"hello");
        assert_eq!(ring.iter().copied().collect::<Vec<u8>>(), b"hello");
    }

    #[test]
    fn test_append_to_ring_eviction() {
        let mut ring = VecDeque::new();
        // Fill with data
        let data = vec![0u8; RING_BUFFER_SIZE];
        append_to_ring(&mut ring, &data);
        assert_eq!(ring.len(), RING_BUFFER_SIZE);

        // Append more - should evict oldest
        append_to_ring(&mut ring, b"new");
        assert_eq!(ring.len(), RING_BUFFER_SIZE);
        // Last 3 bytes should be "new"
        let tail: Vec<u8> = ring.iter().rev().take(3).rev().copied().collect();
        assert_eq!(tail, b"new");
    }

    #[test]
    fn test_windows_to_wsl_path() {
        assert_eq!(
            windows_to_wsl_path("C:\\Users\\test"),
            "/mnt/c/Users/test"
        );
        assert_eq!(windows_to_wsl_path("/already/unix"), "/already/unix");
    }

    // Bug: WSL UNC paths like \\wsl.localhost\Ubuntu\home\user\project are converted to
    // //wsl.localhost/Ubuntu/home/user/project instead of /home/user/project, causing
    // chdir() to fail with error 2 and the shell starts in / instead of the target dir.
    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_unc() {
        // \\wsl.localhost\<distro>\<path> should become /<path>
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\home\\alanm\\dev\\project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_unc_forward_slashes() {
        // Same path but with forward slashes (as it may arrive after partial normalization)
        assert_eq!(
            windows_to_wsl_path("//wsl.localhost/Ubuntu/home/alanm/dev/project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_dollar_unc() {
        // Legacy \\wsl$\<distro>\<path> format should also be handled
        assert_eq!(
            windows_to_wsl_path("\\\\wsl$\\Ubuntu\\home\\alanm\\dev\\project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_root() {
        // UNC path pointing to the distro root
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu"),
            "/"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_root_trailing_slash() {
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\"),
            "/"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_deep_path() {
        // Deep nesting with the exact path from the bug report
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\home\\alanm\\dev\\terraform-tests\\terraform-provider-typesense"),
            "/home/alanm/dev/terraform-tests/terraform-provider-typesense"
        );
    }

    // --- Tests for is_attached_flag (mutex starvation fix) ---
    //
    // Bug: handler threads blocked on is_attached()/info() because they locked
    // output_tx, which the reader thread held in a tight loop under heavy output.
    // Fix: is_attached_flag (AtomicBool) provides lock-free attachment state.

    #[cfg(windows)]
    #[test]
    fn test_attached_flag_lifecycle() {
        let session = DaemonSession::new(
            "test-lifecycle".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        assert!(!session.is_attached(), "new session should be detached");

        let (_buf, _rx) = session.attach();
        assert!(session.is_attached(), "should be attached after attach()");

        session.detach();
        assert!(!session.is_attached(), "should be detached after detach()");

        let (_buf2, _rx2) = session.attach();
        assert!(session.is_attached(), "should be attached after re-attach");

        session.close();
        assert!(!session.is_attached(), "should be detached after close()");
    }

    #[cfg(windows)]
    #[test]
    fn test_info_reflects_attachment_state() {
        let session = DaemonSession::new(
            "test-info".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        assert!(!session.info().attached);

        let (_buf, _rx) = session.attach();
        assert!(session.info().attached);

        session.detach();
        assert!(!session.info().attached);
    }

    /// Bug A1: When the per-session output channel fills during heavy output,
    /// data falls back to the ring buffer but is never replayed to the attached
    /// client. The ring buffer should remain empty while a client is attached —
    /// all data should flow through the channel.
    #[cfg(windows)]
    #[test]
    fn test_no_data_stranded_in_ring_buffer_while_attached() {
        let session = DaemonSession::new(
            "test-bp".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        let (_buf, mut rx) = session.attach();

        // Generate enough output to overflow the channel (capacity 64).
        // PowerShell prints numbered lines; with small PTY reads, this produces
        // many chunks that will fill the 64-message channel.
        session
            .write(b"for ($i = 1; $i -le 300; $i++) { Write-Output \"LINE$i\" }\r\n")
            .unwrap();

        // Wait for output to be produced — don't read from rx yet so the
        // channel fills up and triggers the Full path.
        thread::sleep(Duration::from_secs(3));

        // Now drain the channel, giving the reader thread time to catch up
        // after backpressure is relieved.
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut consecutive_empties = 0;
        loop {
            match rx.try_recv() {
                Ok(_) => {
                    consecutive_empties = 0;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    consecutive_empties += 1;
                    if consecutive_empties > 200 || Instant::now() > deadline {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        // Bug A1: ring buffer should be empty — all data should have flowed
        // through the channel, not fallen back to the ring buffer.
        let ring_size = session.ring_buffer_len();
        assert_eq!(
            ring_size, 0,
            "Bug A1: {} bytes stranded in ring buffer while client was attached \
             — data lost during channel backpressure",
            ring_size
        );

        session.close();
    }

    #[cfg(windows)]
    #[test]
    fn test_is_attached_does_not_block_on_output_tx() {
        // Bug: is_attached() locked output_tx, causing handler starvation when the
        // reader thread held that lock under heavy output. The fix uses AtomicBool
        // so is_attached() never contends with the reader thread.
        let session = DaemonSession::new(
            "test-lockfree".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        let output_tx = session.output_tx.clone();

        // Hold output_tx locked for 500ms on a background thread
        let handle = thread::spawn(move || {
            let _guard = output_tx.lock();
            thread::sleep(Duration::from_millis(500));
        });

        // Give the background thread time to acquire the lock
        thread::sleep(Duration::from_millis(50));

        // is_attached() should return immediately (lock-free via AtomicBool)
        let start = Instant::now();
        let attached = session.is_attached();
        let elapsed = start.elapsed();

        assert!(!attached);
        assert!(
            elapsed.as_millis() < 50,
            "is_attached() took {}ms — should be lock-free, not blocked on output_tx",
            elapsed.as_millis()
        );

        handle.join().unwrap();
    }

    // --- Tests for running flag (SessionClosed on PTY exit) ---
    //
    // Bug: when a shell process exited, the daemon never notified anyone. The
    // session stayed in the HashMap, the terminal tab looked frozen, and the
    // user could type but nothing happened.
    // Fix: reader thread sets running=false on EOF, enabling the forwarding
    // task to detect PTY death and send SessionClosed.

    #[cfg(windows)]
    #[test]
    fn test_close_sets_running_false() {
        // Bug: PTY exit left session marked as running, so dead sessions were
        // invisible to ListSessions and reattached on reconnect.
        let session = DaemonSession::new(
            "test-running".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        assert!(session.is_running(), "new session should be running");

        session.close();
        assert!(!session.is_running(), "session should not be running after close()");
    }

    #[cfg(windows)]
    #[test]
    fn test_info_reflects_running_state() {
        // Bug: SessionInfo had no `running` field, so clients couldn't
        // distinguish dead sessions from live ones via ListSessions.
        let session = DaemonSession::new(
            "test-info-running".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        assert!(session.info().running, "new session info should show running=true");

        session.close();
        assert!(!session.info().running, "closed session info should show running=false");
    }

    #[cfg(windows)]
    #[test]
    fn test_running_flag_is_shared() {
        // The forwarding task needs to read the running flag after the channel
        // closes. Verify running_flag() returns the same Arc as is_running().
        let session = DaemonSession::new(
            "test-flag-shared".into(),
            ShellType::Windows,
            None,
            24,
            80,
            None,
        )
        .unwrap();

        let flag = session.running_flag();
        assert!(flag.load(Ordering::Relaxed));

        session.close();
        assert!(!flag.load(Ordering::Relaxed), "running_flag should reflect close()");
    }
}
