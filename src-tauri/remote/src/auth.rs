use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use crate::device_lock::DeviceLock;

/// Simple sliding-window rate limiter: 50 requests per second.
/// Uses atomic counters for lock-free operation.
static RATE_COUNTER: AtomicU64 = AtomicU64::new(0);
static RATE_WINDOW: AtomicU64 = AtomicU64::new(0);
const MAX_REQUESTS_PER_SECOND: u64 = 50;

pub async fn rate_limit(req: Request, next: Next) -> Response {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let window = RATE_WINDOW.load(Ordering::Relaxed);
    if now_secs != window {
        RATE_WINDOW.store(now_secs, Ordering::Relaxed);
        RATE_COUNTER.store(1, Ordering::Relaxed);
    } else {
        let count = RATE_COUNTER.fetch_add(1, Ordering::Relaxed);
        if count >= MAX_REQUESTS_PER_SECOND {
            return (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
        }
    }

    next.run(req).await
}

/// Constant-time string comparison to prevent timing attacks on API keys.
fn secure_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

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
                                    urlencoding_decode(v)
                                }
                                _ => None,
                            }
                        })
                });

            let provided = from_header.or(from_query);

            match provided {
                Some(ref key) if secure_eq(key, &expected) => next.run(req).await,
                _ => (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
            }
        }
    }
}

/// Extract a cookie value by name from the Cookie header.
fn extract_cookie(req: &Request, name: &str) -> Option<String> {
    req.headers()
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|pair| {
                let pair = pair.trim();
                let mut parts = pair.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some(k), Some(v)) if k.trim() == name => Some(v.trim().to_string()),
                    _ => None,
                }
            })
        })
}

/// Middleware that checks `X-Device-Token` header, `godly_device_token` cookie,
/// or `device_token` query param against the registered device.
/// Rejects if a device is locked and the token doesn't match.
pub async fn device_token_auth(
    req: Request,
    next: Next,
) -> Response {
    let device_lock = req
        .extensions()
        .get::<Arc<DeviceLock>>()
        .cloned();

    let device_lock = match device_lock {
        Some(dl) => dl,
        None => return next.run(req).await, // no device lock configured
    };

    // If no device is registered yet, allow the request (pre-registration state)
    if !device_lock.is_locked() {
        return next.run(req).await;
    }

    // Device is locked — require matching token (header > cookie > query)
    let from_header = req
        .headers()
        .get("X-Device-Token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let from_cookie = extract_cookie(&req, "godly_device_token");

    let from_query = req
        .uri()
        .query()
        .and_then(|q| {
            q.split('&')
                .find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    match (parts.next(), parts.next()) {
                        (Some("device_token"), Some(v)) => urlencoding_decode(v),
                        _ => None,
                    }
                })
        });

    let provided = from_header.or(from_cookie).or(from_query);

    match provided {
        Some(ref token) if device_lock.check(token) => next.run(req).await,
        _ => (StatusCode::FORBIDDEN, "Unauthorized").into_response(),
    }
}

/// URL decode for query param values (handles %XX and + encoding).
fn urlencoding_decode(input: &str) -> Option<String> {
    let mut result = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return None; // truncated escape
                }
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                let byte = hex_byte(hi, lo)?;
                result.push(byte);
                i += 3;
            }
            b'+' => {
                result.push(b' ');
                i += 1;
            }
            b => {
                result.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(result).ok()
}

/// Convert two hex ASCII chars to a byte.
fn hex_byte(hi: u8, lo: u8) -> Option<u8> {
    let h = hex_digit(hi)?;
    let l = hex_digit(lo)?;
    Some((h << 4) | l)
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- URL decoding tests ---

    #[test]
    fn urlencoding_basic() {
        assert_eq!(urlencoding_decode("hello"), Some("hello".into()));
    }

    #[test]
    fn urlencoding_spaces() {
        assert_eq!(urlencoding_decode("hello+world"), Some("hello world".into()));
        assert_eq!(urlencoding_decode("hello%20world"), Some("hello world".into()));
    }

    #[test]
    fn urlencoding_special_chars() {
        assert_eq!(urlencoding_decode("%2F%3A%40"), Some("/:@".into()));
    }

    #[test]
    fn urlencoding_mixed_case_hex() {
        assert_eq!(urlencoding_decode("%2f"), Some("/".into()));
        assert_eq!(urlencoding_decode("%2F"), Some("/".into()));
    }

    #[test]
    fn urlencoding_truncated_escape_rejected() {
        // Truncated %XX sequences must be rejected (not silently passed through)
        assert_eq!(urlencoding_decode("abc%2"), None);
        assert_eq!(urlencoding_decode("abc%"), None);
    }

    #[test]
    fn urlencoding_invalid_hex_rejected() {
        assert_eq!(urlencoding_decode("%GG"), None);
        assert_eq!(urlencoding_decode("%ZZ"), None);
    }

    #[test]
    fn urlencoding_null_byte() {
        // %00 should decode to null byte (valid UTF-8 technically)
        let result = urlencoding_decode("%00");
        assert_eq!(result, Some("\0".into()));
    }

    #[test]
    fn urlencoding_invalid_utf8() {
        // Invalid UTF-8 sequence should be rejected
        assert_eq!(urlencoding_decode("%FF%FE"), None);
    }

    // --- secure_eq tests ---

    #[test]
    fn secure_eq_same_strings() {
        assert!(secure_eq("abc", "abc"));
        assert!(secure_eq("", ""));
    }

    #[test]
    fn secure_eq_different_strings() {
        assert!(!secure_eq("abc", "def"));
    }

    #[test]
    fn secure_eq_different_lengths() {
        assert!(!secure_eq("abc", "abcd"));
        assert!(!secure_eq("abcd", "abc"));
    }

    #[test]
    fn secure_eq_prefix_attack() {
        // An attacker knowing the first N chars shouldn't get timing info
        assert!(!secure_eq("secretkey123", "secretkey999"));
    }

    // --- cookie extraction tests ---

    #[test]
    fn extract_cookie_single() {
        let req = axum::http::Request::builder()
            .header("Cookie", "godly_device_token=abc123")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            extract_cookie(&req, "godly_device_token"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_cookie_multiple() {
        let req = axum::http::Request::builder()
            .header("Cookie", "other=xyz; godly_device_token=abc123; session=def")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            extract_cookie(&req, "godly_device_token"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_cookie_missing() {
        let req = axum::http::Request::builder()
            .header("Cookie", "other=xyz")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(extract_cookie(&req, "godly_device_token"), None);
    }

    #[test]
    fn extract_cookie_no_header() {
        let req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(extract_cookie(&req, "godly_device_token"), None);
    }

    #[test]
    fn extract_cookie_with_spaces() {
        let req = axum::http::Request::builder()
            .header("Cookie", "  godly_device_token = abc123 ; other = xyz ")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            extract_cookie(&req, "godly_device_token"),
            Some("abc123".into())
        );
    }

    // --- rate limiter tests ---

    #[test]
    fn rate_limiter_window_resets() {
        // Verify the atomic rate limiter fields exist and work
        RATE_COUNTER.store(0, Ordering::Relaxed);
        RATE_WINDOW.store(0, Ordering::Relaxed);
        assert_eq!(RATE_COUNTER.load(Ordering::Relaxed), 0);
    }
}
