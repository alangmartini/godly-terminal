pub mod device;
pub mod events;
pub mod health;
pub mod input;
pub mod monitor;
pub mod phone;
pub mod prompts;
pub mod quick_claude;
pub mod sessions;
pub mod sse_ticket;
pub mod workspaces;

use axum::middleware;
use axum::routing::{delete, get, post};
use axum::Router;

use crate::auth::{api_key_auth, device_token_auth};
use crate::AppState;

pub fn build_router(state: AppState) -> Router {
    // Public routes (no auth)
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/phone", get(phone::phone_ui));

    // Device registration (requires API key but not device token — this IS how you get the token)
    let device_routes = Router::new()
        .route("/api/register-device", post(device::register_device))
        .route("/api/device-status", get(device::device_status))
        .layer(middleware::from_fn(api_key_auth));

    // SSE events — auth is done inline via one-time ticket (no middleware needed).
    // EventSource can't set custom headers, so we use a ticket acquired via /api/sse-ticket.
    let sse_routes = Router::new()
        .route("/api/events", get(events::event_stream));

    // Authenticated API routes (require both API key and device token)
    let api = Router::new()
        .route("/api/sessions", get(sessions::list_sessions))
        .route("/api/sessions", post(sessions::create_session))
        .route("/api/sessions/:id", get(sessions::get_session))
        .route("/api/sessions/:id", delete(sessions::delete_session))
        .route("/api/sessions/:id/grid", get(sessions::get_grid))
        .route("/api/sessions/:id/text", get(sessions::get_text))
        .route("/api/sessions/:id/idle", get(sessions::get_idle))
        .route("/api/sessions/:id/write", post(input::write_to_session))
        .route("/api/sessions/:id/resize", post(sessions::resize_session))
        .route("/api/sessions/:id/prompts", get(prompts::session_prompts))
        .route("/api/workspaces", get(workspaces::list_workspaces))
        .route("/api/prompts", get(prompts::list_prompts))
        .route("/api/sse-ticket", post(sse_ticket::create_sse_ticket))
        .route("/api/quick-claude", post(quick_claude::quick_claude))
        .route("/api/monitor", get(monitor::list_monitors))
        .route("/api/monitor/:id", post(monitor::start_monitor))
        .route("/api/monitor/:id", delete(monitor::stop_monitor))
        .layer(middleware::from_fn(device_token_auth))
        .layer(middleware::from_fn(api_key_auth));

    // WebSocket routes (require both API key and device token)
    let ws = Router::new()
        .route("/ws/session/:id", get(crate::ws::ws_upgrade))
        .layer(middleware::from_fn(device_token_auth))
        .layer(middleware::from_fn(api_key_auth));

    Router::new()
        .merge(public)
        .merge(device_routes)
        .merge(sse_routes)
        .merge(api)
        .merge(ws)
        .with_state(state)
}
