use serde::{Deserialize, Serialize};

/// Local testing types for the native adapter.
/// These will be migrated to protocol/testing.rs after Unit 1 merges.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub ok: bool,
    pub target: String,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub ok: bool,
    pub target: String,
    pub action: String,
    pub error: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitResult {
    pub ok: bool,
    pub condition: String,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// Native test adapter — resolves semantic IDs to Iced app state and message dispatch.
///
/// Phase 4 will implement the full adapter; for now all methods return NotImplemented.
pub struct NativeTestAdapter {
    // Will hold a reference to the Iced app state in Phase 4
}

impl NativeTestAdapter {
    pub fn new() -> Self {
        Self {}
    }

    /// Query app state by semantic ID
    pub fn query(&self, target: &str, _args: Option<&serde_json::Value>) -> QueryResult {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        QueryResult {
            ok: false,
            target: target.to_string(),
            data: None,
            error: Some("Native adapter not yet implemented (Phase 4)".to_string()),
            timestamp_ms: now,
        }
    }

    /// Perform a semantic action
    pub fn act(
        &self,
        target: &str,
        action: &str,
        _args: Option<&serde_json::Value>,
    ) -> ActionResult {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        ActionResult {
            ok: false,
            target: target.to_string(),
            action: action.to_string(),
            error: Some("Native adapter not yet implemented (Phase 4)".to_string()),
            timestamp_ms: now,
        }
    }

    /// Wait for a condition to be met
    pub fn wait(
        &self,
        condition: &str,
        _timeout_ms: Option<u64>,
        _poll_interval_ms: Option<u64>,
        _args: Option<&serde_json::Value>,
    ) -> WaitResult {
        WaitResult {
            ok: false,
            condition: condition.to_string(),
            timed_out: true,
            elapsed_ms: 0,
            error: Some("Native adapter not yet implemented (Phase 4)".to_string()),
        }
    }
}

impl Default for NativeTestAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_returns_not_implemented() {
        let adapter = NativeTestAdapter::new();
        let result = adapter.query("workspace.active", None);
        assert!(!result.ok);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("not yet implemented"));
    }

    #[test]
    fn act_returns_not_implemented() {
        let adapter = NativeTestAdapter::new();
        let result = adapter.act("workspace.sidebar.toggle", "click", None);
        assert!(!result.ok);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("not yet implemented"));
    }

    #[test]
    fn wait_returns_timed_out() {
        let adapter = NativeTestAdapter::new();
        let result = adapter.wait("app.ready", Some(1000), None, None);
        assert!(!result.ok);
        assert!(result.timed_out);
    }
}
