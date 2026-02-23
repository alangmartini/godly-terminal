use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub device_token: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct DeviceStatus {
    pub locked: bool,
}

/// POST /api/register-device — Lock the server to the first device that calls this.
pub async fn register_device(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, (StatusCode, Json<RegisterResponse>)> {
    if body.device_token.len() < 16 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(RegisterResponse {
                ok: false,
                message: "Device token must be at least 16 characters".into(),
            }),
        ));
    }

    match state.device_lock.register(&body.device_token) {
        Ok(()) => Ok(Json(RegisterResponse {
            ok: true,
            message: "Device registered".into(),
        })),
        Err(msg) => Err((
            StatusCode::FORBIDDEN,
            Json(RegisterResponse {
                ok: false,
                message: msg.into(),
            }),
        )),
    }
}

/// GET /api/device-status — Check if a device is registered.
pub async fn device_status(
    State(state): State<AppState>,
) -> Json<DeviceStatus> {
    Json(DeviceStatus {
        locked: state.device_lock.is_locked(),
    })
}
