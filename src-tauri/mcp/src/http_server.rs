use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Router;
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::jsonrpc::JsonRpcResponse;
use crate::log::mcp_log;
use crate::session::SessionRegistry;
use crate::handler;

const SESSION_HEADER: &str = "mcp-session-id";
const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60); // 5 minutes
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
/// Default port for the MCP HTTP server. Well-known port for Claude Code discovery.
pub const DEFAULT_PORT: u16 = 45557;

struct AppState {
    registry: SessionRegistry,
}

/// Start the HTTP server. Binds to 127.0.0.1 on the given port (or OS-assigned if None).
pub async fn run_http_server(port: Option<u16>) -> Result<(), String> {
    let state = Arc::new(AppState {
        registry: SessionRegistry::new(),
    });

    let app = Router::new()
        .route("/mcp", post(handle_post))
        .route("/mcp", delete(handle_delete))
        .route("/health", get(handle_health))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let bind_addr: SocketAddr = format!("127.0.0.1:{}", port.unwrap_or(DEFAULT_PORT))
        .parse()
        .map_err(|e| format!("Invalid address: {}", e))?;

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| format!("Failed to bind: {}", e))?;

    let local_addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?;

    let actual_port = local_addr.port();
    let url = format!("http://127.0.0.1:{}/mcp", actual_port);

    mcp_log!("HTTP server listening on {}", local_addr);
    mcp_log!("MCP endpoint: {}", url);

    // Write discovery file
    write_discovery_file(actual_port, &url);

    // Print URL to stdout so the Tauri app can capture it
    println!("{}", url);

    // Start background cleanup task
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(CLEANUP_INTERVAL).await;
            let removed = cleanup_state.registry.cleanup_idle(SESSION_IDLE_TIMEOUT);
            if removed > 0 {
                mcp_log!(
                    "http: cleaned up {} idle sessions ({} remaining)",
                    removed,
                    cleanup_state.registry.count()
                );
            }
        }
    });

    // Serve
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    // Clean up discovery file on shutdown
    cleanup_discovery_file();

    Ok(())
}

/// POST /mcp — Handle JSON-RPC requests
async fn handle_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    mcp_log!("http: POST /mcp ({} bytes)", body.len());

    // Parse JSON-RPC request
    let request: crate::jsonrpc::JsonRpcRequest = match serde_json::from_str(&body) {
        Ok(req) => req,
        Err(e) => {
            mcp_log!("http: JSON parse error: {}", e);
            return json_response(
                StatusCode::BAD_REQUEST,
                None,
                &JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e)),
            );
        }
    };

    mcp_log!(
        "http: method={}, id={:?}",
        request.method,
        request.id
    );

    // Notifications (no id) — accept but don't respond with a body
    if request.id.is_none() {
        mcp_log!("http: notification (no id), returning 202");
        return (StatusCode::ACCEPTED, "").into_response();
    }

    // Handle initialize — create a new session
    if request.method == "initialize" {
        return handle_initialize(state, &request).await;
    }

    // All other requests require a session
    let session_id = match headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        Some(id) => id.to_string(),
        None => {
            mcp_log!("http: missing {} header", SESSION_HEADER);
            return json_response(
                StatusCode::BAD_REQUEST,
                None,
                &JsonRpcResponse::error(
                    request.id.clone(),
                    -32600,
                    format!("Missing {} header. Send initialize first.", SESSION_HEADER),
                ),
            );
        }
    };

    let session = match state.registry.get_session(&session_id) {
        Some(s) => s,
        None => {
            mcp_log!("http: session {} not found", session_id);
            return json_response(
                StatusCode::NOT_FOUND,
                None,
                &JsonRpcResponse::error(
                    request.id.clone(),
                    -32600,
                    "Session not found. Send initialize to create a new session.".to_string(),
                ),
            );
        }
    };

    session.touch();

    // Run the request handler in spawn_blocking since backends do sync pipe I/O
    let backend = session.backend.clone();
    let terminal_session_id = session.terminal_session_id.clone();
    let req_clone = request.clone();

    let response = tokio::task::spawn_blocking(move || {
        handler::handle_request_ref(&req_clone, backend.as_ref(), &terminal_session_id)
    })
    .await
    .unwrap_or_else(|e| {
        JsonRpcResponse::error(
            request.id.clone(),
            -32603,
            format!("Internal error: {}", e),
        )
    });

    json_response(StatusCode::OK, Some(&session_id), &response)
}

