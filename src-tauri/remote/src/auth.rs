use std::sync::Arc;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use crate::device_lock::DeviceLock;

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

/// Middleware that checks `X-Device-Token` header or `device_token` query param
/// against the registered device. Rejects if a device is locked and the token doesn't match.
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

    // Device is locked — require matching token
    let from_header = req
        .headers()
        .get("X-Device-Token")
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
                        (Some("device_token"), Some(v)) => urlencoding_decode(v),
                        _ => None,
                    }
                })
        });

    let provided = from_header.or(from_query);

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
