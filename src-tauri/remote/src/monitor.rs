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

/// Start monitoring a session for permission prompts.
pub fn start_monitor(state: AppState, session_id: String) {
    let poll_ms = state.config.monitor.poll_interval_ms;
    let scan_rows = state.config.monitor.scan_rows;
    let webhook_url = state.config.monitor.webhook_url.clone();
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
                    if let Err(e) = client.post(url).json(&payload).send().await {
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
