use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;

use godly_protocol::{Request, Response};

use crate::daemon_client::async_request;
use crate::detection::PromptDetector;
use crate::AppState;

#[derive(Serialize, Clone)]
pub struct WebhookPayload {
    #[serde(rename = "type")]
    pub event_type: String,
    pub session_id: String,
    pub matched_pattern: String,
    pub grid_text: String,
    pub timestamp_ms: u64,
}

/// Tracks all active monitors. Keyed by session_id.
pub struct MonitorRegistry {
    pub active: RwLock<HashMap<String, tokio::task::JoinHandle<()>>>,
}

impl MonitorRegistry {
    pub fn new() -> Self {
        Self {
            active: RwLock::new(HashMap::new()),
        }
    }
}

/// Compute HMAC-SHA256 signature for webhook payload.
fn compute_webhook_signature(secret: &str, body: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(body);
    let result = mac.finalize();
    let bytes = result.into_bytes();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Start monitoring a session for permission prompts.
pub fn start_monitor(state: AppState, session_id: String) {
    let poll_ms = state.config.monitor.poll_interval_ms;
    let scan_rows = state.config.monitor.scan_rows;
    let webhook_url = state.config.monitor.webhook_url.clone();
    let webhook_secret = state.config.monitor.webhook_secret.clone();
    let daemon = Arc::clone(&state.daemon);
    let monitors = Arc::clone(&state.monitors);

    let sid = session_id.clone();
    let handle = tokio::spawn(async move {
        let detector = PromptDetector::new();
        let mut last_match_time: u64 = 0;
        let cooldown_ms: u64 = 30_000;

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;

            // Read grid
            let resp = async_request(
                &daemon,
                Request::ReadGrid {
                    session_id: sid.clone(),
                },
            )
            .await;

            let rows = match resp {
                Ok(Response::Grid { grid }) => grid.rows,
                Ok(Response::Error { message }) => {
                    if message.contains("not found") {
                        tracing::info!("Session {} closed, stopping monitor", sid);
                        break;
                    }
                    continue;
                }
                _ => continue,
            };

            // Scan bottom N rows
            let start = rows.len().saturating_sub(scan_rows);
            let bottom_text: String = rows[start..].join("\n");

            if let Some(detection) = detector.detect(&bottom_text) {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                // Cooldown to prevent duplicate notifications
                if now_ms.saturating_sub(last_match_time) < cooldown_ms {
                    continue;
                }
                last_match_time = now_ms;

                tracing::info!(
                    "Permission prompt detected in session {}: {}",
                    sid,
                    detection.matched_pattern
                );

                if let Some(ref url) = webhook_url {
                    let payload = WebhookPayload {
                        event_type: "permission_prompt".to_string(),
                        session_id: sid.clone(),
                        matched_pattern: detection.matched_pattern,
                        grid_text: bottom_text.clone(),
                        timestamp_ms: now_ms,
                    };

                    let client = reqwest::Client::new();
                    let mut req = client.post(url).json(&payload);

                    // Sign the webhook payload if a secret is configured
                    if let Some(ref secret) = webhook_secret {
                        if let Ok(body_json) = serde_json::to_vec(&payload) {
                            let sig = compute_webhook_signature(secret, &body_json);
                            req = req.header("X-Webhook-Signature", format!("sha256={}", sig));
                        }
                    }

                    if let Err(e) = req.send().await {
                        tracing::error!("Failed to send webhook: {}", e);
                    }
                }
            }
        }

        // Clean up
        monitors.active.write().await.remove(&sid);
    });

    // Store the handle
    let monitors = Arc::clone(&state.monitors);
    let sid2 = session_id.clone();
    tokio::spawn(async move {
        monitors.active.write().await.insert(sid2, handle);
    });
}

/// Stop monitoring a session.
pub async fn stop_monitor(state: &AppState, session_id: &str) -> bool {
    let mut active = state.monitors.active.write().await;
    if let Some(handle) = active.remove(session_id) {
        handle.abort();
        true
    } else {
        false
    }
}

/// List all actively monitored session IDs.
pub async fn list_monitors(state: &AppState) -> Vec<String> {
    state
        .monitors
        .active
        .read()
        .await
        .keys()
        .cloned()
        .collect()
}

// Tests for prompt detection are in detection.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_signature_deterministic() {
        let sig1 = compute_webhook_signature("secret", b"hello");
        let sig2 = compute_webhook_signature("secret", b"hello");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn webhook_signature_differs_with_different_secret() {
        let sig1 = compute_webhook_signature("secret1", b"hello");
        let sig2 = compute_webhook_signature("secret2", b"hello");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn webhook_signature_differs_with_different_body() {
        let sig1 = compute_webhook_signature("secret", b"hello");
        let sig2 = compute_webhook_signature("secret", b"world");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn webhook_signature_is_hex() {
        let sig = compute_webhook_signature("key", b"data");
        assert!(sig.len() == 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn webhook_signature_known_vector() {
        // Verify against known HMAC-SHA256 test vector
        // HMAC-SHA256("key", "The quick brown fox jumps over the lazy dog")
        // = f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8
        let sig = compute_webhook_signature(
            "key",
            b"The quick brown fox jumps over the lazy dog",
        );
        assert_eq!(
            sig,
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn webhook_payload_serializes() {
        let payload = WebhookPayload {
            event_type: "permission_prompt".to_string(),
            session_id: "test-123".to_string(),
            matched_pattern: "Allow?".to_string(),
            grid_text: "some text".to_string(),
            timestamp_ms: 1234567890,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"type\":\"permission_prompt\""));
        assert!(json.contains("\"session_id\":\"test-123\""));
    }
}
