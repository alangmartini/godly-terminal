use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use godly_protocol::types::ShellType;
use godly_protocol::{
    read_shim_frame, write_shim_binary, write_shim_json, ShimFrame, ShimMetadata, ShimRequest,
    ShimResponse, TAG_SHIM_BUFFER_DATA, TAG_SHIM_OUTPUT, TAG_SHIM_WRITE,
};

use crate::debug_log::daemon_log;
use crate::shim_client;
use crate::shim_metadata;

const RING_BUFFER_SIZE: usize = 1024 * 1024; // 1MB ring buffer

/// Output from the PTY reader thread to the forwarding task.
/// RawBytes is the PTY output data; GridDiff is a pushed diff for the frontend.
pub enum SessionOutput {
    RawBytes(Vec<u8>),
    GridDiff(godly_protocol::types::RichGridDiff),
    Bell,
}

/// A Write wrapper that sends TAG_SHIM_WRITE frames to the shim pipe.
/// Each write() call produces a complete length-prefixed binary frame.
struct ShimPipeWriter {
    inner: std::fs::File,
}

impl Write for ShimPipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        write_shim_binary(&mut self.inner, TAG_SHIM_WRITE, buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // Named pipes deliver data immediately; no explicit flush needed
        Ok(())
    }
}

/// A daemon-managed PTY session backed by a pty-shim process.
///
/// The shim holds the actual ConPTY handles and shell process. The daemon
/// communicates with the shim via a named pipe, sending input and receiving
/// PTY output. This architecture allows the terminal session to survive
/// daemon crashes -- the shim keeps running and buffers output until a new
/// daemon instance reconnects.
pub struct DaemonSession {
    pub id: String,
    pub shell_type: ShellType,
    pub cwd: Option<String>,
    pub created_at: u64,
    pub rows: u16,
    pub cols: u16,
    #[cfg(windows)]
    pub pid: u32,
    /// PID of the pty-shim process
    #[allow(dead_code)]
    shim_pid: u32,
    /// Named pipe name for shim communication
    #[allow(dead_code)]
    shim_pipe_name: String,
    /// Writer to the shim pipe for user input (sends TAG_SHIM_WRITE binary frames)
    shim_writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Raw pipe handle for sending JSON control messages (Resize, Shutdown, Status).
    /// Separate from shim_writer because ShimPipeWriter wraps all data as binary
    /// frames with TAG_SHIM_WRITE, but control messages must be sent as plain JSON.
    shim_control: Arc<Mutex<std::fs::File>>,
    running: Arc<AtomicBool>,
    /// Ring buffer accumulates output when no client is attached
    ring_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Channel sender for live output to an attached client
    output_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<SessionOutput>>>>,
    /// Lock-free attachment flag -- readable without locking output_tx.
    /// Updated in attach()/detach()/close() and by the reader thread on send failure.
    is_attached_flag: Arc<AtomicBool>,
    /// Always-on output history buffer -- captures all PTY output regardless of
    /// attachment state. Used by the ReadBuffer command to let MCP clients read
    /// terminal output without attaching.
    output_history: Arc<Mutex<VecDeque<u8>>>,
    /// Epoch ms of the last PTY output. Used by WaitForIdle to detect when
    /// terminal output has stopped.
    last_output_epoch_ms: Arc<AtomicU64>,
    /// godly-vt terminal state engine -- parses all PTY output and maintains an
    /// in-memory grid. Used by ReadGrid to provide clean, parsed terminal content
    /// without ANSI escape stripping.
    vt_parser: Arc<Mutex<godly_vt::Parser>>,
    /// Exit code from the child process. Set by the reader thread when
    /// ShellExited is received from the shim. i64::MIN means "not yet exited" (sentinel).
    exit_code: Arc<AtomicI64>,
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
        // Spawn a pty-shim process that holds the ConPTY and shell
        let meta = shim_client::spawn_shim(
            &id,
            &shell_type,
            cwd.as_deref(),
            rows,
            cols,
            env.as_ref(),
        )?;

        // Connect to the shim's named pipe
        let shim_pipe = shim_client::connect_to_shim(&meta.shim_pipe_name)?;

        // Duplicate handles for separate reader, writer (binary input), and control (JSON)
        let pipe_reader = shim_client::duplicate_handle(&shim_pipe)
            .map_err(|e| format!("Failed to duplicate shim pipe handle for reader: {}", e))?;
        let pipe_control = shim_client::duplicate_handle(&shim_pipe)
            .map_err(|e| format!("Failed to duplicate shim pipe handle for control: {}", e))?;
        let pipe_writer = shim_pipe; // Original handle goes to the binary writer

        // Query status to get the shell PID (use control handle for JSON messages)
        let mut status_control = shim_client::duplicate_handle(&pipe_control)
            .map_err(|e| format!("Failed to duplicate handle for status query: {}", e))?;
        write_shim_json(&mut status_control, &ShimRequest::Status)
            .map_err(|e| format!("Failed to send Status request to shim: {}", e))?;

