use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::device_lock::DeviceLock;
use crate::AppState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    /// Client may provide its own token (backward compat), or omit to let server generate one.
    #[serde(default)]
    pub device_token: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct DeviceStatus {
    pub locked: bool,
    pub password_required: bool,
}

/// Generate a cryptographically random device token.
fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// POST /api/register-device — Lock the server to the first device that calls this.
/// Server generates the device token and returns it via HttpOnly Set-Cookie.
/// Requires password if one is configured.
pub async fn register_device(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(HeaderMap, Json<RegisterResponse>), (StatusCode, Json<RegisterResponse>)> {
    // Use client-provided token for backward compat, or generate server-side
    let token = body
        .device_token
        .filter(|t| t.len() >= 16)
        .unwrap_or_else(generate_token);

    if token.len() < 16 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(RegisterResponse {
                ok: false,
                message: "Device token must be at least 16 characters".into(),
            }),
        ));
    }

    // Verify password first
    let password = body.password.as_deref().unwrap_or("");
    if let Err(msg) = state.device_lock.verify_password(password) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(RegisterResponse {
                ok: false,
                message: msg.into(),
            }),
        ));
    }

    match state.device_lock.register(&token) {
        Ok(()) => {
            // Set HttpOnly cookie so the token is never accessible to JS
            let cookie = format!(
                "godly_device_token={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800",
                token
            );
            let mut headers = HeaderMap::new();
            if let Ok(val) = HeaderValue::from_str(&cookie) {
                headers.insert(axum::http::header::SET_COOKIE, val);
            }
            Ok((
                headers,
                Json(RegisterResponse {
                    ok: true,
                    message: "Device registered".into(),
                }),
            ))
        }
        Err(msg) => Err((
            StatusCode::FORBIDDEN,
            Json(RegisterResponse {
                ok: false,
                message: msg.into(),
            }),
        )),
    }
}

/// GET /api/device-status — Check if a device is registered and if password is required.
pub async fn device_status(
    State(state): State<AppState>,
) -> Json<DeviceStatus> {
    Json(DeviceStatus {
        locked: state.device_lock.is_locked(),
        password_required: state.device_lock.has_password(),
    })
}

#[derive(Deserialize)]
pub struct AuthenticateRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthenticateResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// POST /api/authenticate — Exchange a password for the API key.
/// This is a public endpoint (no auth middleware) so browser users can
/// authenticate by entering a password instead of copy-pasting an API key.
pub async fn authenticate(
    Extension(device_lock): Extension<Arc<DeviceLock>>,
    Extension(api_key): Extension<Option<String>>,
    Json(body): Json<AuthenticateRequest>,
) -> Result<Json<AuthenticateResponse>, (StatusCode, Json<AuthenticateResponse>)> {
    if !device_lock.has_password() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthenticateResponse {
                ok: false,
                api_key: None,
                message: Some("No password configured".into()),
            }),
        ));
    }

    if let Err(msg) = device_lock.verify_password(&body.password) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(AuthenticateResponse {
                ok: false,
                api_key: None,
                message: Some(msg.into()),
            }),
        ));
    }

    Ok(Json(AuthenticateResponse {
        ok: true,
        api_key,
        message: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_is_uuid_format() {
        let token = generate_token();
        assert!(token.len() >= 36); // UUID v4 = 36 chars with hyphens
        assert!(uuid::Uuid::parse_str(&token).is_ok());
    }

    #[test]
    fn generate_token_is_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn generate_token_is_long_enough() {
        let token = generate_token();
        // UUID v4 is 36 chars, well above the 16-char minimum
        assert!(token.len() >= 16);
    }

    #[test]
    fn register_request_accepts_no_token() {
        // Server should generate token when client doesn't provide one
        let json = r#"{"password": "test123"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert!(req.device_token.is_none());
    }

    #[test]
    fn register_request_accepts_client_token() {
        let json = r#"{"device_token": "abcdefghijklmnopqrstuvwxyz123456", "password": "test"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert!(req.device_token.is_some());
        assert!(req.device_token.unwrap().len() >= 16);
    }

    #[test]
    fn set_cookie_format_is_httponly() {
        let token = generate_token();
        let cookie = format!(
            "godly_device_token={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800",
            token
        );
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=604800"));
        assert!(!cookie.contains("Secure")); // Not required for localhost/ngrok
    }

    #[test]
    fn authenticate_request_deserializes() {
        let json = r#"{"password": "hunter2"}"#;
        let req: AuthenticateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.password, "hunter2");
    }

    #[test]
    fn authenticate_response_success_serializes() {
        let resp = AuthenticateResponse {
            ok: true,
            api_key: Some("abc123".into()),
            message: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""ok":true"#));
        assert!(json.contains(r#""api_key":"abc123""#));
        // message should be omitted when None
        assert!(!json.contains("message"));
    }

    #[test]
    fn authenticate_response_failure_serializes() {
        let resp = AuthenticateResponse {
            ok: false,
            api_key: None,
            message: Some("Invalid password".into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""ok":false"#));
        assert!(json.contains(r#""message":"Invalid password""#));
        // api_key should be omitted when None
        assert!(!json.contains("api_key"));
    }
}
