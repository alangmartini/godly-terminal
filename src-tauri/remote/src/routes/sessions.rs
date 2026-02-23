use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use godly_protocol::{Request, Response, ShellType};

use crate::daemon_client::async_request;
use crate::AppState;

#[derive(Serialize)]
pub struct SessionItem {
    pub id: String,
    pub shell_type: String,
}

#[derive(Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionItem>,
}

pub async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<SessionListResponse>, (StatusCode, String)> {
    let resp = async_request(&state.daemon, Request::ListSessions)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::SessionList { sessions } => Ok(Json(SessionListResponse {
            sessions: sessions
                .into_iter()
                .map(|s| SessionItem {
                    id: s.id,
                    shell_type: s.shell_type.display_name(),
                })
                .collect(),
        })),
        Response::Error { message } => Err((StatusCode::BAD_GATEWAY, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub shell_type: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "default_rows")]
    pub rows: u16,
    #[serde(default = "default_cols")]
    pub cols: u16,
}

fn default_rows() -> u16 {
    24
}
fn default_cols() -> u16 {
    80
}

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
}

pub async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<CreateSessionResponse>), (StatusCode, String)> {
    // Validate rows/cols to prevent resource exhaustion
    let rows = body.rows.clamp(1, 500);
    let cols = body.cols.clamp(1, 500);

    // Validate cwd if provided — must be an existing directory, no path traversal
    let cwd = match &body.cwd {
        Some(path) => {
            let p = std::path::Path::new(path);
            // Reject relative paths and path traversal
            if !p.is_absolute() {
                return Err((StatusCode::BAD_REQUEST, "cwd must be an absolute path".into()));
            }
            if path.contains("..") {
                return Err((StatusCode::BAD_REQUEST, "cwd must not contain path traversal".into()));
            }
            if !p.is_dir() {
                return Err((StatusCode::BAD_REQUEST, "cwd directory does not exist".into()));
            }
            Some(path.clone())
        }
        None => None,
    };

    // Validate shell_type
    let shell = match body.shell_type.as_deref() {
        Some("wsl") => ShellType::Wsl { distribution: None },
        Some("windows") | None => ShellType::Windows,
        Some(other) => {
            return Err((StatusCode::BAD_REQUEST, format!("Unknown shell_type: {}", other)));
        }
    };

    let session_id = uuid::Uuid::new_v4().to_string();

    let mut env_vars = HashMap::new();
    env_vars.insert("GODLY_SESSION_ID".to_string(), session_id.clone());

    let create_req = Request::CreateSession {
        id: session_id.clone(),
        shell_type: shell,
        cwd,
        rows,
        cols,
        env: Some(env_vars),
    };

    let resp = async_request(&state.daemon, create_req)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::SessionCreated { .. } => {
            // Attach so we receive output events
            let _ = async_request(
                &state.daemon,
                Request::Attach {
                    session_id: session_id.clone(),
                },
            )
            .await;

            Ok((
                StatusCode::CREATED,
                Json(CreateSessionResponse { session_id }),
            ))
        }
        Response::Error { message } => Err((StatusCode::BAD_REQUEST, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SessionItem>, (StatusCode, String)> {
    let resp = async_request(&state.daemon, Request::ListSessions)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::SessionList { sessions } => {
            match sessions.into_iter().find(|s| s.id == id) {
                Some(s) => Ok(Json(SessionItem {
                    id: s.id,
                    shell_type: s.shell_type.display_name(),
                })),
                None => Err((StatusCode::NOT_FOUND, format!("Session {} not found", id))),
            }
        }
        Response::Error { message } => Err((StatusCode::BAD_GATEWAY, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

pub async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let resp = async_request(
        &state.daemon,
        Request::CloseSession {
            session_id: id,
        },
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::Ok => Ok(StatusCode::NO_CONTENT),
        Response::Error { message } => Err((StatusCode::BAD_REQUEST, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

#[derive(Serialize)]
pub struct GridResponse {
    pub rows: Vec<String>,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cols: u16,
    pub num_rows: u16,
}

pub async fn get_grid(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GridResponse>, (StatusCode, String)> {
    let resp = async_request(
        &state.daemon,
        Request::ReadGrid {
            session_id: id,
        },
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::Grid { grid } => Ok(Json(GridResponse {
            rows: grid.rows,
            cursor_row: grid.cursor_row,
            cursor_col: grid.cursor_col,
            cols: grid.cols,
            num_rows: grid.num_rows,
        })),
        Response::Error { message } => Err((StatusCode::BAD_REQUEST, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

#[derive(Serialize)]
pub struct IdleResponse {
    pub idle_ms: u64,
    pub running: bool,
}

pub async fn get_idle(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<IdleResponse>, (StatusCode, String)> {
    let resp = async_request(
        &state.daemon,
        Request::GetLastOutputTime {
            session_id: id,
        },
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::LastOutputTime { epoch_ms, running } => {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let idle_ms = now_ms.saturating_sub(epoch_ms);
            Ok(Json(IdleResponse { idle_ms, running }))
        }
        Response::Error { message } => Err((StatusCode::BAD_REQUEST, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

#[derive(Deserialize)]
pub struct ResizeRequest {
    pub rows: u16,
    pub cols: u16,
}

pub async fn resize_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ResizeRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let rows = body.rows.clamp(1, 500);
    let cols = body.cols.clamp(1, 500);

    let resp = async_request(
        &state.daemon,
        Request::Resize {
            session_id: id,
            rows,
            cols,
        },
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::Ok => Ok(StatusCode::NO_CONTENT),
        Response::Error { message } => Err((StatusCode::BAD_REQUEST, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}

#[derive(Deserialize)]
pub struct TextQuery {
    #[serde(default = "default_text_lines")]
    pub lines: usize,
}

fn default_text_lines() -> usize {
    50
}

#[derive(Serialize)]
pub struct TextResponse {
    pub lines: Vec<String>,
    pub total_rows: usize,
}

/// GET /api/sessions/:id/text?lines=50
/// Returns last N lines of terminal output as plain text (strips empty trailing lines).
pub async fn get_text(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TextQuery>,
) -> Result<Json<TextResponse>, (StatusCode, String)> {
    let resp = async_request(
        &state.daemon,
        Request::ReadGrid {
            session_id: id,
        },
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    match resp {
        Response::Grid { grid } => {
            let total_rows = grid.rows.len();

            // Strip empty trailing lines
            let mut rows = grid.rows;
            while rows.last().is_some_and(|r| r.trim().is_empty()) {
                rows.pop();
            }

            // Take last N lines (capped at 200 to limit data exposure)
            let n = query.lines.min(200).min(rows.len());
            let lines: Vec<String> = rows[rows.len().saturating_sub(n)..].to_vec();

            Ok(Json(TextResponse { lines, total_rows }))
        }
        Response::Error { message } => Err((StatusCode::BAD_REQUEST, message)),
        other => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unexpected response: {:?}", other),
        )),
    }
}
