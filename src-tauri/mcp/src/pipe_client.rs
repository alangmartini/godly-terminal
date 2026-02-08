use std::io;

use godly_protocol::{McpRequest, McpResponse};

use crate::log::mcp_log;

/// Client that communicates with the Tauri app via the MCP named pipe.
pub struct McpPipeClient {
    pipe: std::fs::File,
}

impl McpPipeClient {
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
        mcp_log!("pipe_client: pipe name = {}", pipe_name_str);

        let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        mcp_log!("pipe_client: calling CreateFileW...");
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
            mcp_log!("pipe_client: CreateFileW FAILED — error code {}", err);
            return Err(format!(
                "Cannot connect to MCP pipe (error: {}). Is Godly Terminal running?",
                err
            ));
        }

        mcp_log!("pipe_client: CreateFileW succeeded");

        use std::os::windows::io::FromRawHandle;
        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };

        Ok(Self { pipe })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("MCP named pipes are only supported on Windows".to_string())
    }

    /// Send an MCP request and wait for the response.
    pub fn send_request(&mut self, request: &McpRequest) -> Result<McpResponse, io::Error> {
        mcp_log!("pipe_client: sending request: {:?}", request);

        godly_protocol::write_message(&mut self.pipe, request)?;
        // Flush to ensure the message is sent
        use std::io::Write;
        self.pipe.flush().ok();

        mcp_log!("pipe_client: request sent, waiting for response...");

        match godly_protocol::read_message::<_, McpResponse>(&mut self.pipe)? {
            Some(response) => {
                mcp_log!("pipe_client: response received: {:?}", response);
                Ok(response)
            }
            None => {
                mcp_log!("pipe_client: pipe closed (EOF) while waiting for response");
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Pipe closed",
                ))
            }
        }
    }
}
