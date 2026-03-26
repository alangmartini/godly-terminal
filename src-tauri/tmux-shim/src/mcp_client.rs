use std::io;
use std::sync::Mutex;

use godly_protocol::{McpRequest, McpResponse};

/// Client that communicates with the Godly Terminal app via the MCP named pipe.
pub struct McpPipeClient {
    pipe: Mutex<std::fs::File>,
}

impl McpPipeClient {
    /// Connect to the MCP named pipe.
    #[cfg(windows)]
    pub fn connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;
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
                "Cannot connect to MCP pipe '{}' (error: {}). Is Godly Terminal running?",
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
        Err("MCP named pipes are only supported on Windows".to_string())
    }

    /// Send an MCP request and wait for the response.
    pub fn send_request(&self, request: &McpRequest) -> Result<McpResponse, io::Error> {
        let mut pipe = self
            .pipe
            .lock()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Mutex poisoned: {}", e)))?;

        godly_protocol::write_message(&mut *pipe, request)?;
        use std::io::Write;
        pipe.flush().ok();

        match godly_protocol::read_message::<_, McpResponse>(&mut *pipe)? {
            Some(response) => Ok(response),
            None => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "MCP pipe closed",
            )),
        }
    }

    /// Helper: send request and map IO errors to String.
    fn request(&self, req: &McpRequest) -> Result<McpResponse, String> {
        self.send_request(req)
            .map_err(|e| format!("MCP error: {}", e))
    }

    /// Create a terminal in a workspace, returning the new terminal ID.
    pub fn create_terminal(
        &self,
        workspace_id: &str,
        cwd: Option<String>,
    ) -> Result<String, String> {
        match self.request(&McpRequest::CreateTerminal {
            workspace_id: workspace_id.to_string(),
            shell_type: None,
            cwd,
            worktree_name: None,
            worktree: None,
            command: None,
            focus: Some(false),
        })? {
            McpResponse::Created { id, .. } => Ok(id),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Split a terminal, placing new_terminal_id beside target_terminal_id.
    pub fn split_terminal(
        &self,
        workspace_id: &str,
        target_terminal_id: &str,
        new_terminal_id: &str,
        direction: &str,
        ratio: f64,
    ) -> Result<(), String> {
        match self.request(&McpRequest::SplitTerminal {
            workspace_id: workspace_id.to_string(),
            target_terminal_id: target_terminal_id.to_string(),
            new_terminal_id: new_terminal_id.to_string(),
            direction: direction.to_string(),
            ratio,
        })? {
            McpResponse::Ok => Ok(()),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Focus a terminal.
    pub fn focus_terminal(&self, terminal_id: &str) -> Result<(), String> {
        match self.request(&McpRequest::FocusTerminal {
            terminal_id: terminal_id.to_string(),
        })? {
            McpResponse::Ok => Ok(()),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Close a terminal.
    pub fn close_terminal(&self, terminal_id: &str) -> Result<(), String> {
        match self.request(&McpRequest::CloseTerminal {
            terminal_id: terminal_id.to_string(),
        })? {
            McpResponse::Ok => Ok(()),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Write data to a terminal.
    pub fn write_to_terminal(&self, terminal_id: &str, data: &str) -> Result<(), String> {
        match self.request(&McpRequest::WriteToTerminal {
            terminal_id: terminal_id.to_string(),
            data: data.to_string(),
            focus: None,
        })? {
            McpResponse::Ok => Ok(()),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Get the active workspace.
    pub fn get_active_workspace(&self) -> Result<godly_protocol::McpWorkspaceInfo, String> {
        match self.request(&McpRequest::GetActiveWorkspace)? {
            McpResponse::ActiveWorkspace {
                workspace: Some(ws),
            } => Ok(ws),
            McpResponse::ActiveWorkspace { workspace: None } => {
                Err("no active workspace".to_string())
            }
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }

    /// Read terminal content (capture-pane).
    pub fn read_terminal(&self, terminal_id: &str) -> Result<String, String> {
        match self.request(&McpRequest::ReadTerminal {
            terminal_id: terminal_id.to_string(),
            mode: None,
            lines: None,
            strip_ansi: Some(true),
        })? {
            McpResponse::TerminalOutput { content } => Ok(content),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("unexpected response: {:?}", other)),
        }
    }
}
