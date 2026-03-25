//! MCP pipe client for layout operations (split, resize, etc.).
//!
//! Connects to the MCP named pipe and sends `McpRequest` messages.
//! This is a stub — the pane/session commands (implemented by other units)
//! will use this client.

use std::io::Write;

use godly_protocol::{McpRequest, McpResponse};

/// Client for the Godly Terminal MCP pipe.
pub struct McpClient {
    pipe: std::fs::File,
}

impl McpClient {
    /// Connect to the MCP named pipe.
    #[cfg(windows)]
    pub fn connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let pipe_name_str = godly_protocol::mcp_pipe_name();
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
                "cannot connect to MCP pipe (error: {}). Is Godly Terminal running?",
                err
            ));
        }

        use std::os::windows::io::FromRawHandle;
        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };
        Ok(Self { pipe })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("MCP pipe is only supported on Windows".to_string())
    }

    /// Send an MCP request and wait for the response.
    pub fn request(&mut self, req: &McpRequest) -> Result<McpResponse, String> {
        godly_protocol::write_message(&mut self.pipe, req)
            .map_err(|e| format!("MCP write error: {}", e))?;
        self.pipe.flush().ok();

        godly_protocol::read_message::<_, McpResponse>(&mut self.pipe)
            .map_err(|e| format!("MCP read error: {}", e))?
            .ok_or_else(|| "MCP pipe closed".to_string())
    }
}