        // Read the status response. The shim may first send buffered data (TAG_SHIM_BUFFER_DATA)
        // from a previous daemon connection, or early PTY output (shell prompt), then StatusInfo.
        // We capture any early output so it isn't lost.
        let mut temp_reader = shim_client::duplicate_handle(&pipe_reader)
            .map_err(|e| format!("Failed to duplicate handle for status read: {}", e))?;
        let (shell_pid, early_output) = Self::read_status_response(&mut temp_reader)?;

        daemon_log!(
            "Session {} connected to shim: shim_pid={}, shell_pid={}, pipe={}, early_output={}B",
            id,
            meta.shim_pid,
            shell_pid,
            meta.shim_pipe_name,
            early_output.len()
        );

        // Write metadata file for daemon restart recovery
        let mut final_meta = meta.clone();
        final_meta.shell_pid = shell_pid;
        if let Err(e) = shim_metadata::write_metadata(&final_meta) {
            daemon_log!(
                "Warning: failed to write shim metadata for session {}: {}",
                id,
                e
            );
        }

        let shim_writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(Box::new(ShimPipeWriter { inner: pipe_writer })));
        let shim_control = Arc::new(Mutex::new(pipe_control));
        let running = Arc::new(AtomicBool::new(true));
        let ring_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_SIZE)));
        let output_history = Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_SIZE)));
        let output_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<SessionOutput>>>> =
            Arc::new(Mutex::new(None));
        let is_attached_flag = Arc::new(AtomicBool::new(false));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_output_epoch_ms = Arc::new(AtomicU64::new(now_ms));

        let vt_parser = Arc::new(Mutex::new(godly_vt::Parser::new(rows, cols, 10_000)));
        let exit_code = Arc::new(AtomicI64::new(i64::MIN));

        // Feed any early output (captured during status query) into ring buffer,
        // output history, and VT parser so the initial shell prompt is preserved.
        if !early_output.is_empty() {
            append_to_ring(&mut ring_buffer.lock(), &early_output);
            append_to_ring(&mut output_history.lock(), &early_output);
            vt_parser.lock().process(&early_output);
        }

        // Spawn reader thread that reads frames from the shim pipe
        Self::spawn_reader_thread(
            id.clone(),
            pipe_reader,
            running.clone(),
            ring_buffer.clone(),
            output_history.clone(),
            output_tx.clone(),
            is_attached_flag.clone(),
            last_output_epoch_ms.clone(),
            vt_parser.clone(),
            exit_code.clone(),
        );

        Ok(Self {
            id,
            shell_type,
            cwd,
            created_at: now,
            rows,
            cols,
            #[cfg(windows)]
            pid: shell_pid,
            shim_pid: meta.shim_pid,
            shim_pipe_name: meta.shim_pipe_name,
            shim_writer,
            shim_control,
            running,
            ring_buffer,
            output_tx,
            is_attached_flag,
            output_history,
            last_output_epoch_ms,
            vt_parser,
            exit_code,
        })
    }

    /// Reconnect to a surviving pty-shim after daemon restart.
    /// Connects to the shim pipe, drains buffered output, and sets up reader/writer.
    pub fn reconnect(meta: ShimMetadata) -> Result<Self, String> {
        daemon_log!(
            "Reconnecting to shim for session {}: pipe={}",
            meta.session_id,
            meta.shim_pipe_name
        );

        // Connect to shim pipe
        let shim_pipe = shim_client::connect_to_shim(&meta.shim_pipe_name)?;

        // Duplicate for reader, writer (binary input), and control (JSON)
        let pipe_reader = shim_client::duplicate_handle(&shim_pipe)
            .map_err(|e| format!("Failed to duplicate shim pipe handle for reader: {}", e))?;
        let pipe_control = shim_client::duplicate_handle(&shim_pipe)
            .map_err(|e| format!("Failed to duplicate shim pipe handle for control: {}", e))?;
        let pipe_writer = shim_pipe;

        // Query status to verify the shim is alive and get current state
        let mut status_control = shim_client::duplicate_handle(&pipe_control)
            .map_err(|e| format!("Failed to duplicate handle for status query: {}", e))?;
        write_shim_json(&mut status_control, &ShimRequest::Status)
            .map_err(|e| format!("Failed to send Status request to shim: {}", e))?;

        // Read status response (may be preceded by buffered data)
        let mut temp_reader = shim_client::duplicate_handle(&pipe_reader)
            .map_err(|e| format!("Failed to duplicate handle for status read: {}", e))?;

        // Read all frames until we get the StatusInfo response.
        // Buffer data frames go into the vt parser.
        let vt_parser = Arc::new(Mutex::new(godly_vt::Parser::new(
            meta.rows, meta.cols, 10_000,
        )));
        let output_history = Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_SIZE)));

        let mut shell_pid = meta.shell_pid;
        #[allow(unused_assignments)]
        let mut is_running = true;
        let mut exit_code_val = i64::MIN;

        // Read frames from the shim: it will first send any buffered output,
        // then the StatusInfo response.
        loop {
            match read_shim_frame(&mut temp_reader) {
                Ok(Some(ShimFrame::Binary {
                    tag: TAG_SHIM_BUFFER_DATA,
                    data,
                })) => {
                    daemon_log!(
                        "Session {} reconnect: received {} bytes of buffered data",
                        meta.session_id,
                        data.len()
                    );
                    // Feed buffered data into vt parser
                    vt_parser.lock().process(&data);
                    append_to_ring(&mut output_history.lock(), &data);
                }
                Ok(Some(ShimFrame::Binary {
                    tag: TAG_SHIM_OUTPUT,
                    data,
                })) => {
                    // Output that arrived between our connect and status query
                    vt_parser.lock().process(&data);
                    append_to_ring(&mut output_history.lock(), &data);
                }
                Ok(Some(ShimFrame::Json(json_bytes))) => {
                    if let Ok(resp) = serde_json::from_slice::<ShimResponse>(&json_bytes) {
                        match resp {
                            ShimResponse::StatusInfo {
                                shell_pid: pid,
                                running,
                                rows: _,
                                cols: _,
                            } => {
                                shell_pid = pid;
                                is_running = running;
                                daemon_log!(
                                    "Session {} reconnect: shell_pid={}, running={}",
                                    meta.session_id,
                                    shell_pid,
                                    is_running
                                );
                                break;
                            }
                            ShimResponse::ShellExited { exit_code } => {
                                is_running = false;
                                if let Some(code) = exit_code {
                                    exit_code_val = code;
                                }
                                daemon_log!(
                                    "Session {} reconnect: shell already exited (exit_code={:?})",
                                    meta.session_id,
                                    exit_code
                                );
                                break;
                            }
                        }
                    } else {
                        daemon_log!(
                            "Session {} reconnect: unrecognized JSON frame, skipping",
                            meta.session_id
                        );
                    }
                }
                Ok(Some(ShimFrame::Binary { tag, .. })) => {
                    daemon_log!(
                        "Session {} reconnect: unexpected binary tag 0x{:02X}, skipping",
                        meta.session_id,
                        tag
                    );
                }
                Ok(None) => {
                    return Err(format!(
                        "Shim pipe EOF during reconnect for session {}",
                        meta.session_id
                    ));
                }
                Err(e) => {
                    return Err(format!(
                        "Shim pipe read error during reconnect for session {}: {}",
                        meta.session_id, e
                    ));
                }
            }
        }

        let shim_writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(Box::new(ShimPipeWriter { inner: pipe_writer })));
        let shim_control = Arc::new(Mutex::new(pipe_control));
        let running = Arc::new(AtomicBool::new(is_running));
        let ring_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_SIZE)));
        let output_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<SessionOutput>>>> =
            Arc::new(Mutex::new(None));
        let is_attached_flag = Arc::new(AtomicBool::new(false));

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_output_epoch_ms = Arc::new(AtomicU64::new(now_ms));

        let exit_code = Arc::new(AtomicI64::new(exit_code_val));

        // Spawn reader thread for ongoing shim communication
        if is_running {
            Self::spawn_reader_thread(
                meta.session_id.clone(),
                pipe_reader,
                running.clone(),
                ring_buffer.clone(),
                output_history.clone(),
                output_tx.clone(),
                is_attached_flag.clone(),
                last_output_epoch_ms.clone(),
                vt_parser.clone(),
                exit_code.clone(),
            );
        }

        // Update metadata file with current shell_pid
        let mut updated_meta = meta.clone();
        updated_meta.shell_pid = shell_pid;
        if let Err(e) = shim_metadata::write_metadata(&updated_meta) {
            daemon_log!(
                "Warning: failed to update shim metadata for session {}: {}",
                meta.session_id,
                e
            );
        }

        Ok(Self {
            id: meta.session_id,
            shell_type: meta.shell_type,
            cwd: meta.cwd,
            created_at: meta.created_at,
            rows: meta.rows,
            cols: meta.cols,
            #[cfg(windows)]
            pid: shell_pid,
            shim_pid: meta.shim_pid,
            shim_pipe_name: meta.shim_pipe_name,
            shim_writer,
            shim_control,
            running,
            ring_buffer,
            output_tx,
            is_attached_flag,
            output_history,
            last_output_epoch_ms,
            vt_parser,
            exit_code,
        })
    }

    /// Read the StatusInfo response from the shim, handling any preceding
    /// buffer data or output frames that arrive first.
    /// Returns (shell_pid, early_output) — early_output contains any PTY output
    /// that arrived before the StatusInfo response, which must be fed to the
    /// ring buffer and VT parser to avoid losing the initial shell prompt.
    fn read_status_response(reader: &mut std::fs::File) -> Result<(u32, Vec<u8>), String> {
        let mut early_output = Vec::new();
        loop {
            match read_shim_frame(reader) {
                Ok(Some(ShimFrame::Binary {
                    tag: TAG_SHIM_BUFFER_DATA,
                    data,
                })) => {
                    // Buffer data from previous connection — save for replay
                    early_output.extend_from_slice(&data);
                    continue;
                }
                Ok(Some(ShimFrame::Binary {
                    tag: TAG_SHIM_OUTPUT,
                    data,
                })) => {
                    // Early PTY output (shell prompt etc.) — save for replay
                    early_output.extend_from_slice(&data);
                    continue;
                }
                Ok(Some(ShimFrame::Json(json_bytes))) => {
                    if let Ok(ShimResponse::StatusInfo { shell_pid, .. }) =
                        serde_json::from_slice::<ShimResponse>(&json_bytes)
                    {
                        return Ok((shell_pid, early_output));
                    }
                    if let Ok(ShimResponse::ShellExited { .. }) =
                        serde_json::from_slice::<ShimResponse>(&json_bytes)
                    {
                        // Shell already exited before we connected
                        return Ok((0, early_output));
                    }
                    // Unknown JSON, keep reading
                    continue;
                }
                Ok(Some(ShimFrame::Binary { .. })) => continue,
                Ok(None) => return Err("Shim pipe EOF while waiting for StatusInfo".to_string()),
                Err(e) => {
                    return Err(format!("Shim pipe error while waiting for StatusInfo: {}", e))
                }
            }
        }
    }

    /// Spawn the reader thread that reads frames from the shim pipe and
    /// dispatches them (output to vt_parser/ring buffer/client, shell exit to cleanup).
    fn spawn_reader_thread(
        session_id: String,
        mut pipe_reader: std::fs::File,
        reader_running: Arc<AtomicBool>,
        reader_ring: Arc<Mutex<VecDeque<u8>>>,
        reader_history: Arc<Mutex<VecDeque<u8>>>,
        reader_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<SessionOutput>>>>,
        reader_attached: Arc<AtomicBool>,
        reader_last_output: Arc<AtomicU64>,
        reader_vt: Arc<Mutex<godly_vt::Parser>>,
        reader_exit_code: Arc<AtomicI64>,
    ) {
        thread::spawn(move || {
            daemon_log!("Session {} shim reader thread started", session_id);

            let mut total_bytes: u64 = 0;
            let mut total_reads: u64 = 0;
            let mut channel_send_failures: u64 = 0;
            let mut last_stats = Instant::now();
            let mut last_diff_time = Instant::now();
            const DIFF_INTERVAL: Duration = Duration::from_millis(16);

            while reader_running.load(Ordering::Relaxed) {
                match read_shim_frame(&mut pipe_reader) {
                    Ok(Some(ShimFrame::Binary {
                        tag: TAG_SHIM_OUTPUT,
                        data,
                    })) => {
                        let n = data.len();
                        total_bytes += n as u64;
                        total_reads += 1;

                        // Update last output timestamp for idle detection
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        reader_last_output.store(now_ms, Ordering::Relaxed);

                        // Always append to output_history for ReadBuffer access
                        {
                            let mut history = reader_history.lock();
                            append_to_ring(&mut history, &data);
                        }

                        // Feed PTY output into godly-vt parser for grid state.
                        let (maybe_diff, bell_fired) = {
                            let mut vt = reader_vt.lock();
                            vt.process(&data);
                            let bell = vt.take_bell_pending();
                            let now = Instant::now();
                            let diff = if reader_attached.load(Ordering::Relaxed)
                                && vt.screen().has_dirty_rows()
                                && now.duration_since(last_diff_time) >= DIFF_INTERVAL
                            {
                                last_diff_time = now;
                                Some(extract_diff(&mut vt))
                            } else {
                                None
                            };
                            (diff, bell)
                        };

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
                            // Client attached: try to send live output
                            match tx.try_send(SessionOutput::RawBytes(data.clone())) {
                                Ok(()) => {
                                    if let Some(diff) = maybe_diff {
                                        let _ = tx.try_send(SessionOutput::GridDiff(diff));
                                    }
                                    if bell_fired {
                                        let _ = tx.try_send(SessionOutput::Bell);
                                    }
                                    drop(tx_guard);
                                    thread::yield_now();
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Full(msg)) => {
                                    let tx_clone = tx.clone();
                                    drop(tx_guard);
                                    let bp_start = Instant::now();
                                    match tx_clone.blocking_send(msg) {
                                        Ok(()) => {
                                            let bp_elapsed = bp_start.elapsed();
                                            if bp_elapsed.as_millis() > 50 {
                                                daemon_log!(
                                                    "Session {} reader: backpressure {:.1}ms (channel was full)",
                                                    session_id,
                                                    bp_elapsed.as_secs_f64() * 1000.0
                                                );
                                            }
                                            if let Some(diff) = maybe_diff {
                                                let _ =
                                                    tx_clone.try_send(SessionOutput::GridDiff(diff));
                                            }
                                            if bell_fired {
                                                let _ = tx_clone.try_send(SessionOutput::Bell);
                                            }
                                            thread::yield_now();
                                        }
                                        Err(send_err) => {
                                            channel_send_failures += 1;
                                            daemon_log!(
                                                "Session {} reader: channel closed during backpressure (disconnect #{})",
                                                session_id,
                                                channel_send_failures
                                            );
                                            reader_attached.store(false, Ordering::Relaxed);
                                            *reader_tx.lock() = None;
                                            let mut ring = reader_ring.lock();
                                            if let SessionOutput::RawBytes(data) = send_err.0 {
                                                append_to_ring(&mut ring, &data);
                                            }
                                        }
                                    }
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                    channel_send_failures += 1;
                                    daemon_log!(
                                        "Session {} reader: channel send failed (client disconnect #{})",
                                        session_id,
                                        channel_send_failures
                                    );
                                    drop(tx_guard);
                                    reader_attached.store(false, Ordering::Relaxed);
                                    *reader_tx.lock() = None;
                                    let mut ring = reader_ring.lock();
                                    append_to_ring(&mut ring, &data);
                                }
                            }
                        } else {
                            // No client attached: buffer output
                            drop(tx_guard);
                            let mut ring = reader_ring.lock();
                            append_to_ring(&mut ring, &data);
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
                    Ok(Some(ShimFrame::Binary {
                        tag: TAG_SHIM_BUFFER_DATA,
                        data,
                    })) => {
                        // Buffer data from shim (replayed on reconnect).
                        // Feed into vt parser and output history.
                        daemon_log!(
                            "Session {} reader: received {} bytes of buffer replay data",
                            session_id,
                            data.len()
                        );
                        {
                            let mut vt = reader_vt.lock();
                            vt.process(&data);
                        }
                        {
                            let mut history = reader_history.lock();
                            append_to_ring(&mut history, &data);
                        }
                    }
                    Ok(Some(ShimFrame::Json(json_bytes))) => {
                        // Try to parse as ShimResponse
                        if let Ok(resp) = serde_json::from_slice::<ShimResponse>(&json_bytes) {
                            match resp {
                                ShimResponse::ShellExited { exit_code } => {
                                    daemon_log!(
                                        "Session {} reader: ShellExited (exit_code={:?})",
                                        session_id,
                                        exit_code
                                    );
                                    if let Some(code) = exit_code {
                                        reader_exit_code.store(code, Ordering::Relaxed);
                                    }
                                    reader_running.store(false, Ordering::Relaxed);
                                    reader_attached.store(false, Ordering::Relaxed);
                                    *reader_tx.lock() = None;
                                    break;
                                }
                                ShimResponse::StatusInfo { .. } => {
                                    // Ignore unsolicited status info
                                }
                            }
                        }
                    }
                    Ok(Some(ShimFrame::Binary { tag, .. })) => {
                        daemon_log!(
                            "Session {} reader: unexpected shim binary tag: 0x{:02X}",
                            session_id,
                            tag
                        );
                    }
                    Ok(None) => {
                        // Shim pipe EOF -- shim process died
                        daemon_log!("Session {} reader: shim pipe EOF", session_id);
                        break;
                    }
                    Err(e) => {
                        daemon_log!("Session {} reader: shim read error: {}", session_id, e);
                        break;
                    }
                }
            }

            // Shim disconnected or shell exited -- mark session as dead and close output channel
            reader_running.store(false, Ordering::Relaxed);
            reader_attached.store(false, Ordering::Relaxed);
            *reader_tx.lock() = None;

            daemon_log!(
                "Session {} shim reader thread exited: reads={}, bytes={}, send_failures={}",
                session_id,
                total_reads,
                total_bytes,
                channel_send_failures
            );
            eprintln!("[daemon] Session {} shim reader thread exited", session_id);
        });
    }

    /// Attach a client to this session.
    /// Returns (buffered_data, receiver_for_live_output).
    ///
    /// Uses `try_lock_for` with timeouts to avoid blocking the handler indefinitely
    /// when the reader thread holds ring_buffer or output_tx under heavy output.
    pub fn attach(&self) -> (Vec<u8>, tokio::sync::mpsc::Receiver<SessionOutput>) {
        let (tx, rx) = tokio::sync::mpsc::channel(64);

        // Drain ring buffer as initial replay -- timeout to avoid blocking handler
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

        // Set the output sender for live streaming -- timeout to avoid blocking handler
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
    pub fn is_attached(&self) -> bool {
        self.is_attached_flag.load(Ordering::Relaxed)
    }

    /// Write data to the PTY via the shim pipe.
    ///
    /// On Windows, raw `\x03` (Ctrl+C) written to ConPTY's input pipe does NOT
    /// generate `CTRL_C_EVENT` for child processes. To interrupt a running process,
    /// we detect `\x03` and terminate child processes of the shell, leaving the
    /// shell itself alive so the user gets a fresh prompt.
    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        daemon_log!(
            "Session {} write: {} bytes (first={:?})",
            self.id,
            data.len(),
            &data[..data.len().min(20)]
        );

        #[cfg(windows)]
        if data.contains(&0x03) {
            match terminate_child_processes(self.pid) {
                Ok(count) => {
                    if count > 0 {
                        daemon_log!(
                            "Session {} Ctrl+C: terminated {} child process(es) of shell pid {}",
                            self.id,
                            count,
                            self.pid
                        );
                    }
                }
                Err(e) => {
                    daemon_log!("Session {} Ctrl+C failed: {}", self.id, e);
                }
            }
            // Also write \x03 to the shim -- some shells (like PSReadLine) may
            // read it from the input buffer and cancel the current line.
            let mut writer = self.shim_writer.lock();
            writer
                .write_all(data)
                .map_err(|e| format!("Failed to write to shim: {}", e))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush shim: {}", e))?;
            return Ok(());
        }

        let mut writer = self.shim_writer.lock();
        writer
            .write_all(data)
            .map_err(|e| format!("Failed to write to shim: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush shim: {}", e))?;
        Ok(())
    }

    /// Resize the PTY via a control message to the shim.
    /// Also updates the godly-vt parser dimensions.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        // Send Resize request to the shim via the control handle (JSON, not binary)
        let mut control = self.shim_control.lock();
        write_shim_json(&mut *control, &ShimRequest::Resize { rows, cols })
            .map_err(|e| format!("Failed to send resize to shim: {}", e))?;
        drop(control);

        // Keep the godly-vt parser in sync with the PTY size
        {
            let mut vt = self.vt_parser.lock();
            vt.screen_mut().set_size(rows, cols);
        }

        Ok(())
    }

    /// Set the scrollback viewport offset.
    /// offset=0 means live view, offset>0 scrolls into history.
    pub fn set_scrollback(&self, offset: usize) {
        let mut vt = self.vt_parser.lock();
        vt.screen_mut().set_scrollback(offset);
    }

    /// Check if the session is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get the exit code of the child process, if it has exited.
    /// Returns None if the process hasn't exited yet or exit code is unavailable.
    pub fn exit_code(&self) -> Option<i64> {
        let code = self.exit_code.load(Ordering::Relaxed);
        if code == i64::MIN {
            None
        } else {
            Some(code)
        }
    }

    /// Get a clone of the exit_code Arc for use in async forwarding tasks.
    pub fn exit_code_arc(&self) -> Arc<AtomicI64> {
        self.exit_code.clone()
    }

    /// Get the running flag for external monitoring (e.g., forwarding task).
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// Close the session. Sends shutdown to the shim and removes metadata.
    pub fn close(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.is_attached_flag.store(false, Ordering::Relaxed);
        // Drop the output channel to notify attached clients
        *self.output_tx.lock() = None;

        // Tell the shim to shut down gracefully via the control handle
        if let Ok(mut control) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.shim_control.lock()
        })) {
            let _ = write_shim_json(&mut *control, &ShimRequest::Shutdown);
        }

        // Remove metadata file
        shim_metadata::remove_metadata(&self.id);
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

    /// Read the output history buffer (non-destructive).
    /// Returns all captured PTY output up to the 1MB rolling limit.
    pub fn read_output_history(&self) -> Vec<u8> {
        self.output_history.lock().iter().copied().collect()
    }

    /// Get the epoch ms of the last PTY output.
    pub fn last_output_epoch_ms(&self) -> u64 {
        self.last_output_epoch_ms.load(Ordering::Relaxed)
    }

    /// Get the current epoch ms (helper for callers computing idle time).
    #[allow(dead_code)]
    pub fn current_epoch_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Read the godly-vt grid state as a GridData snapshot.
    /// Returns plain-text rows (no ANSI escapes) plus cursor position.
    pub fn read_grid(&self) -> godly_protocol::GridData {
        let vt = self.vt_parser.lock();
        let screen = vt.screen();
        let (num_rows, cols) = screen.size();
        let (cursor_row, cursor_col) = screen.cursor_position();
        let rows: Vec<String> = screen.rows(0, cols).collect();
        godly_protocol::GridData {
            rows,
            cursor_row,
            cursor_col,
            cols,
            num_rows,
            alternate_screen: screen.alternate_screen(),
        }
    }

    /// Read rich grid snapshot with per-cell attributes for Canvas2D rendering.
    pub fn read_rich_grid(&self) -> godly_protocol::types::RichGridData {
        let vt = self.vt_parser.lock();
        let screen = vt.screen();
        let (num_rows, cols) = screen.size();
        let (cursor_row, cursor_col) = screen.cursor_position();

        let mut rows = Vec::with_capacity(usize::from(num_rows));
        for row_idx in 0..num_rows {
            let mut cells = Vec::with_capacity(usize::from(cols));
            for col_idx in 0..cols {
                let cell = screen.cell(row_idx, col_idx);
                match cell {
                    Some(c) => {
                        cells.push(godly_protocol::types::RichGridCell {
                            content: c.contents().to_string(),
                            fg: color_to_hex(c.fgcolor()),
                            bg: color_to_hex(c.bgcolor()),
                            bold: c.bold(),
                            dim: c.dim(),
                            italic: c.italic(),
                            underline: c.underline(),
                            inverse: c.inverse(),
                            wide: c.is_wide(),
                            wide_continuation: c.is_wide_continuation(),
                        });
                    }
                    None => {
                        cells.push(godly_protocol::types::RichGridCell {
                            content: String::new(),
                            fg: "default".to_string(),
                            bg: "default".to_string(),
                            bold: false,
                            dim: false,
                            italic: false,
                            underline: false,
                            inverse: false,
                            wide: false,
                            wide_continuation: false,
                        });
                    }
                }
            }
            let wrapped = screen.row_wrapped(row_idx);
            rows.push(godly_protocol::types::RichGridRow { cells, wrapped });
        }

        godly_protocol::types::RichGridData {
            rows,
            cursor: godly_protocol::types::CursorState {
                row: cursor_row,
                col: cursor_col,
            },
            dimensions: godly_protocol::types::GridDimensions {
                rows: num_rows,
                cols,
            },
            alternate_screen: screen.alternate_screen(),
            cursor_hidden: screen.hide_cursor(),
            title: screen.window_title().to_string(),
            scrollback_offset: screen.scrollback(),
            total_scrollback: screen.scrollback_count(),
        }
    }

    /// Read differential rich grid snapshot: only rows that changed since last call.
    /// Falls back to full repaint when >=50% of rows are dirty (e.g. after scroll).
    pub fn read_rich_grid_diff(&self) -> godly_protocol::types::RichGridDiff {
        let mut vt = self.vt_parser.lock();
        let screen = vt.screen_mut();
        let (num_rows, cols) = screen.size();
        let (cursor_row, cursor_col) = screen.cursor_position();

        let dirty_flags = screen.take_dirty_rows();
        let dirty_count = dirty_flags.iter().filter(|&&d| d).count();
        let total_rows = usize::from(num_rows);
        let full_repaint = dirty_count * 2 >= total_rows;

        // Read the screen immutably now that we've taken dirty flags
        let screen = vt.screen();
        let mut dirty_rows =
            Vec::with_capacity(if full_repaint { total_rows } else { dirty_count });

        for row_idx in 0..num_rows {
            if full_repaint || dirty_flags.get(usize::from(row_idx)).copied().unwrap_or(false) {
                let mut cells = Vec::with_capacity(usize::from(cols));
                for col_idx in 0..cols {
                    let cell = screen.cell(row_idx, col_idx);
                    match cell {
                        Some(c) => {
                            cells.push(godly_protocol::types::RichGridCell {
                                content: c.contents().to_string(),
                                fg: color_to_hex(c.fgcolor()),
                                bg: color_to_hex(c.bgcolor()),
                                bold: c.bold(),
                                dim: c.dim(),
                                italic: c.italic(),
                                underline: c.underline(),
                                inverse: c.inverse(),
                                wide: c.is_wide(),
                                wide_continuation: c.is_wide_continuation(),
                            });
                        }
                        None => {
                            cells.push(godly_protocol::types::RichGridCell {
                                content: String::new(),
                                fg: "default".to_string(),
                                bg: "default".to_string(),
                                bold: false,
                                dim: false,
                                italic: false,
                                underline: false,
                                inverse: false,
                                wide: false,
                                wide_continuation: false,
                            });
                        }
                    }
                }
                let wrapped = screen.row_wrapped(row_idx);
                dirty_rows.push((
                    row_idx,
                    godly_protocol::types::RichGridRow { cells, wrapped },
                ));
            }
        }

        godly_protocol::types::RichGridDiff {
            dirty_rows,
            cursor: godly_protocol::types::CursorState {
                row: cursor_row,
                col: cursor_col,
            },
            dimensions: godly_protocol::types::GridDimensions {
                rows: num_rows,
                cols,
            },
            alternate_screen: screen.alternate_screen(),
            cursor_hidden: screen.hide_cursor(),
            title: screen.window_title().to_string(),
            scrollback_offset: screen.scrollback(),
            total_scrollback: screen.scrollback_count(),
            full_repaint,
        }
    }

    /// Read text between two grid positions (for selection/copy).
    pub fn read_grid_text(
        &self,
        start_row: u16,
        start_col: u16,
        end_row: u16,
        end_col: u16,
    ) -> String {
        let vt = self.vt_parser.lock();
        let screen = vt.screen();
        screen.contents_between(start_row, start_col, end_row, end_col)
    }

    /// Search the output history for a text string.
    /// Optionally strips ANSI escape sequences before matching.
    pub fn search_output_history(&self, text: &str, do_strip_ansi: bool) -> bool {
        let data = self
            .output_history
            .lock()
            .iter()
            .copied()
            .collect::<Vec<u8>>();
        let haystack = String::from_utf8_lossy(&data);
        if do_strip_ansi {
            godly_protocol::ansi::strip_ansi(&haystack).contains(text)
        } else {
            haystack.contains(text)
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

/// Extract a RichGridDiff from the current godly-vt parser state.
/// Called from the PTY reader thread while holding the vt lock.
fn extract_diff(vt: &mut godly_vt::Parser) -> godly_protocol::types::RichGridDiff {
    let screen = vt.screen_mut();
    let (num_rows, cols) = screen.size();
    let (cursor_row, cursor_col) = screen.cursor_position();

    let dirty_flags = screen.take_dirty_rows();
    let dirty_count = dirty_flags.iter().filter(|&&d| d).count();
    let total_rows = usize::from(num_rows);
    let full_repaint = dirty_count * 2 >= total_rows;

    let screen = vt.screen();
    let mut dirty_rows = Vec::with_capacity(if full_repaint { total_rows } else { dirty_count });

    for row_idx in 0..num_rows {
        if full_repaint || dirty_flags.get(usize::from(row_idx)).copied().unwrap_or(false) {
            let mut cells = Vec::with_capacity(usize::from(cols));
            for col_idx in 0..cols {
                let cell = screen.cell(row_idx, col_idx);
                match cell {
                    Some(c) => {
                        cells.push(godly_protocol::types::RichGridCell {
                            content: c.contents().to_string(),
                            fg: color_to_hex(c.fgcolor()),
                            bg: color_to_hex(c.bgcolor()),
                            bold: c.bold(),
                            dim: c.dim(),
                            italic: c.italic(),
                            underline: c.underline(),
                            inverse: c.inverse(),
                            wide: c.is_wide(),
                            wide_continuation: c.is_wide_continuation(),
                        });
                    }
                    None => {
                        cells.push(godly_protocol::types::RichGridCell {
                            content: String::new(),
                            fg: "default".to_string(),
                            bg: "default".to_string(),
                            bold: false,
                            dim: false,
                            italic: false,
                            underline: false,
                            inverse: false,
                            wide: false,
                            wide_continuation: false,
                        });
                    }
                }
            }
            let wrapped = screen.row_wrapped(row_idx);
            dirty_rows.push((
                row_idx,
                godly_protocol::types::RichGridRow { cells, wrapped },
            ));
        }
    }

    godly_protocol::types::RichGridDiff {
        dirty_rows,
        cursor: godly_protocol::types::CursorState {
            row: cursor_row,
            col: cursor_col,
        },
        dimensions: godly_protocol::types::GridDimensions {
            rows: num_rows,
            cols,
        },
        alternate_screen: screen.alternate_screen(),
        cursor_hidden: screen.hide_cursor(),
        title: screen.window_title().to_string(),
        scrollback_offset: screen.scrollback(),
        total_scrollback: screen.scrollback_count(),
        full_repaint,
    }
}

/// Convert a godly-vt Color to a hex string for the frontend renderer.
fn color_to_hex(color: godly_vt::Color) -> String {
    match color {
        godly_vt::Color::Default => "default".to_string(),
        godly_vt::Color::Idx(idx) => {
            // Standard 256-color xterm palette
            match idx {
                // Standard 16 colors
                0 => "#000000".to_string(),
                1 => "#cd3131".to_string(),
                2 => "#0dbc79".to_string(),
                3 => "#e5e510".to_string(),
                4 => "#2472c8".to_string(),
                5 => "#bc3fbc".to_string(),
                6 => "#11a8cd".to_string(),
                7 => "#e5e5e5".to_string(),
                8 => "#666666".to_string(),
                9 => "#f14c4c".to_string(),
                10 => "#23d18b".to_string(),
                11 => "#f5f543".to_string(),
                12 => "#3b8eea".to_string(),
                13 => "#d670d6".to_string(),
                14 => "#29b8db".to_string(),
                15 => "#e5e5e5".to_string(),
                // 216-color cube (indices 16-231)
                16..=231 => {
                    let i = idx - 16;
                    let r = i / 36;
                    let g = (i % 36) / 6;
                    let b = i % 6;
                    let r = if r == 0 { 0 } else { 55 + 40 * r };
                    let g = if g == 0 { 0 } else { 55 + 40 * g };
                    let b = if b == 0 { 0 } else { 55 + 40 * b };
                    format!("#{:02x}{:02x}{:02x}", r, g, b)
                }
                // 24 grayscale (indices 232-255)
                232..=255 => {
                    let v = 8 + 10 * (idx - 232);
                    format!("#{:02x}{:02x}{:02x}", v, v, v)
                }
            }
        }
        godly_vt::Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
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
#[allow(dead_code)]
fn windows_to_wsl_path(path: &str) -> String {
    let path = path.replace('\\', "/");

    // Handle WSL UNC paths: //wsl.localhost/<distro>/... or //wsl$/<distro>/...
    if path.starts_with("//wsl.localhost/") || path.starts_with("//wsl$/") {
        let after_host = if path.starts_with("//wsl.localhost/") {
            &path["//wsl.localhost/".len()..]
        } else {
            &path["//wsl$/".len()..]
        };
        return match after_host.find('/') {
            Some(idx) => {
                let linux_path = &after_host[idx..];
                if linux_path == "/" {
                    "/".to_string()
                } else {
                    linux_path.to_string()
                }
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

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_unc() {
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\home\\alanm\\dev\\project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_unc_forward_slashes() {
        assert_eq!(
            windows_to_wsl_path("//wsl.localhost/Ubuntu/home/alanm/dev/project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_dollar_unc() {
        assert_eq!(
            windows_to_wsl_path("\\\\wsl$\\Ubuntu\\home\\alanm\\dev\\project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_root() {
        assert_eq!(windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu"), "/");
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_root_trailing_slash() {
        assert_eq!(windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\"), "/");
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_deep_path() {
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\home\\alanm\\dev\\terraform-tests\\terraform-provider-typesense"),
            "/home/alanm/dev/terraform-tests/terraform-provider-typesense"
        );
    }
}
