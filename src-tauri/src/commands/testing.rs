use std::sync::Arc;

use crate::state::AppState;
use crate::testing::harness::TestHarnessService;

use godly_protocol::testing::TestHarnessStatus;

#[tauri::command]
pub fn test_harness_status(
    harness: tauri::State<'_, Arc<TestHarnessService>>,
) -> TestHarnessStatus {
    harness.status()
}

#[tauri::command]
pub fn reset_staging_profile() -> Result<(), String> {
    // Stub — returns Ok for now. Will be implemented to clear staging-specific
    // state (persisted layout, scrollback, etc.) in a future unit.
    Ok(())
}

#[tauri::command]
pub fn export_state_dump(
    app_state: tauri::State<'_, Arc<AppState>>,
) -> serde_json::Value {
    let dump = crate::testing::state_dump::dump_app_state(&app_state);
    serde_json::to_value(dump).unwrap_or_default()
}

#[tauri::command]
pub fn collect_artifact_bundle(
    harness: tauri::State<'_, Arc<TestHarnessService>>,
    run_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let run_id = run_id
        .or_else(|| harness.current_run_id())
        .unwrap_or_else(|| harness.start_run());

    // Use a well-known test artifacts directory under APPDATA
    let base_dir = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    let artifacts_dir = std::path::PathBuf::from(base_dir)
        .join(format!("com.godly.terminal{}", godly_protocol::instance_suffix()))
        .join("testing")
        .join("artifacts");

    let collector = crate::testing::artifacts::ArtifactCollector::new(artifacts_dir);
    let manifest = collector.create_bundle(&run_id)?;

    serde_json::to_value(manifest).map_err(|e| format!("Failed to serialize manifest: {}", e))
}
