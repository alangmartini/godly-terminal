use std::io::{self, Write};
use std::sync::Mutex;

use godly_protocol::{DaemonMessage, Request, Response};

/// Client that communicates with the Godly Terminal daemon via the daemon named pipe.
pub struct DaemonPipeClient {
    pipe: Mutex<std::fs::File>,
}

impl DaemonPipeClient {
    /// Connect to the daemon named pipe.
    #[cfg(windows)]
    pub fn connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;
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
                "Cannot connect to daemon pipe '{}' (error: {}). Is the daemon running?",
                pipe_name_str, err
            ));
        }

        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };

        Ok(Self {
            pipe: Mutex::new(pipe),
        })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("Daemon named pipes are only supported on Windows".to_string())
    }

    /// Send a daemon Request and read the Response, discarding async Events.
    pub fn send_request(&self, request: &Request) -> Result<Response, io::Error> {
        let mut pipe = self
            .pipe
            .lock()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Mutex poisoned: {}", e)))?;

        godly_protocol::write_request(&mut *pipe, request)?;
        pipe.flush().ok();

        // Read messages, discarding Events until we get a Response
        loop {
            let msg: DaemonMessage =
                godly_protocol::read_daemon_message(&mut *pipe)?.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "Daemon pipe closed")
                })?;

            match msg {
                DaemonMessage::Response(resp) => return Ok(resp),
                DaemonMessage::Event(_) => continue,
            }
        }
    }

    /// Write data to a terminal session.
    pub fn write_to_terminal(&self, session_id: &str, data: Vec<u8>) -> Result<(), String> {
        let resp = self
            .send_request(&Request::Write {
                session_id: session_id.to_string(),
                data,
            })
            .map_err(|e| format!("daemon write error: {}", e))?;
        match resp {
            Response::Ok => Ok(()),
            Response::Error { message } => Err(format!("daemon error: {}", message)),
            other => Err(format!("unexpected daemon response: {:?}", other)),
        }
    }

    /// Read the grid dimensions (cols, rows) for a terminal session.
    pub fn read_grid_size(&self, session_id: &str) -> Result<(u16, u16), String> {
        let resp = self
            .send_request(&Request::ReadGrid {
                session_id: session_id.to_string(),
            })
            .map_err(|e| format!("daemon read error: {}", e))?;
        match resp {
            Response::Grid { grid } => Ok((grid.cols, grid.num_rows)),
            Response::Error { message } => Err(message),
            other => Err(format!("unexpected daemon response: {:?}", other)),
        }
    }
}
