use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use godly_protocol::Request;
use godly_protocol::Response;

use crate::daemon_client::async_request;
use crate::AppState;

/// Maximum write payload size (64 KB). Prevents memory exhaustion from oversized requests.
const MAX_WRITE_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
pub struct WriteRequest {
    pub data: String,
}

pub async fn write_to_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<WriteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if body.data.len() > MAX_WRITE_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Write payload exceeds {} byte limit", MAX_WRITE_BYTES),
        ));
    }

    // Convert \n → \r for PTY (same as MCP handler)
    let converted = body.data.replace("\r\n", "\r").replace('\n', "\r");

    let resp = async_request(
        &state.daemon,
        Request::Write {
            session_id: id,
            data: converted.into_bytes(),
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
