//! MCP pipe client for communicating with the Godly Terminal app.
//!
//! Sends `McpRequest` messages and reads `McpResponse` replies
//! over the MCP named pipe using length-prefixed JSON framing.

use std::io::Write;

use godly_protocol::{McpRequest, McpResponse};

/// Client that communicates with Godly Terminal via the MCP named pipe.
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
                "Cannot connect to MCP pipe (error: {}). Is Godly Terminal running?",
                err
            ));
        }

        use std::os::windows::io::FromRawHandle;
        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };
        Ok(Self { pipe })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("MCP named pipes are only supported on Windows".to_string())
    }

    /// Send an MCP request and read the response.
    pub fn request(&mut self, req: &McpRequest) -> Result<McpResponse, String> {
        godly_protocol::write_message(&mut self.pipe, req)
            .map_err(|e| format!("MCP write error: {}", e))?;
        self.pipe.flush().ok();

        match godly_protocol::read_message::<_, McpResponse>(&mut self.pipe) {
            Ok(Some(resp)) => Ok(resp),
            Ok(None) => Err("MCP pipe closed (EOF)".to_string()),
            Err(e) => Err(format!("MCP read error: {}", e)),
        }
    }

    /// Send GetActiveWorkspace and return the workspace_id.
    pub fn get_active_workspace_id(&mut self) -> Result<String, String> {
        match self.request(&McpRequest::GetActiveWorkspace)? {
            McpResponse::ActiveWorkspace {
                workspace: Some(ws),
            } => Ok(ws.id),
            McpResponse::ActiveWorkspace { workspace: None } => {
                Err("No active workspace".to_string())
            }
            McpResponse::Error { message } => Err(message),
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }

    /// Create a terminal in a workspace, returning the new terminal ID.
    pub fn create_terminal(
        &mut self,
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
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }

    /// Split a terminal, placing new_terminal_id beside target_terminal_id.
    pub fn split_terminal(
        &mut self,
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
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }

    /// Focus a terminal.
    pub fn focus_terminal(&mut self, terminal_id: &str) -> Result<(), String> {
        match self.request(&McpRequest::FocusTerminal {
            terminal_id: terminal_id.to_string(),
        })? {
            McpResponse::Ok => Ok(()),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }

    /// Close a terminal.
    pub fn close_terminal(&mut self, terminal_id: &str) -> Result<(), String> {
        match self.request(&McpRequest::CloseTerminal {
            terminal_id: terminal_id.to_string(),
        })? {
            McpResponse::Ok => Ok(()),
            McpResponse::Error { message } => Err(message),
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    }
}
