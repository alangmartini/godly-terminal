use godly_protocol::{McpRequest, McpResponse};

/// Abstraction over how godly-mcp sends requests.
/// Two implementations:
/// - `AppBackend`: talks to the Tauri app via MCP pipe (full functionality)
/// - `DaemonDirectBackend`: talks directly to the daemon (subset of tools)
pub trait Backend: Send + Sync {
    fn send_request(&self, request: &McpRequest) -> Result<McpResponse, String>;

    /// Human-readable label for logging (e.g. "app" or "daemon-direct").
    fn label(&self) -> &'static str;
}
