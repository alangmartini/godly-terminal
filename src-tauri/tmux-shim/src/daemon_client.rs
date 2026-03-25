//! Daemon pipe client for reading terminal grid state.
//!
//! Connects to the daemon pipe and sends `Request` messages,
//! reading `DaemonMessage` replies (discarding async Events).

use std::io::Write;

use godly_protocol::{read_daemon_message, write_request, DaemonMessage, Request, Response};

/// Client that communicates with the Godly Terminal daemon pipe.
pub struct DaemonClient {
    pipe: std::fs::File,
}

impl DaemonClient {
    /// Connect to the daemon named pipe.
    #[cfg(windows)]
    pub fn connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let pipe_name_str = godly_protocol::pipe_name();
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
        Ok(Self { pipe })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("Daemon pipe is only supported on Windows".to_string())
    }

    /// Send a daemon Request and read the Response, discarding async Events.
    pub fn request(&mut self, req: &Request) -> Result<Response, String> {
        write_request(&mut self.pipe, req).map_err(|e| format!("Daemon write error: {}", e))?;
        self.pipe.flush().ok();

        loop {
            let msg: DaemonMessage = read_daemon_message(&mut self.pipe)
                .map_err(|e| format!("Daemon read error: {}", e))?
                .ok_or_else(|| "Daemon pipe closed".to_string())?;

            match msg {
                DaemonMessage::Response(resp) => return Ok(resp),
                DaemonMessage::Event(_) => continue, // discard async events
            }
        }
    }

    /// Read the grid state for a terminal/session, returning (cols, rows).
    pub fn read_grid_size(&mut self, session_id: &str) -> Result<(u16, u16), String> {
        match self.request(&Request::ReadGrid {
            session_id: session_id.to_string(),
        })? {
            Response::Grid { grid } => Ok((grid.cols, grid.num_rows)),
            Response::Error { message } => Err(message),
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }
}
