mod app_backend;
mod backend;
mod daemon_direct;
pub mod http_server;
mod jsonrpc;
mod log;
mod pipe_client;
pub mod session;
mod tools;

use std::io::{self, BufReader};
use std::sync::Arc;

use serde_json::json;

use app_backend::AppBackend;
use backend::Backend;
use daemon_direct::DaemonDirectBackend;
use jsonrpc::{JsonRpcResponse, read_message, write_message};
use log::mcp_log;
use pipe_client::McpPipeClient;

/// Bump this on every godly-mcp code change so logs show which binary is running.
const BUILD: u32 = 15;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // If there are CLI arguments (beyond the binary name), run in CLI mode.
    if args.len() > 1 {
        std::process::exit(run_cli(&args[1..]));
    }

    // Default: run in stdio MCP server mode (backward compat).
    run_mcp_server();
}

// ---------------------------------------------------------------------------
// Backend connection
// ---------------------------------------------------------------------------

/// Try to connect a backend: MCP pipe (app) first, then daemon direct.
pub fn connect_backend() -> Result<Arc<dyn Backend>, String> {
    // Try MCP pipe first (Tauri app)
    match McpPipeClient::connect() {
        Ok(client) => {
            mcp_log!("Connected via MCP pipe (app backend)");
            return Ok(Arc::new(AppBackend::new(client)));
        }
        Err(e) => {
            mcp_log!("MCP pipe unavailable: {} — trying daemon direct...", e);
        }
    }

    // Fall back to daemon direct
    match DaemonDirectBackend::connect() {
        Ok(backend) => {
            mcp_log!("Connected via daemon pipe (daemon-direct fallback)");
            Ok(Arc::new(backend))
        }
        Err(e) => Err(format!(
            "Cannot connect to Godly Terminal. App pipe and daemon pipe both unavailable. Last error: {}",
            e
        )),
    }
}

/// Try to reconnect: MCP pipe first, then daemon direct.
/// Returns new backend on success, None on failure.
fn try_reconnect() -> Option<Arc<dyn Backend>> {
    if let Ok(client) = McpPipeClient::connect() {
        mcp_log!("Reconnected via MCP pipe (app backend)");
        return Some(Arc::new(AppBackend::new(client)));
    }
    if let Ok(backend) = DaemonDirectBackend::connect() {
        mcp_log!("Reconnected via daemon pipe (daemon-direct fallback)");
        return Some(Arc::new(backend));
    }
    None
}

// ---------------------------------------------------------------------------
// CLI mode
// ---------------------------------------------------------------------------

fn print_help() {
    eprintln!(
        "\
godly-mcp — Godly Terminal MCP server & CLI

USAGE:
    godly-mcp                          Start MCP server (stdio JSON-RPC, default)
    godly-mcp --stdio                  Start MCP server (stdio JSON-RPC, explicit)
    godly-mcp --http [PORT]            Start MCP server (HTTP, default port {})
    godly-mcp notify [OPTIONS]         Send a notification to Godly Terminal
    godly-mcp --help                   Show this help

COMMANDS:
    notify    Send a sound notification and badge alert

Run 'godly-mcp notify --help' for subcommand details.",
        http_server::DEFAULT_PORT
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
        "--stdio" => {
            run_mcp_server();
            0
        }
        "--http" => {
            let port = args.get(1).and_then(|s| s.parse::<u16>().ok());
            run_http_server_blocking(port);
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

    // Connect to pipe and send the request (CLI notify only uses app backend)
    let client = match McpPipeClient::connect() {
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
// HTTP server mode
// ---------------------------------------------------------------------------

fn run_http_server_blocking(port: Option<u16>) {
    log::init();

    mcp_log!("=== godly-mcp starting (HTTP mode) === build={}", BUILD);
    mcp_log!("PID: {}", std::process::id());

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = http_server::run_http_server(port).await {
            mcp_log!("HTTP server error: {}", e);
            eprintln!("[godly-mcp] HTTP server error: {}", e);
            std::process::exit(1);
        }
    });
}

// ---------------------------------------------------------------------------
// Stdio MCP server mode
// ---------------------------------------------------------------------------

fn run_mcp_server() {
    log::init();

    mcp_log!("=== godly-mcp starting (stdio mode) === build={}", BUILD);
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

    mcp_log!("Connecting backend...");
    let mut backend = match connect_backend() {
        Ok(b) => {
            mcp_log!("Backend connected: {}", b.label());
            b
        }
        Err(e) => {
            mcp_log!("FATAL: No backend available: {}", e);
            eprintln!("Failed to connect to Godly Terminal: {}", e);
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

        // If in fallback mode, try to upgrade to app backend
        if backend.label() == "daemon-direct" {
            if let Ok(client) = McpPipeClient::connect() {
                mcp_log!("App pipe is back — upgrading from daemon-direct to app backend");
                backend = Arc::new(AppBackend::new(client));
            }
        }

        let response = handle_request(&request, backend.as_ref(), &session_id);

        let response_json = serde_json::to_string(&response).unwrap_or_default();
        mcp_log!("Sending response: {}", response_json);

        if let Err(e) = write_message(&mut stdout, &response) {
            mcp_log!("Write error: {}", e);
            eprintln!("[godly-mcp] Write error: {}", e);
            break;
        }

        // If the response was an error from a pipe failure, try to reconnect
        if is_pipe_error_response(&response) {
            mcp_log!("Pipe error detected in response — attempting reconnect...");
            if let Some(new_backend) = try_reconnect() {
                mcp_log!("Reconnected via {}", new_backend.label());
                backend = new_backend;
            }
        }

        mcp_log!("Response sent successfully");
    }

    mcp_log!("=== godly-mcp shutting down ===");
}

/// Check if a response contains a pipe error that warrants reconnection.
fn is_pipe_error_response(response: &JsonRpcResponse) -> bool {
    if let Some(result) = &response.result {
        if let Some(true) = result.get("isError").and_then(|v| v.as_bool()) {
            if let Some(text) = result
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
            {
                return text.contains("Pipe error")
                    || text.contains("write error")
                    || text.contains("read error")
                    || text.contains("Pipe closed")
                    || text.contains("Daemon pipe closed");
            }
        }
    }
    false
}

pub fn handle_request(
    request: &jsonrpc::JsonRpcRequest,
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
