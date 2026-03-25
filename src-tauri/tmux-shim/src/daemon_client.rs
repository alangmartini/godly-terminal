//! Daemon pipe client for I/O operations (Write, ReadGrid, etc.).
//!
//! Connects to the daemon's named pipe and sends `Request` messages,
//! reading `DaemonMessage` responses (discarding async Events).

use std::io::Write;

use godly_protocol::{DaemonMessage, Request, Response};

/// Client for the Godly Terminal daemon pipe.
pub struct DaemonClient {
    pipe: std::fs::File,
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
                "cannot connect to daemon pipe (error: {}). Is Godly Terminal running?",
                err
            ));
        }

        use std::os::windows::io::FromRawHandle;
        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };
        Ok(Self { pipe })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("daemon pipe is only supported on Windows".to_string())
    }

    /// Send a Request and wait for the Response, discarding async Events.
    pub fn request(&mut self, req: &Request) -> Result<Response, String> {
        godly_protocol::write_request(&mut self.pipe, req)
            .map_err(|e| format!("daemon write error: {}", e))?;
        self.pipe.flush().ok();

        loop {
            let msg: DaemonMessage = godly_protocol::read_daemon_message(&mut self.pipe)
                .map_err(|e| format!("daemon read error: {}", e))?
                .ok_or_else(|| "daemon pipe closed".to_string())?;

            match msg {
                DaemonMessage::Response(resp) => return Ok(resp),
                DaemonMessage::Event(_) => continue, // discard async events
            }
        }
    }
}
