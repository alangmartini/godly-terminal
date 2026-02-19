use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Middleware that checks `X-API-Key` header against the configured key.
/// If no key is configured (dev mode), all requests are allowed.
pub async fn api_key_auth(
    req: Request,
    next: Next,
) -> Response {
    let expected_key = req
        .extensions()
        .get::<Option<String>>()
        .cloned()
        .flatten();

    match expected_key {
        None => next.run(req).await,
        Some(expected) => {
            let provided = req
                .headers()
                .get("X-API-Key")
                .and_then(|v| v.to_str().ok());

            match provided {
                Some(key) if key == expected => next.run(req).await,
                _ => (StatusCode::UNAUTHORIZED, "Invalid or missing API key").into_response(),
            }
        }
    }
}
