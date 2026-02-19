use std::io::Write;
use std::sync::Mutex;

use godly_protocol::{
    DaemonMessage, Request, Response, read_daemon_message, write_request,
};

/// Synchronous daemon pipe client. Sends requests and reads responses,
/// discarding async events (same pattern as `mcp/src/daemon_direct.rs`).
pub struct DaemonClient {
    pipe: Mutex<std::fs::File>,
}

impl DaemonClient {
    /// Connect to the daemon's named pipe.
    #[cfg(windows)]
    pub fn connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let pipe_name_str = godly_protocol::pipe_name();
        tracing::info!("Connecting to daemon pipe: {}", pipe_name_str);

        let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            return Err(format!(
                "Cannot connect to daemon pipe (error: {}). Is the daemon running?",
                err
            ));
        }

        use std::os::windows::io::FromRawHandle;
        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };
        tracing::info!("Connected to daemon pipe");
        Ok(Self {
            pipe: Mutex::new(pipe),
        })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("Daemon pipe connection is only supported on Windows".to_string())
    }

    /// Send a request and read the response, discarding async events.
    pub fn send_request(&self, request: &Request) -> Result<Response, String> {
        let mut pipe = self.pipe.lock().map_err(|e| format!("Lock poisoned: {}", e))?;
        write_request(&mut *pipe, request)
            .map_err(|e| format!("Daemon write error: {}", e))?;
        pipe.flush().ok();

        loop {
            let msg: DaemonMessage = read_daemon_message(&mut *pipe)
                .map_err(|e| format!("Daemon read error: {}", e))?
                .ok_or_else(|| "Daemon pipe closed".to_string())?;

            match msg {
                DaemonMessage::Response(resp) => return Ok(resp),
                DaemonMessage::Event(_) => continue,
            }
        }
    }

    /// Check if the daemon is reachable by sending a Ping.
    pub fn is_connected(&self) -> bool {
        self.send_request(&Request::Ping)
            .map(|r| matches!(r, Response::Pong))
            .unwrap_or(false)
    }
}

/// Async wrapper: runs a blocking daemon request on a blocking thread.
pub async fn async_request(
    client: &DaemonClient,
    request: Request,
) -> Result<Response, String> {
    // We can't move the client into spawn_blocking, so we use a scoped approach.
    // Since DaemonClient uses a Mutex internally, we do the blocking call directly
    // but wrap it in spawn_blocking to avoid blocking the tokio runtime.
    //
    // Note: We need to use a raw pointer trick since spawn_blocking requires 'static.
    // This is safe because we await the result immediately, so client outlives the task.
    let client_ptr = client as *const DaemonClient as usize;
    let result = tokio::task::spawn_blocking(move || {
        let client = unsafe { &*(client_ptr as *const DaemonClient) };
        client.send_request(&request)
    })
    .await
    .map_err(|e| format!("Blocking task panicked: {}", e))?;

    result
}
