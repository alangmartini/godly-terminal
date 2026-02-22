mod metadata;
mod protocol;
mod pty;
mod ring_buffer;

use metadata::ShimMetadata;
use protocol::{
    parse_incoming_frame, write_binary_frame, write_json, ShimControlRequest,
    ShimControlResponse, ShimFrame, TAG_BUFFER_DATA, TAG_OUTPUT, TAG_WRITE,
};
use ring_buffer::RingBuffer;
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicBool, AtomicU16, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Grace period after shell exits before shim self-terminates.
const SHELL_EXIT_GRACE_SECS: u64 = 30;

struct Args {
    session_id: String,
    shell_type: String,
    rows: u16,
    cols: u16,
    cwd: Option<String>,
    pipe_name: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut session_id = None;
    let mut shell_type = None;
    let mut rows = None;
    let mut cols = None;
    let mut cwd = None;
    let mut pipe_name = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--session-id" => {
                i += 1;
                session_id = args.get(i).cloned();
            }
            "--shell-type" => {
                i += 1;
                shell_type = args.get(i).cloned();
            }
            "--rows" => {
                i += 1;
                rows = args.get(i).and_then(|s| s.parse().ok());
            }
            "--cols" => {
                i += 1;
                cols = args.get(i).and_then(|s| s.parse().ok());
            }
            "--cwd" => {
                i += 1;
                cwd = args.get(i).cloned();
            }
            "--pipe-name" => {
                i += 1;
                pipe_name = args.get(i).cloned();
            }
            other => {
                return Err(format!("Unknown argument: {}", other));
            }
        }
        i += 1;
    }

    Ok(Args {
        session_id: session_id.ok_or("--session-id is required")?,
        shell_type: shell_type.ok_or("--shell-type is required")?,
        rows: rows.ok_or("--rows is required")?,
        cols: cols.ok_or("--cols is required")?,
        cwd,
        pipe_name,
    })
}

