use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Middleware that checks `X-API-Key` header or `api_key` query param against the configured key.
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
            // Check header first, then query param (needed for EventSource which can't set headers)
            let from_header = req
                .headers()
                .get("X-API-Key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let from_query = req
                .uri()
                .query()
                .and_then(|q| {
                    q.split('&')
                        .find_map(|pair| {
                            let mut parts = pair.splitn(2, '=');
                            match (parts.next(), parts.next()) {
                                (Some("api_key"), Some(v)) => {
                                    // URL-decode the value
                                    urlencoding_decode(v)
                                }
                                _ => None,
                            }
                        })
                });

            let provided = from_header.or(from_query);

            match provided {
                Some(ref key) if key == &expected => next.run(req).await,
                _ => (StatusCode::UNAUTHORIZED, "Invalid or missing API key").into_response(),
            }
        }
    }
}

/// Simple URL decode for query param values (handles %XX encoding).
fn urlencoding_decode(input: &str) -> Option<String> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        match b {
            b'%' => {
                let hi = chars.next()?;
                let lo = chars.next()?;
                let hex = [hi, lo];
                let s = std::str::from_utf8(&hex).ok()?;
                let byte = u8::from_str_radix(s, 16).ok()?;
                result.push(byte as char);
            }
            b'+' => result.push(' '),
            _ => result.push(b as char),
        }
    }
    Some(result)
}
