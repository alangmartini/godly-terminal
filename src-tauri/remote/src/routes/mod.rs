pub mod health;
pub mod input;
pub mod monitor;
pub mod quick_claude;
pub mod sessions;

use axum::middleware;
use axum::routing::{delete, get, post};
use axum::Router;

use crate::auth::api_key_auth;
use crate::AppState;

pub fn build_router(state: AppState) -> Router {
    // Public routes (no auth)
    let public = Router::new().route("/health", get(health::health));

    // Authenticated API routes
    let api = Router::new()
        .route("/api/sessions", get(sessions::list_sessions))
        .route("/api/sessions", post(sessions::create_session))
        .route("/api/sessions/{id}", get(sessions::get_session))
        .route("/api/sessions/{id}", delete(sessions::delete_session))
        .route("/api/sessions/{id}/grid", get(sessions::get_grid))
        .route("/api/sessions/{id}/idle", get(sessions::get_idle))
        .route("/api/sessions/{id}/write", post(input::write_to_session))
        .route("/api/sessions/{id}/resize", post(sessions::resize_session))
        .route("/api/quick-claude", post(quick_claude::quick_claude))
        .route("/api/monitor", get(monitor::list_monitors))
        .route("/api/monitor/{id}", post(monitor::start_monitor))
        .route("/api/monitor/{id}", delete(monitor::stop_monitor))
        .layer(middleware::from_fn(api_key_auth));

    // WebSocket routes (auth checked via query param or header)
    let ws = Router::new()
        .route("/ws/session/{id}", get(crate::ws::ws_upgrade))
        .layer(middleware::from_fn(api_key_auth));

    Router::new()
        .merge(public)
        .merge(api)
        .merge(ws)
        .with_state(state)
}
