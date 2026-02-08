mod jsonrpc;
mod log;
mod pipe_client;
mod tools;

use std::io::{self, BufReader};

use serde_json::json;

use jsonrpc::{JsonRpcResponse, read_message, write_message};
use log::mcp_log;
use pipe_client::McpPipeClient;

/// Bump this on every godly-mcp code change so logs show which binary is running.
const BUILD: u32 = 5;

fn main() {
    log::init();

    mcp_log!("=== godly-mcp starting === build={}", BUILD);
    mcp_log!("PID: {}", std::process::id());
    if let Ok(exe) = std::env::current_exe() {
        mcp_log!("exe: {}", exe.display());
    }
    if let Ok(cwd) = std::env::current_dir() {
        mcp_log!("cwd: {}", cwd.display());
    }

    let session_id = std::env::var("GODLY_SESSION_ID").ok();
    mcp_log!("GODLY_SESSION_ID: {:?}", session_id);

    let pipe_name = std::env::var("GODLY_MCP_PIPE_NAME").ok();
    mcp_log!("GODLY_MCP_PIPE_NAME: {:?}", pipe_name);

    mcp_log!("Connecting to MCP pipe...");
    let mut client = match McpPipeClient::connect() {
        Ok(c) => {
            mcp_log!("Pipe connected successfully");
            c
        }
        Err(e) => {
            mcp_log!("FATAL: Pipe connection failed: {}", e);
            eprintln!("Failed to connect to Godly Terminal MCP pipe: {}", e);
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();

    mcp_log!("Entering main JSON-RPC loop, waiting for stdin...");

    loop {
        let request = match read_message(&mut reader) {
            Ok(Some(req)) => {
                mcp_log!(
                    "Received request: method={}, id={:?}",
                    req.method,
                    req.id
                );
                req
            }
            Ok(None) => {
                mcp_log!("stdin EOF — shutting down");
                break;
            }
            Err(e) => {
                mcp_log!("Read error: {}", e);
                eprintln!("[godly-mcp] Read error: {}", e);
                break;
            }
        };

        // JSON-RPC notifications have no id — servers MUST NOT respond to them.
        if request.id.is_none() {
            mcp_log!("Skipping notification (no id): method={}", request.method);
            continue;
        }

        let response = handle_request(&request, &mut client, &session_id);

        let response_json = serde_json::to_string(&response).unwrap_or_default();
        mcp_log!("Sending response: {}", response_json);

        if let Err(e) = write_message(&mut stdout, &response) {
            mcp_log!("Write error: {}", e);
            eprintln!("[godly-mcp] Write error: {}", e);
            break;
        }

        mcp_log!("Response sent successfully");
    }

    mcp_log!("=== godly-mcp shutting down ===");
}

fn handle_request(
    request: &jsonrpc::JsonRpcRequest,
    client: &mut McpPipeClient,
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

            mcp_log!("Handling tools/call: tool={}, args={}", tool_name, args);

            match tools::call_tool(client, tool_name, &args, session_id) {
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
