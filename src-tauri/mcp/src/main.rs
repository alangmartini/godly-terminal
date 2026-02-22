mod app_backend;
mod backend;
mod daemon_direct;
mod handler;
mod jsonrpc;
mod log;
mod pipe_client;
mod sse;
mod tools;

use std::io::{self, BufReader};

use jsonrpc::{read_message, write_message};
use log::mcp_log;

/// Bump this on every godly-mcp code change so logs show which binary is running.
const BUILD: u32 = 15;

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
    godly-mcp sse [OPTIONS]            Start MCP server (SSE/HTTP transport)
    godly-mcp notify [OPTIONS]         Send a notification to Godly Terminal
    godly-mcp --help                   Show this help

COMMANDS:
    sse       Start SSE transport server (HTTP, serves multiple sessions)
    notify    Send a sound notification and badge alert

Run 'godly-mcp sse --help' or 'godly-mcp notify --help' for subcommand details."
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

fn print_sse_help() {
    eprintln!(
        "\
Start the SSE transport server.

Runs a persistent HTTP server that serves multiple Claude Code sessions
over Server-Sent Events (SSE). One process, many clients.

USAGE:
    godly-mcp sse [OPTIONS]

OPTIONS:
    -p, --port <PORT>          Port to listen on (default: 8089)
    -h, --help                 Show this help

PROTOCOL:
    1. Client opens GET /sse → receives SSE stream
    2. Server sends event: endpoint with POST URL
    3. Client sends JSON-RPC via POST /messages?sessionId=XXX
    4. Server pushes response as event: message on the SSE stream

EXAMPLES:
    godly-mcp sse
    godly-mcp sse --port 9090"
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
        "sse" => run_cli_sse(&args[1..]),
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
    let mut client = match pipe_client::McpPipeClient::connect() {
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

/// Parse and execute `godly-mcp sse [OPTIONS]`.
fn run_cli_sse(args: &[String]) -> i32 {
    let mut port: u16 = 8089;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_sse_help();
                return 0;
            }
            "-p" | "--port" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --port requires a value");
                    return 1;
                }
                port = match args[i].parse() {
                    Ok(p) => p,
                    Err(_) => {
                        eprintln!("Error: invalid port number '{}'", args[i]);
                        return 1;
                    }
                };
            }
            other => {
                eprintln!("Error: unknown option '{}'\n", other);
                print_sse_help();
                return 1;
            }
        }
        i += 1;
    }

    log::init();
    mcp_log!("=== godly-mcp SSE starting === build={}", BUILD);
    mcp_log!("PID: {}", std::process::id());

    // run_sse_server blocks forever
    sse::run_sse_server(port);
    0
}

// ---------------------------------------------------------------------------
// MCP server mode (stdio)
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

    mcp_log!("Connecting backend...");
    let mut backend = match handler::connect_backend() {
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
        handler::maybe_upgrade_backend(&mut backend);

        let response = handler::handle_request(&request, &mut backend, &session_id);

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
