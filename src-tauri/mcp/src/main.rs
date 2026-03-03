#![recursion_limit = "256"]

mod app_backend;
mod backend;
mod daemon_direct;
mod handler;
pub mod http_server;
mod jsonrpc;
mod log;
mod pipe_client;
pub mod session;
mod sse;
mod tools;

use std::io::{self, BufReader};
use jsonrpc::{read_message, write_message};
use log::mcp_log;

/// Bump this on every godly-mcp code change so logs show which binary is running.
const BUILD: u32 = 24;

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
// CLI mode
// ---------------------------------------------------------------------------

fn print_help() {
    eprintln!(
        "\
godly-mcp — Godly Terminal MCP server & CLI

USAGE:
    godly-mcp                          Start MCP server (stdio JSON-RPC, default)
    godly-mcp --stdio                  Start MCP server (stdio JSON-RPC, explicit)
    godly-mcp --http [PORT]            Start MCP server (Streamable HTTP, default port {port})
    godly-mcp --ensure [PORT]          Ensure HTTP server is running, start if needed (default port {port})
    godly-mcp sse [OPTIONS]            Start MCP server (SSE/HTTP transport)
    godly-mcp notify [OPTIONS]         Send a notification to Godly Terminal
    godly-mcp --help                   Show this help

COMMANDS:
    sse       Start SSE transport server (HTTP, serves multiple sessions)
    notify    Send a sound notification and badge alert

FLAGS:
    --ensure  Check if the HTTP server is already running (via discovery file).
              If running, exit 0. If not, spawn a detached HTTP server process
              and wait until it's healthy before exiting 0. Exit 1 on failure.

Run 'godly-mcp sse --help' or 'godly-mcp notify --help' for subcommand details.",
        port = http_server::DEFAULT_PORT
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
        "--ensure" => {
            let port = args.get(1).and_then(|s| s.parse::<u16>().ok());
            run_ensure(port)
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
    let client = match pipe_client::McpPipeClient::connect() {
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
// Ensure mode — check/start HTTP server
// ---------------------------------------------------------------------------

/// Ensure the HTTP server is running. If not, spawn it as a detached process
/// and poll /health until it's ready.
fn run_ensure(port: Option<u16>) -> i32 {
    let port = port.unwrap_or(http_server::DEFAULT_PORT);

    // 1. Check discovery file for an existing server
    if let Some(pid) = read_discovery_pid() {
        if is_process_alive(pid) {
            if health_check(port) {
                eprintln!("[godly-mcp] HTTP server already running (PID {}, port {})", pid, port);
                return 0;
            }
            // PID alive but health check failed — stale process or wrong port, continue to spawn
        }
    }

    // 2. Spawn detached HTTP server
    eprintln!("[godly-mcp] Starting HTTP server on port {}...", port);

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[godly-mcp] Failed to get current exe path: {}", e);
            return 1;
        }
    };

    if let Err(e) = spawn_detached(&exe, port) {
        eprintln!("[godly-mcp] Failed to spawn HTTP server: {}", e);
        return 1;
    }

    // 3. Poll /health with exponential backoff (up to ~5 seconds)
    let delays_ms = [100, 200, 400, 500, 500, 500, 500, 500, 500, 500, 500, 500];
    for delay in delays_ms {
        std::thread::sleep(std::time::Duration::from_millis(delay));
        if health_check(port) {
            eprintln!("[godly-mcp] HTTP server is ready (port {})", port);
            return 0;
        }
    }

    eprintln!("[godly-mcp] HTTP server failed to start within timeout");
    1
}

/// Read the PID from the discovery file, if it exists.
fn read_discovery_pid() -> Option<u32> {
    #[cfg(windows)]
    {
        let path = std::env::var("APPDATA").ok().map(|appdata| {
            std::path::PathBuf::from(appdata)
                .join("com.godly.terminal")
                .join("mcp-http.json")
        })?;

        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        json.get("pid")?.as_u64().map(|p| p as u32)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Check if a process with the given PID is still alive.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use winapi::um::processthreadsapi::OpenProcess;
        use winapi::um::handleapi::CloseHandle;
        use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            CloseHandle(handle);
            true
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

/// Hit GET /health on localhost and return true if we get a 200.
fn health_check(port: u16) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr = format!("127.0.0.1:{}", port);
    let mut stream = match TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        Duration::from_millis(500),
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(1000)));

    let request = format!(
        "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        port
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut buf = [0u8; 256];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => {
            let response = String::from_utf8_lossy(&buf[..n]);
            response.contains("200")
        }
        _ => false,
    }
}

/// Spawn a detached godly-mcp --http process.
fn spawn_detached(exe: &std::path::Path, port: u16) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS (0x8) | CREATE_NO_WINDOW (0x08000000)
        const FLAGS: u32 = 0x00000008 | 0x08000000;

        std::process::Command::new(exe)
            .args(["--http", &port.to_string()])
            .creation_flags(FLAGS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn failed: {}", e))?;

        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, port);
        Err("--ensure is only supported on Windows".to_string())
    }
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
