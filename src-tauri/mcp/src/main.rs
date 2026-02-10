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
const BUILD: u32 = 7;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // If there are CLI arguments (beyond the binary name), run in CLI mode.
    if args.len() > 1 {
        std::process::exit(run_cli(&args[1..]));
    }

    // Otherwise, run in MCP server mode (stdin/stdout JSON-RPC).
    run_mcp_server();
}

// ---------------------------------------------------------------------------
// CLI mode
// ---------------------------------------------------------------------------

fn print_help() {
    eprintln!(
        "\
godly-mcp — Godly Terminal MCP server & CLI

USAGE:
    godly-mcp                          Start MCP server (stdin/stdout JSON-RPC)
    godly-mcp notify [OPTIONS]         Send a notification to Godly Terminal
    godly-mcp --help                   Show this help

COMMANDS:
    notify    Send a sound notification and badge alert

Run 'godly-mcp notify --help' for subcommand details."
    );
}

fn print_notify_help() {
    eprintln!(
        "\
Send a notification to Godly Terminal.

Plays a chime and shows a badge on the terminal tab.

USAGE:
    godly-mcp notify [OPTIONS]

OPTIONS:
    -m, --message <TEXT>       Message to include with the notification
    --terminal-id <ID>         Terminal ID (defaults to GODLY_SESSION_ID env var)
    -h, --help                 Show this help

ENVIRONMENT:
    GODLY_SESSION_ID           Terminal ID used when --terminal-id is not provided

EXAMPLES:
    godly-mcp notify
    godly-mcp notify -m \"Build complete\"
    godly-mcp notify --terminal-id abc123 --message \"Done\""
    );
}

/// Run in CLI mode. Returns the process exit code.
fn run_cli(args: &[String]) -> i32 {
    match args[0].as_str() {
        "--help" | "-h" => {
            print_help();
            0
        }
        "notify" => run_cli_notify(&args[1..]),
        other => {
            eprintln!("Error: unknown command '{}'\n", other);
            print_help();
            1
        }
    }
}

/// Parse and execute `godly-mcp notify [OPTIONS]`.
fn run_cli_notify(args: &[String]) -> i32 {
    let mut message: Option<String> = None;
    let mut terminal_id: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_notify_help();
                return 0;
            }
            "-m" | "--message" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --message requires a value");
                    return 1;
                }
                message = Some(args[i].clone());
            }
            "--terminal-id" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --terminal-id requires a value");
                    return 1;
                }
                terminal_id = Some(args[i].clone());
            }
            other => {
                eprintln!("Error: unknown option '{}'\n", other);
                print_notify_help();
                return 1;
            }
        }
        i += 1;
    }

    // Resolve terminal ID: explicit flag > env var
    let terminal_id = match terminal_id.or_else(|| std::env::var("GODLY_SESSION_ID").ok()) {
        Some(id) => id,
        None => {
            eprintln!("Error: no terminal ID. Set GODLY_SESSION_ID or pass --terminal-id");
            return 1;
        }
    };

    // Connect to pipe and send the request
    let mut client = match McpPipeClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };

    let request = godly_protocol::McpRequest::Notify {
        terminal_id,
        message,
    };

    match client.send_request(&request) {
        Ok(godly_protocol::McpResponse::Ok) => {
            println!("Notification sent");
            0
        }
        Ok(godly_protocol::McpResponse::Error { message }) => {
            eprintln!("Error: {}", message);
            1
        }
        Ok(other) => {
            println!("OK: {:?}", other);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

// ---------------------------------------------------------------------------
// MCP server mode (original behavior)
// ---------------------------------------------------------------------------

fn run_mcp_server() {
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
