use std::fs::File;
use std::io;

use godly_protocol::{read_message, write_message, WhisperRequest, WhisperResponse};
use parking_lot::Mutex;

pub struct WhisperClient {
    pipe: Mutex<Option<File>>,
}

impl WhisperClient {
    pub fn new() -> Self {
        Self {
            pipe: Mutex::new(None),
        }
    }

    /// Connect to the whisper sidecar's named pipe.
    #[cfg(windows)]
    pub fn connect(&self, pipe_name: &str) -> io::Result<()> {
        use std::os::windows::io::FromRawHandle;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let wide_name: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

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

        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let file = unsafe { File::from_raw_handle(handle as _) };
        *self.pipe.lock() = Some(file);
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn connect(&self, _pipe_name: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Whisper sidecar is only supported on Windows",
        ))
    }

    /// Send a request and wait for the response.
    pub fn send_request(&self, request: &WhisperRequest) -> io::Result<WhisperResponse> {
        let mut guard = self.pipe.lock();
        let pipe = guard.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "Not connected to whisper sidecar")
        })?;

        write_message(pipe, request)?;

        read_message::<_, WhisperResponse>(pipe)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Whisper sidecar closed connection"))
    }

    pub fn disconnect(&self) {
        *self.pipe.lock() = None;
    }

    pub fn is_connected(&self) -> bool {
        self.pipe.lock().is_some()
    }
}
