use godly_protocol::{McpRequest, McpResponse};

use crate::backend::Backend;
use crate::pipe_client::McpPipeClient;

/// Backend that routes requests through the Tauri app via the MCP named pipe.
/// This is the primary backend with full functionality.
pub struct AppBackend {
    pub client: McpPipeClient,
}

impl AppBackend {
    pub fn new(client: McpPipeClient) -> Self {
        Self { client }
    }
}

impl Backend for AppBackend {
    fn send_request(&mut self, request: &McpRequest) -> Result<McpResponse, String> {
        self.client
            .send_request(request)
            .map_err(|e| format!("Pipe error: {}", e))
    }

    fn label(&self) -> &'static str {
        "app"
    }
}
