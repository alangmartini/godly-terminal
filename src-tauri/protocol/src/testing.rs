//! Shared testing protocol types for the staging autonomous test harness.
//!
//! These types are data-only and ungated — both the web (Tauri) and native
//! (iced-shell) frontends can compile against them.

use serde::{Deserialize, Serialize};

/// Status of the test harness runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestHarnessStatus {
    pub ready: bool,
    /// "web" or "native"
    pub frontend_type: String,
    pub harness_mode: bool,
    pub run_id: Option<String>,
    pub uptime_ms: u64,
}

/// Manifest describing collected test artifacts for a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub run_id: String,
    pub created_at: String,
    pub artifact_dir: String,
    pub files: Vec<String>,
}

/// Full dump of application state for test assertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDump {
    pub workspaces: serde_json::Value,
    pub terminals: serde_json::Value,
    pub layout_trees: serde_json::Value,
    pub daemon_sessions: serde_json::Value,
    pub active_workspace_id: Option<String>,
    pub active_terminal_id: Option<String>,
}
