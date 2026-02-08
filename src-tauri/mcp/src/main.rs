mod jsonrpc;
mod pipe_client;
mod tools;

use std::io::{self, BufReader};

use serde_json::json;

use jsonrpc::{JsonRpcResponse, read_message, write_message};
use pipe_client::McpPipeClient;

fn main() {
    // Read session ID from environment (set by Godly Terminal when creating the PTY)
    let session_id = std::env::var("GODLY_SESSION_ID").ok();

    // Connect to the Tauri app's MCP pipe
    let mut client = match McpPipeClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to Godly Terminal MCP pipe: {}", e);
            std::process::exit(1);
        }
    };

    // Run the stdio JSON-RPC loop
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout().lock();

    loop {
        let request = match read_message(&mut reader) {
            Ok(Some(req)) => req,
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("[godly-mcp] Read error: {}", e);
                break;
            }
        };

        let response = handle_request(&request, &mut client, &session_id);

        if let Err(e) = write_message(&mut stdout, &response) {
            eprintln!("[godly-mcp] Write error: {}", e);
            break;
        }
    }
}

fn handle_request(
    request: &jsonrpc::JsonRpcRequest,
    client: &mut McpPipeClient,
    session_id: &Option<String>,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => {
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

        "notifications/initialized" => {
            // No response needed for notifications, but since our loop always
            // writes a response, return an empty success. MCP clients should
            // ignore responses to notifications.
            JsonRpcResponse::success(request.id.clone(), json!({}))
        }

        "tools/list" => {
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

            match tools::call_tool(client, tool_name, &args, session_id) {
                Ok(result) => {
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
            JsonRpcResponse::error(
                request.id.clone(),
                -32601,
                format!("Method not found: {}", request.method),
            )
        }
    }
}
