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

/// Newtype for semantic target identifiers (e.g. "workspace.active", "tab.active").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTarget(pub String);

/// A query against the semantic testing API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    pub target: String,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

/// An action to perform via the semantic testing API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticAction {
    pub target: String,
    pub action: String,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

/// A wait condition for the semantic testing API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticWait {
    pub condition: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub poll_interval_ms: Option<u64>,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
}

/// Result of a semantic query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub ok: bool,
    pub target: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    pub timestamp_ms: u64,
}

/// Result of a semantic action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub target: String,
    pub action: String,
    #[serde(default)]
    pub error: Option<String>,
    pub timestamp_ms: u64,
}

/// Result of a semantic wait.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitResult {
    pub ok: bool,
    pub condition: String,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
}
