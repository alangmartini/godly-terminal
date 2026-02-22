use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use tokio::sync::mpsc;

use crate::backend::Backend;
use crate::handler;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use crate::log::mcp_log;

/// Shared state for the SSE server.
struct SseState {
    /// The backend connection (sync — guarded by std::sync::Mutex since pipe I/O blocks).
    backend: Mutex<Box<dyn Backend>>,
    /// Map of session ID → channel sender for pushing SSE responses.
    sessions: Mutex<HashMap<String, mpsc::Sender<JsonRpcResponse>>>,
}

type SharedState = Arc<SseState>;

/// Start the SSE server on the given port. Blocks forever.
pub fn run_sse_server(port: u16) {
    mcp_log!("Starting SSE server on port {}", port);

    let backend = match handler::connect_backend() {
        Ok(b) => {
            mcp_log!("SSE backend connected: {}", b.label());
            b
        }
        Err(e) => {
            mcp_log!("FATAL: No backend available: {}", e);
            eprintln!("Failed to connect to Godly Terminal: {}", e);
            std::process::exit(1);
        }
    };

    let state = Arc::new(SseState {
        backend: Mutex::new(backend),
        sessions: Mutex::new(HashMap::new()),
    });

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async move {
        let app = Router::new()
            .route("/sse", get(sse_handler))
            .route("/messages", post(messages_handler))
            .with_state(state);

        let addr = format!("127.0.0.1:{}", port);
        mcp_log!("SSE server listening on {}", addr);
        eprintln!("godly-mcp SSE server listening on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .unwrap_or_else(|e| {
                mcp_log!("FATAL: Failed to bind to {}: {}", addr, e);
                eprintln!("Failed to bind to {}: {}", addr, e);
                std::process::exit(1);
            });

        axum::serve(listener, app).await.unwrap_or_else(|e| {
            mcp_log!("FATAL: Server error: {}", e);
            eprintln!("Server error: {}", e);
        });
    });
}

/// GET /sse — Create a new SSE session and return the event stream.
async fn sse_handler(
    State(state): State<SharedState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::channel::<JsonRpcResponse>(64);

    // Register the session
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), tx);
    }
    mcp_log!("SSE session connected: {}", session_id);

    let endpoint_url = format!("/messages?sessionId={}", session_id);
    let log_id = session_id.clone();
    let cleanup_state = state.clone();
    let cleanup_id = session_id.clone();

    let stream = async_stream::stream! {
        // First event: tell the client where to POST requests
        yield Ok(Event::default().event("endpoint").data(endpoint_url));

        // Then relay all responses for this session
        loop {
            match rx.recv().await {
                Some(response) => {
                    match serde_json::to_string(&response) {
                        Ok(json) => {
                            yield Ok(Event::default().event("message").data(json));
                        }
                        Err(e) => {
                            mcp_log!("SSE serialize error for session {}: {}", log_id, e);
                        }
                    }
                }
                None => {
                    // Channel closed — session ended
                    break;
                }
            }
        }

        // Clean up session when stream drops
        mcp_log!("SSE stream ended for session: {}", cleanup_id);
        let mut sessions = cleanup_state.sessions.lock().unwrap();
        sessions.remove(&cleanup_id);
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(serde::Deserialize)]
struct MessageQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

/// POST /messages?sessionId=X — Receive a JSON-RPC request, process it, push response via SSE.
async fn messages_handler(
    State(state): State<SharedState>,
    Query(query): Query<MessageQuery>,
    body: String,
) -> impl IntoResponse {
    let session_id = query.session_id;

    // Find the sender for this session
    let tx = {
        let sessions = state.sessions.lock().unwrap();
        match sessions.get(&session_id) {
            Some(tx) => tx.clone(),
            None => {
                mcp_log!("POST /messages: unknown session {}", session_id);
                return StatusCode::NOT_FOUND;
            }
        }
    };

    // Parse the JSON-RPC request
    let request: JsonRpcRequest = match serde_json::from_str(&body) {
        Ok(req) => req,
        Err(e) => {
            mcp_log!("POST /messages: JSON parse error: {}", e);
            // Send error response via SSE
            let error_response =
                JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
            let _ = tx.send(error_response).await;
            return StatusCode::ACCEPTED;
        }
    };

    mcp_log!(
        "POST /messages: session={}, method={}, id={:?}",
        session_id,
        request.method,
        request.id
    );

    // JSON-RPC notifications (no id) — don't respond
    if request.id.is_none() {
        mcp_log!("Skipping notification (no id): method={}", request.method);
        return StatusCode::ACCEPTED;
    }

    // Process the request on a blocking thread (pipe I/O is synchronous)
    let state_clone = state.clone();
    tokio::task::spawn_blocking(move || {
        let response = {
            let mut backend = state_clone.backend.lock().unwrap();

            // Try to upgrade backend if in fallback mode
            handler::maybe_upgrade_backend(&mut backend);

            // No per-session GODLY_SESSION_ID in SSE mode — each request must supply it via tool args
            let session_id_env = std::env::var("GODLY_SESSION_ID").ok();

            handler::handle_request(&request, &mut backend, &session_id_env)
        };

        let response_json = serde_json::to_string(&response).unwrap_or_default();
        mcp_log!("SSE response: {}", response_json);

        // Push through SSE channel
        let _ = tx.blocking_send(response);
    });

    StatusCode::ACCEPTED
}