#[cfg(not(windows))]
fn main() {
    eprintln!("godly-pty-shim is only supported on Windows");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    if let Err(e) = run() {
        eprintln!("godly-pty-shim fatal: {}", e);
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run() -> Result<(), String> {
    let args = parse_args()?;

    let pipe_name = args
        .pipe_name
        .unwrap_or_else(|| format!(r"\\.\pipe\godly-shim-{}", args.session_id));

    // Open PTY — returns separately-owned parts for different threads
    let pty_parts =
        pty::open_pty(&args.shell_type, args.cwd.as_deref(), args.rows, args.cols, None)?;
    let shell_pid = pty_parts.shell_pid;

    eprintln!(
        "godly-pty-shim: session={} shell_pid={} pipe={}",
        args.session_id, shell_pid, pipe_name
    );

    // Write metadata file
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let meta = ShimMetadata {
        session_id: args.session_id.clone(),
        shim_pid: std::process::id(),
        shim_pipe_name: pipe_name.clone(),
        shell_pid,
        shell_type: args.shell_type.clone(),
        cwd: args.cwd.clone(),
        rows: args.rows,
        cols: args.cols,
        created_at: now,
    };
    metadata::write_metadata(&meta).map_err(|e| format!("write metadata: {}", e))?;

    // Shared state
    let ring_buffer = Arc::new(Mutex::new(RingBuffer::new()));
    let shell_running = Arc::new(AtomicBool::new(true));
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let current_rows = Arc::new(AtomicU16::new(args.rows));
    let current_cols = Arc::new(AtomicU16::new(args.cols));

    // Channels
    let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();
    let (exit_tx, exit_rx) = mpsc::channel::<Option<i64>>();
    let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>();
    let (resize_tx, resize_rx) = mpsc::channel::<(u16, u16)>();

    // PTY reader thread: reads from ConPTY → output channel
    let shell_running_r = shell_running.clone();
    let mut pty_reader = pty_parts.reader;
    let pty_reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match pty_reader.read(&mut buf) {
                Ok(0) => {
                    shell_running_r.store(false, Ordering::SeqCst);
                    let _ = exit_tx.send(None);
                    break;
                }
                Ok(n) => {
                    if output_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("godly-pty-shim: PTY read error: {}", e);
                    shell_running_r.store(false, Ordering::SeqCst);
                    let _ = exit_tx.send(None);
                    break;
                }
            }
        }
    });

    // PTY writer thread: input channel → ConPTY; also handles resize
    let pty_master = pty_parts.master;
    let mut pty_writer = pty_parts.writer;
    let writer_thread = std::thread::spawn(move || {
        loop {
            // Handle resize requests
            while let Ok((rows, cols)) = resize_rx.try_recv() {
                let _ = pty_master.resize(portable_pty::PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }

            // Wait for input data with timeout
            match input_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(data) => {
                    if pty_writer.write_all(&data).is_err() {
                        break;
                    }
                    let _ = pty_writer.flush();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    // Main loop: accept daemon connections and handle bidirectional I/O
    let session_id = args.session_id.clone();
    let result = main_loop(
        &pipe_name,
        &session_id,
        shell_pid,
        ring_buffer,
        shell_running.clone(),
        shutdown_requested,
        output_rx,
        exit_rx,
        input_tx,
        resize_tx,
        current_rows,
        current_cols,
    );

    // Cleanup
    let _ = metadata::remove_metadata(&args.session_id);
    let _ = pty_reader_thread.join();
    let _ = writer_thread.join();

    result
}

#[cfg(windows)]
fn main_loop(
    pipe_name: &str,
    _session_id: &str,
    shell_pid: u32,
    ring_buffer: Arc<Mutex<RingBuffer>>,
    shell_running: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    exit_rx: mpsc::Receiver<Option<i64>>,
    input_tx: mpsc::Sender<Vec<u8>>,
    resize_tx: mpsc::Sender<(u16, u16)>,
    current_rows: Arc<AtomicU16>,
    current_cols: Arc<AtomicU16>,
) -> Result<(), String> {
    use mpsc::TryRecvError;

    loop {
        if shutdown_requested.load(Ordering::SeqCst) {
            eprintln!("godly-pty-shim: shutdown requested, exiting");
            return Ok(());
        }

        if !shell_running.load(Ordering::SeqCst) {
            // Shell has exited. Drain remaining output into ring buffer.
            while let Ok(data) = output_rx.try_recv() {
                ring_buffer.lock().unwrap().append(&data);
            }

            eprintln!(
                "godly-pty-shim: shell exited, waiting {}s for daemon to collect status",
                SHELL_EXIT_GRACE_SECS
            );

            let exit_code = exit_rx.try_recv().unwrap_or(None);

            // Wait for one last daemon connection within the grace period
            match pipe::create_pipe_and_wait(pipe_name, Duration::from_secs(SHELL_EXIT_GRACE_SECS))
            {
                Ok(Some(handle)) => {
                    let buf_data = ring_buffer.lock().unwrap().drain_all();
                    if !buf_data.is_empty() {
                        let mut w = pipe::PipeWriter::new(&handle);
                        let _ = write_binary_frame(&mut w, TAG_BUFFER_DATA, &buf_data);
                    }
                    let mut w = pipe::PipeWriter::new(&handle);
                    let resp = ShimControlResponse::ShellExited { exit_code };
                    let _ = write_json(&mut w, &resp);
                    handle.disconnect_and_close();
                }
                Ok(None) => {
                    eprintln!("godly-pty-shim: grace period expired, no daemon connected");
                }
                Err(e) => {
                    eprintln!("godly-pty-shim: pipe error during grace: {}", e);
                }
            }

            return Ok(());
        }

        // Create pipe and wait for daemon (short timeout to re-check flags)
        let handle = match pipe::create_pipe_and_wait(pipe_name, Duration::from_secs(1)) {
            Ok(Some(h)) => h,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("godly-pty-shim: pipe error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        eprintln!("godly-pty-shim: daemon connected on {}", pipe_name);

        // Send buffered data first
        {
            let buf_data = ring_buffer.lock().unwrap().drain_all();
            if !buf_data.is_empty() {
                let mut w = pipe::PipeWriter::new(&handle);
                if let Err(e) = write_binary_frame(&mut w, TAG_BUFFER_DATA, &buf_data) {
                    eprintln!("godly-pty-shim: error sending buffer: {}", e);
                    handle.disconnect_and_close();
                    continue;
                }
            }
        }

        // Bidirectional I/O loop
        let mut disconnected = false;

        loop {
            if shutdown_requested.load(Ordering::SeqCst) {
                handle.disconnect_and_close();
                return Ok(());
            }

            if !shell_running.load(Ordering::SeqCst) {
                // Shell exited while daemon is connected — drain & notify
                while let Ok(data) = output_rx.try_recv() {
                    let mut w = pipe::PipeWriter::new(&handle);
                    if write_binary_frame(&mut w, TAG_OUTPUT, &data).is_err() {
                        break;
                    }
                }
                let exit_code = exit_rx.try_recv().unwrap_or(None);
                let mut w = pipe::PipeWriter::new(&handle);
                let resp = ShimControlResponse::ShellExited { exit_code };
                let _ = write_json(&mut w, &resp);
                handle.disconnect_and_close();
                return Ok(());
            }

            // Forward PTY output to daemon
            match output_rx.try_recv() {
                Ok(data) => {
                    let mut w = pipe::PipeWriter::new(&handle);
                    if let Err(e) = write_binary_frame(&mut w, TAG_OUTPUT, &data) {
                        eprintln!("godly-pty-shim: pipe write error: {}", e);
                        ring_buffer.lock().unwrap().append(&data);
                        disconnected = true;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    shell_running.store(false, Ordering::SeqCst);
                    continue;
                }
            }

            if disconnected {
                break;
            }

            // Read daemon→shim messages (non-blocking)
            match pipe::try_read_frame(&handle) {
                Ok(Some(frame_data)) => {
                    match parse_incoming_frame(&frame_data) {
                        Ok(ShimFrame::Binary { tag, data }) if tag == TAG_WRITE => {
                            let _ = input_tx.send(data);
                        }
                        Ok(ShimFrame::Control(ShimControlRequest::Resize { rows, cols })) => {
                            current_rows.store(rows, Ordering::SeqCst);
                            current_cols.store(cols, Ordering::SeqCst);
                            let _ = resize_tx.send((rows, cols));
                        }
                        Ok(ShimFrame::Control(ShimControlRequest::Status)) => {
                            let resp = ShimControlResponse::StatusInfo {
                                shell_pid,
                                running: shell_running.load(Ordering::SeqCst),
                                rows: current_rows.load(Ordering::SeqCst),
                                cols: current_cols.load(Ordering::SeqCst),
                            };
                            let mut w = pipe::PipeWriter::new(&handle);
                            if let Err(e) = write_json(&mut w, &resp) {
                                eprintln!("godly-pty-shim: status write error: {}", e);
                                disconnected = true;
                            }
                        }
                        Ok(ShimFrame::Control(ShimControlRequest::Shutdown)) => {
                            shutdown_requested.store(true, Ordering::SeqCst);
                            handle.disconnect_and_close();
                            return Ok(());
                        }
                        Ok(ShimFrame::Control(ShimControlRequest::DrainBuffer)) => {
                            let buf_data = ring_buffer.lock().unwrap().drain_all();
                            if !buf_data.is_empty() {
                                let mut w = pipe::PipeWriter::new(&handle);
                                if let Err(e) =
                                    write_binary_frame(&mut w, TAG_BUFFER_DATA, &buf_data)
                                {
                                    eprintln!("godly-pty-shim: drain write error: {}", e);
                                    disconnected = true;
                                }
                            }
                        }
                        Ok(ShimFrame::Binary { tag, .. }) => {
                            eprintln!("godly-pty-shim: unknown binary tag: 0x{:02x}", tag);
                        }
                        Err(e) => {
                            eprintln!("godly-pty-shim: frame parse error: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // No data available
                }
                Err(pipe::PipeError::Disconnected) => {
                    eprintln!("godly-pty-shim: daemon disconnected");
                    disconnected = true;
                }
                Err(pipe::PipeError::Io(e)) => {
                    eprintln!("godly-pty-shim: pipe read error: {}", e);
                    disconnected = true;
                }
            }

            if disconnected {
                break;
            }

            // Avoid busy-spinning
            std::thread::sleep(Duration::from_millis(1));
        }

        handle.disconnect_and_close();
        eprintln!("godly-pty-shim: daemon disconnected, buffering output");

        // Drain any output that arrived during the session into the ring buffer
        while let Ok(data) = output_rx.try_recv() {
            ring_buffer.lock().unwrap().append(&data);
        }
    }
}

// ── Windows named pipe implementation ──────────────────────────────────────

#[cfg(windows)]
mod pipe {
    use std::io::{self, Write};
    use std::time::Duration;
    use winapi::shared::minwindef::DWORD;
    use winapi::shared::winerror;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::fileapi::{FlushFileBuffers, ReadFile, WriteFile};
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::namedpipeapi::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe,
    };
    use winapi::um::winbase::{PIPE_ACCESS_DUPLEX, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT};
    use winapi::um::winnt::HANDLE;

    /// A Windows named pipe handle with RAII disconnect/close.
    pub struct PipeHandle {
        handle: HANDLE,
    }

    // SAFETY: The pipe handle is used from a single thread at a time
    // with proper synchronization in the main loop.
    unsafe impl Send for PipeHandle {}
    unsafe impl Sync for PipeHandle {}

    impl PipeHandle {
        pub fn disconnect_and_close(self) {
            unsafe {
                DisconnectNamedPipe(self.handle);
                CloseHandle(self.handle);
            }
            std::mem::forget(self); // prevent Drop from double-closing
        }
    }

    impl Drop for PipeHandle {
        fn drop(&mut self) {
            unsafe {
                DisconnectNamedPipe(self.handle);
                CloseHandle(self.handle);
            }
        }
    }

    pub enum PipeError {
        Disconnected,
        Io(io::Error),
    }

    /// Create a named pipe server and wait for a client to connect.
    /// Returns `Ok(Some(pipe))` on connection, `Ok(None)` on timeout, `Err` on failure.
    ///
    /// IMPORTANT: The pipe is created WITHOUT FILE_FLAG_OVERLAPPED. All subsequent
    /// ReadFile/WriteFile calls use synchronous I/O. Mixing FILE_FLAG_OVERLAPPED
    /// with NULL OVERLAPPED parameters in ReadFile/WriteFile causes undefined
    /// behavior on Windows (MSDN: "the function can incorrectly report that the
    /// write/read operation is complete"), which silently drops pipe data.
    ///
    /// For the connection timeout, we run blocking ConnectNamedPipe in a dedicated
    /// thread and wait on a channel with timeout.
    pub fn create_pipe_and_wait(
        pipe_name: &str,
        timeout: Duration,
    ) -> Result<Option<PipeHandle>, String> {
        let wide_name: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

        let handle = unsafe {
            CreateNamedPipeW(
                wide_name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                1,    // max instances
                8192, // out buffer
                8192, // in buffer
                timeout.as_millis() as DWORD,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(format!("CreateNamedPipeW failed: error {}", err));
        }

        // Run blocking ConnectNamedPipe in a thread so we can apply a timeout.
        // The thread holds a raw handle pointer (safe: ConnectNamedPipe is a
        // system call that returns cleanly when the handle is closed).
        let handle_raw = handle as usize;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let h = handle_raw as HANDLE;
            let result = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) };
            let err = if result == 0 {
                unsafe { GetLastError() }
            } else {
                0
            };
            let _ = tx.send((result, err));
        });

        match rx.recv_timeout(timeout) {
            Ok((result, err)) => {
                if result != 0 || err == winerror::ERROR_PIPE_CONNECTED {
                    Ok(Some(PipeHandle { handle }))
                } else {
                    unsafe {
                        DisconnectNamedPipe(handle);
                        CloseHandle(handle);
                    }
                    Err(format!("ConnectNamedPipe failed: error {}", err))
                }
            }
            Err(_) => {
                // Timeout: close the handle to unblock ConnectNamedPipe in the thread
                unsafe {
                    DisconnectNamedPipe(handle);
                    CloseHandle(handle);
                }
                Ok(None)
            }
        }
    }

    /// Non-blocking read of a complete length-prefixed frame from the pipe.
    /// Returns `Ok(Some(data))` if a complete frame is available, `Ok(None)` if
    /// insufficient data, `Err(Disconnected)` if the pipe is broken.
    pub fn try_read_frame(pipe: &PipeHandle) -> Result<Option<Vec<u8>>, PipeError> {
        let mut available: DWORD = 0;
        let peek_ok = unsafe {
            PeekNamedPipe(
                pipe.handle,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };

        if peek_ok == 0 {
            let err = unsafe { GetLastError() };
            if err == winerror::ERROR_BROKEN_PIPE || err == winerror::ERROR_NO_DATA {
                return Err(PipeError::Disconnected);
            }
            return Err(PipeError::Io(io::Error::from_raw_os_error(err as i32)));
        }

        if available < 4 {
            return Ok(None);
        }

        // Read the 4-byte length header
        let mut len_buf = [0u8; 4];
        read_pipe_exact(pipe, &mut len_buf).map_err(PipeError::Io)?;

        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 16 * 1024 * 1024 {
            return Err(PipeError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Frame too large: {}", len),
            )));
        }

        let mut buf = vec![0u8; len];
        if !buf.is_empty() {
            read_pipe_exact(pipe, &mut buf).map_err(PipeError::Io)?;
        }

        Ok(Some(buf))
    }

    fn read_pipe_exact(pipe: &PipeHandle, buf: &mut [u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < buf.len() {
            let mut bytes_read: DWORD = 0;
            let ok = unsafe {
                ReadFile(
                    pipe.handle,
                    buf[offset..].as_mut_ptr() as *mut _,
                    (buf.len() - offset) as DWORD,
                    &mut bytes_read,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            if bytes_read == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "pipe EOF"));
            }
            offset += bytes_read as usize;
        }
        Ok(())
    }

    /// Writer wrapper for pipe handle, implements `std::io::Write`.
    pub struct PipeWriter<'a> {
        pipe: &'a PipeHandle,
    }

    impl<'a> PipeWriter<'a> {
        pub fn new(pipe: &'a PipeHandle) -> Self {
            Self { pipe }
        }
    }

    impl<'a> Write for PipeWriter<'a> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut bytes_written: DWORD = 0;
            let ok = unsafe {
                WriteFile(
                    self.pipe.handle,
                    buf.as_ptr() as *const _,
                    buf.len() as DWORD,
                    &mut bytes_written,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            Ok(bytes_written as usize)
        }

        fn flush(&mut self) -> io::Result<()> {
            let ok = unsafe { FlushFileBuffers(self.pipe.handle) };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                return Err(io::Error::from_raw_os_error(err as i32));
            }
            Ok(())
        }
    }
}
