use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::broadcast;

use godly_protocol::{Request, Response};

use crate::daemon_client::{async_request, DaemonClient};
use crate::detection::PromptDetector;

/// SSE event types.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum SseEvent {
    #[serde(rename = "prompt_detected")]
    PromptDetected {
        session_id: String,
        matched_pattern: String,
        prompt_type: String,
        context_text: String,
    },
    #[serde(rename = "prompt_resolved")]
    PromptResolved {
        session_id: String,
    },
    #[serde(rename = "heartbeat")]
    Heartbeat {
        timestamp_ms: u64,
    },
}

/// Background event pump that monitors daemon sessions for prompt changes
/// and broadcasts SSE events to subscribers.
pub struct EventPump {
    tx: broadcast::Sender<SseEvent>,
}

impl EventPump {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    /// Get a new receiver for SSE events.
    pub fn subscribe(&self) -> broadcast::Receiver<SseEvent> {
        self.tx.subscribe()
    }

    /// Spawn the background polling task.
    pub fn spawn(self: &Arc<Self>, daemon: Arc<DaemonClient>, scan_rows: usize) {
        let tx = self.tx.clone();
        let daemon = daemon.clone();

        tokio::spawn(async move {
            let detector = PromptDetector::new();
            // Track prompt state per session: Some(pattern) = active prompt, None = no prompt
            let mut prompt_state: HashMap<String, Option<String>> = HashMap::new();
            let mut tick: u64 = 0;

            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                tick += 1;

                // Heartbeat every 15 ticks (15s)
                if tick % 15 == 0 {
                    let now_ms = now_epoch_ms();
                    let _ = tx.send(SseEvent::Heartbeat { timestamp_ms: now_ms });
                }

                // List sessions every 5 ticks (5s)
                let session_ids = if tick % 5 == 1 || tick == 1 {
                    match async_request(&daemon, Request::ListSessions).await {
                        Ok(Response::SessionList { sessions }) => {
                            let ids: Vec<String> = sessions.into_iter().map(|s| s.id).collect();
                            // Remove stale entries
                            prompt_state.retain(|k, _| ids.contains(k));
                            ids
                        }
                        _ => continue,
                    }
                } else {
                    prompt_state.keys().cloned().collect()
                };

                // Check each session for prompts
                for sid in &session_ids {
                    let resp = async_request(
                        &daemon,
                        Request::ReadGrid { session_id: sid.clone() },
                    ).await;

                    let rows = match resp {
                        Ok(Response::Grid { grid }) => grid.rows,
                        Ok(Response::Error { message }) if message.contains("not found") => {
                            // Session closed — resolve any active prompt
                            if let Some(Some(_)) = prompt_state.remove(sid) {
                                let _ = tx.send(SseEvent::PromptResolved {
                                    session_id: sid.clone(),
                                });
                            }
                            continue;
                        }
                        _ => continue,
                    };

                    let start = rows.len().saturating_sub(scan_rows);
                    let bottom_text: String = rows[start..].join("\n");

                    let detection = detector.detect(&bottom_text);
                    let prev = prompt_state.get(sid).cloned().flatten();

                    match (&detection, &prev) {
                        (Some(det), None) => {
                            // New prompt detected
                            prompt_state.insert(sid.clone(), Some(det.matched_pattern.clone()));
                            let _ = tx.send(SseEvent::PromptDetected {
                                session_id: sid.clone(),
                                matched_pattern: det.matched_pattern.clone(),
                                prompt_type: det.prompt_type.clone(),
                                context_text: det.context_text.clone(),
                            });
                        }
                        (None, Some(_)) => {
                            // Prompt resolved
                            prompt_state.insert(sid.clone(), None);
                            let _ = tx.send(SseEvent::PromptResolved {
                                session_id: sid.clone(),
                            });
                        }
                        (Some(det), Some(prev_pattern)) if det.matched_pattern != *prev_pattern => {
                            // Different prompt — resolve old, detect new
                            let _ = tx.send(SseEvent::PromptResolved {
                                session_id: sid.clone(),
                            });
                            prompt_state.insert(sid.clone(), Some(det.matched_pattern.clone()));
                            let _ = tx.send(SseEvent::PromptDetected {
                                session_id: sid.clone(),
                                matched_pattern: det.matched_pattern.clone(),
                                prompt_type: det.prompt_type.clone(),
                                context_text: det.context_text.clone(),
                            });
                        }
                        _ => {
                            // No change — ensure session is tracked
                            prompt_state.entry(sid.clone()).or_insert(None);
                        }
                    }
                }
            }
        });
    }
}

fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
