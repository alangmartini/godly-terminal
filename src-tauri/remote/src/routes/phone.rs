use axum::http::StatusCode;
use axum::response::Html;

const PHONE_HTML: &str = include_str!("../../static/phone.html");

/// GET /phone — serves the mobile web UI (no auth, API key entered in-app).
pub async fn phone_ui() -> Result<Html<&'static str>, StatusCode> {
    Ok(Html(PHONE_HTML))
}
