use parking_lot::RwLock;
use std::time::Instant;

use godly_protocol::testing::TestHarnessStatus;

/// Central service for the staging test harness.
///
/// Detects harness mode from the `GODLY_TEST_HARNESS=1` environment variable
/// and tracks test run lifecycle (start/end, run IDs, uptime).
pub struct TestHarnessService {
    harness_mode: bool,
    start_time: Instant,
    current_run_id: RwLock<Option<String>>,
}

impl TestHarnessService {
    /// Create a new test harness service.
    /// Reads `GODLY_TEST_HARNESS` from the environment to determine if
    /// harness mode is active.
    pub fn new() -> Self {
        let harness_mode = std::env::var("GODLY_TEST_HARNESS")
            .map(|v| v == "1")
            .unwrap_or(false);

        Self {
            harness_mode,
            start_time: Instant::now(),
            current_run_id: RwLock::new(None),
        }
    }

    /// Whether the harness is ready to accept test commands.
    pub fn is_ready(&self) -> bool {
        self.harness_mode
    }

    /// Return the current status of the test harness.
    pub fn status(&self) -> TestHarnessStatus {
        let frontend_type = if cfg!(feature = "native-frontend") {
            "native".to_string()
        } else {
            "web".to_string()
        };

        TestHarnessStatus {
            ready: self.harness_mode,
            frontend_type,
            harness_mode: self.harness_mode,
            run_id: self.current_run_id.read().clone(),
            uptime_ms: self.start_time.elapsed().as_millis() as u64,
        }
    }

    /// Start a new test run. Generates and returns a UUID run ID.
    pub fn start_run(&self) -> String {
        let run_id = uuid::Uuid::new_v4().to_string();
        *self.current_run_id.write() = Some(run_id.clone());
        run_id
    }

    /// End the current test run, clearing the run ID.
    pub fn end_run(&self) {
        *self.current_run_id.write() = None;
    }

    /// Get the current run ID, if any.
    pub fn current_run_id(&self) -> Option<String> {
        self.current_run_id.read().clone()
    }
}
