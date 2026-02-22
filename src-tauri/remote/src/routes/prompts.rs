use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use godly_protocol::{Request, Response};

use crate::daemon_client::async_request;
use crate::detection::PromptDetector;
use crate::AppState;

#[derive(Serialize)]
pub struct PromptItem {
    pub session_id: String,
    pub matched_pattern: String,
    pub prompt_type: String,
    pub context_text: String,
}

#[derive(Serialize)]
pub struct PromptsResponse {
    pub prompts: Vec<PromptItem>,
}

/// GET /api/prompts
/// Scan all live sessions for active prompts.
pub async fn list_prompts(
    State(state): State<AppState>,
) -> Result<Json<PromptsResponse>, (StatusCode, String)> {
    let scan_rows = state.config.monitor.scan_rows;

    // Get all live sessions
    let sessions = match async_request(&state.daemon, Request::ListSessions).await {
        Ok(Response::SessionList { sessions }) => sessions,
        Ok(Response::Error { message }) => return Err((StatusCode::BAD_GATEWAY, message)),
        _ => return Err((StatusCode::BAD_GATEWAY, "Unexpected daemon response".to_string())),
    };

    let detector = PromptDetector::new();
    let mut prompts = Vec::new();

    for session in sessions {
        let resp = async_request(
            &state.daemon,
            Request::ReadGrid { session_id: session.id.clone() },
        ).await;

        let rows = match resp {
            Ok(Response::Grid { grid }) => grid.rows,
            _ => continue,
        };

        let start = rows.len().saturating_sub(scan_rows);
        let bottom_text: String = rows[start..].join("\n");

        if let Some(det) = detector.detect(&bottom_text) {
            prompts.push(PromptItem {
                session_id: session.id,
                matched_pattern: det.matched_pattern,
                prompt_type: det.prompt_type,
                context_text: det.context_text,
            });
        }
    }

    Ok(Json(PromptsResponse { prompts }))
}

/// GET /api/sessions/:id/prompts
/// Check a single session for active prompts.
pub async fn session_prompts(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PromptsResponse>, (StatusCode, String)> {
    let scan_rows = state.config.monitor.scan_rows;

    let resp = async_request(
        &state.daemon,
        Request::ReadGrid { session_id: id.clone() },
    ).await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    let rows = match resp {
        Response::Grid { grid } => grid.rows,
        Response::Error { message } => return Err((StatusCode::BAD_REQUEST, message)),
        other => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    };

    let start = rows.len().saturating_sub(scan_rows);
    let bottom_text: String = rows[start..].join("\n");

    let detector = PromptDetector::new();
    let prompts = match detector.detect(&bottom_text) {
        Some(det) => vec![PromptItem {
            session_id: id,
            matched_pattern: det.matched_pattern,
            prompt_type: det.prompt_type,
            context_text: det.context_text,
        }],
        None => Vec::new(),
    };

    Ok(Json(PromptsResponse { prompts }))
}
