use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct MonitorListResponse {
    pub monitors: Vec<String>,
}

pub async fn list_monitors(
    State(state): State<AppState>,
) -> Json<MonitorListResponse> {
    let monitors = crate::monitor::list_monitors(&state).await;
    Json(MonitorListResponse { monitors })
}

#[derive(Serialize)]
pub struct MonitorStartResponse {
    pub session_id: String,
    pub status: &'static str,
}

pub async fn start_monitor(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<MonitorStartResponse>), (StatusCode, String)> {
    // Check if already monitoring
    let active = state.monitors.active.read().await;
    if active.contains_key(&id) {
        return Err((
            StatusCode::CONFLICT,
            format!("Already monitoring session {}", id),
        ));
    }
    drop(active);

    // Check webhook is configured
    if state.config.monitor.webhook_url.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No webhook_url configured. Set monitor.webhook_url in config or GODLY_REMOTE_WEBHOOK_URL env var".to_string(),
        ));
    }

    crate::monitor::start_monitor(state.clone(), id.clone());

    Ok((
        StatusCode::CREATED,
        Json(MonitorStartResponse {
            session_id: id,
            status: "started",
        }),
    ))
}

pub async fn stop_monitor(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    if crate::monitor::stop_monitor(&state, &id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("No active monitor for session {}", id),
        ))
    }
}