/// Handle the `initialize` method — create a new session with its own backend.
async fn handle_initialize(
    state: Arc<AppState>,
    request: &crate::jsonrpc::JsonRpcRequest,
) -> Response {
    // Connect a new backend for this session (blocking I/O)
    let backend = match tokio::task::spawn_blocking(handler::connect_backend_arc).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            mcp_log!("http: failed to connect backend for new session: {}", e);
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                None,
                &JsonRpcResponse::error(
                    request.id.clone(),
                    -32603,
                    format!("Cannot connect to Godly Terminal: {}", e),
                ),
            );
        }
        Err(e) => {
            return json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
                &JsonRpcResponse::error(
                    request.id.clone(),
                    -32603,
                    format!("Internal error: {}", e),
                ),
            );
        }
    };

    // Extract terminal session ID from params if provided
    let terminal_session_id = request
        .params
        .as_ref()
        .and_then(|p| p.get("clientInfo"))
        .and_then(|ci| ci.get("sessionId"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let session_id = state
        .registry
        .create_session(backend, terminal_session_id);

    mcp_log!("http: created session {} (backend connected)", session_id);

    let response = JsonRpcResponse::success(
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
    );

    json_response(StatusCode::OK, Some(&session_id), &response)
}

/// DELETE /mcp — Terminate a session
async fn handle_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    let session_id = match headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        Some(id) => id.to_string(),
        None => {
            return (StatusCode::BAD_REQUEST, "Missing Mcp-Session-Id header").into_response();
        }
    };

    state.registry.remove_session(&session_id);
    mcp_log!("http: DELETE session {}", session_id);

    (StatusCode::OK, "Session terminated").into_response()
}

/// GET /health — Health check
async fn handle_health(State(state): State<Arc<AppState>>) -> Response {
    let count = state.registry.count();
    let body = json!({
        "status": "ok",
        "sessions": count,
        "pid": std::process::id(),
    });
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// Build a JSON response with optional Mcp-Session-Id header.
fn json_response(
    status: StatusCode,
    session_id: Option<&str>,
    body: &JsonRpcResponse,
) -> Response {
    let json_body = serde_json::to_string(body).unwrap_or_default();

    let mut builder = Response::builder()
        .status(status)
        .header("content-type", "application/json");

    if let Some(id) = session_id {
        builder = builder.header(SESSION_HEADER, id);
    }

    builder
        .body(Body::from(json_body))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response())
}

/// Write discovery file to %APPDATA%/com.godly.terminal/mcp-http.json
fn write_discovery_file(port: u16, url: &str) {
    let path = discovery_file_path();
    if let Some(path) = path {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = json!({
            "port": port,
            "pid": std::process::id(),
            "url": url,
        });
        match std::fs::write(&path, content.to_string()) {
            Ok(_) => mcp_log!("http: wrote discovery file: {}", path.display()),
            Err(e) => mcp_log!("http: failed to write discovery file: {}", e),
        }
    }
}

/// Remove discovery file on shutdown.
fn cleanup_discovery_file() {
    if let Some(path) = discovery_file_path() {
        let _ = std::fs::remove_file(&path);
        mcp_log!("http: cleaned up discovery file");
    }
}

/// Get the path to the discovery file.
fn discovery_file_path() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| {
                std::path::PathBuf::from(appdata)
                    .join("com.godly.terminal")
                    .join("mcp-http.json")
            })
    }
    #[cfg(not(windows))]
    {
        None
    }
}
