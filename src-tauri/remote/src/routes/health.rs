use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub daemon_connected: bool,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let connected = state.daemon.is_connected();
    Json(HealthResponse {
        status: "ok",
        daemon_connected: connected,
    })
}
