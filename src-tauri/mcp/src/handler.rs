use serde_json::json;

use crate::app_backend::AppBackend;
use crate::backend::Backend;
use crate::daemon_direct::DaemonDirectBackend;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use crate::log::mcp_log;
use crate::pipe_client::McpPipeClient;
use crate::tools;

/// Try to connect a backend: MCP pipe (app) first, then daemon direct.
pub fn connect_backend() -> Result<Box<dyn Backend>, String> {
    // Try MCP pipe first (Tauri app)
    match McpPipeClient::connect() {
        Ok(client) => {
            mcp_log!("Connected via MCP pipe (app backend)");
            return Ok(Box::new(AppBackend::new(client)));
        }
        Err(e) => {
            mcp_log!("MCP pipe unavailable: {} — trying daemon direct...", e);
        }
    }

    // Fall back to daemon direct
    match DaemonDirectBackend::connect() {
        Ok(backend) => {
            mcp_log!("Connected via daemon pipe (daemon-direct fallback)");
            Ok(Box::new(backend))
        }
        Err(e) => Err(format!(
            "Cannot connect to Godly Terminal. App pipe and daemon pipe both unavailable. Last error: {}",
            e
        )),
    }
}

/// Try to reconnect: MCP pipe first, then daemon direct.
/// Returns new backend on success, None on failure.
pub fn try_reconnect() -> Option<Box<dyn Backend>> {
    if let Ok(client) = McpPipeClient::connect() {
        mcp_log!("Reconnected via MCP pipe (app backend)");
        return Some(Box::new(AppBackend::new(client)));
    }
    if let Ok(backend) = DaemonDirectBackend::connect() {
        mcp_log!("Reconnected via daemon pipe (daemon-direct fallback)");
        return Some(Box::new(backend));
    }
    None
}

/// If currently in daemon-direct mode, cheaply probe the MCP pipe
/// and upgrade to app backend if the Tauri app is back.
pub fn maybe_upgrade_backend(backend: &mut Box<dyn Backend>) {
    if backend.label() != "daemon-direct" {
        return;
    }

    // Cheap probe: try opening the MCP pipe
    if let Ok(client) = McpPipeClient::connect() {
        mcp_log!("App pipe is back — upgrading from daemon-direct to app backend");
        *backend = Box::new(AppBackend::new(client));
    }
}

pub fn handle_request(
    request: &JsonRpcRequest,
    backend: &mut Box<dyn Backend>,
    session_id: &Option<String>,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            mcp_log!("Handling initialize");
            JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "godly-terminal",
                        "version": "0.1.0"
                    }
                }),
            )
        }

        "tools/list" => {
            mcp_log!("Handling tools/list");
            JsonRpcResponse::success(request.id.clone(), tools::list_tools())
        }

        "tools/call" => {
            let params = request.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            mcp_log!(
                "Handling tools/call: tool={}, args={}, backend={}",
                tool_name,
                args,
                backend.label()
            );

            match tools::call_tool(backend.as_ref(), tool_name, &args, session_id) {
                Ok(result) => {
                    mcp_log!("Tool call succeeded: {}", tool_name);
                    JsonRpcResponse::success(
                        request.id.clone(),
                        json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&result)
                                    .unwrap_or_else(|_| result.to_string())
                            }]
                        }),
                    )
                }
                Err(e) => {
                    mcp_log!("Tool call failed: {} — {}", tool_name, e);

                    // If it looks like a pipe error, try to reconnect
                    if e.contains("Pipe error")
                        || e.contains("write error")
                        || e.contains("read error")
                        || e.contains("Pipe closed")
                        || e.contains("Daemon pipe closed")
                    {
                        mcp_log!("Pipe error detected — attempting reconnect...");
                        if let Some(new_backend) = try_reconnect() {
                            mcp_log!("Reconnected via {}", new_backend.label());
                            *backend = new_backend;

                            // Retry the tool call once
                            match tools::call_tool(
                                backend.as_ref(),
                                tool_name,
                                &args,
                                session_id,
                            ) {
                                Ok(result) => {
                                    mcp_log!("Retry succeeded: {}", tool_name);
                                    return JsonRpcResponse::success(
                                        request.id.clone(),
                                        json!({
                                            "content": [{
                                                "type": "text",
                                                "text": serde_json::to_string_pretty(&result)
                                                    .unwrap_or_else(|_| result.to_string())
                                            }]
                                        }),
                                    );
                                }
                                Err(retry_err) => {
                                    mcp_log!("Retry also failed: {}", retry_err);
                                    return JsonRpcResponse::success(
                                        request.id.clone(),
                                        json!({
                                            "content": [{
                                                "type": "text",
                                                "text": format!("Error: {}", retry_err)
                                            }],
                                            "isError": true
                                        }),
                                    );
                                }
                            }
                        }
                    }

                    JsonRpcResponse::success(
                        request.id.clone(),
                        json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }],
                            "isError": true
                        }),
                    )
                }
            }
        }

        _ => {
            mcp_log!("Unknown method: {}", request.method);
            JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            )
        }
    }
}



/// Connect a backend and return as Arc (for HTTP server multi-session use).
pub fn connect_backend_arc() -> Result<std::sync::Arc<dyn Backend>, String> {
    connect_backend().map(|b| {
        let arc: std::sync::Arc<dyn Backend> = std::sync::Arc::from(b);
        arc
    })
}


/// Handle a JSON-RPC request using a shared backend reference.
/// Unlike handle_request, this does NOT attempt reconnection (caller manages that).
pub fn handle_request_ref(
    request: &JsonRpcRequest,
    backend: &dyn Backend,
    session_id: &Option<String>,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
            mcp_log!("Handling initialize");
            JsonRpcResponse::success(
                request.id.clone(),
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "godly-terminal",
                        "version": "0.1.0"
                    }
                }),
            )
        }

        "tools/list" => {
            mcp_log!("Handling tools/list");
            JsonRpcResponse::success(request.id.clone(), tools::list_tools())
        }

        "tools/call" => {
            let params = request.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            mcp_log!(
                "Handling tools/call: tool={}, args={}, backend={}",
                tool_name,
                args,
                backend.label()
            );

            match tools::call_tool(backend, tool_name, &args, session_id) {
                Ok(result) => {
                    mcp_log!("Tool call succeeded: {}", tool_name);
                    JsonRpcResponse::success(
                        request.id.clone(),
                        json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&result)
                                    .unwrap_or_else(|_| result.to_string())
                            }]
                        }),
                    )
                }
                Err(e) => {
                    mcp_log!("Tool call failed: {} — {}", tool_name, e);



                    JsonRpcResponse::success(
                        request.id.clone(),
                        json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }],
                            "isError": true
                        }),
                    )
                }
            }
        }

        _ => {
            mcp_log!("Unknown method: {}", request.method);
            JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            )
        }
    }
}
