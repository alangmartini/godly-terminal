use std::collections::HashMap;

use axum::extract::{Path, State};
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
    let session_id = uuid::Uuid::new_v4().to_string();
    let shell = match body.shell_type.as_deref() {
        Some("wsl") => ShellType::Wsl { distribution: None },
        _ => ShellType::Windows,
    };

    let mut env_vars = HashMap::new();
    env_vars.insert("GODLY_SESSION_ID".to_string(), session_id.clone());

    let create_req = Request::CreateSession {
        id: session_id.clone(),
        shell_type: shell,
        cwd: body.cwd,
        rows: body.rows,
        cols: body.cols,
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
    let resp = async_request(
        &state.daemon,
        Request::Resize {
            session_id: id,
            rows: body.rows,
            cols: body.cols,
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
