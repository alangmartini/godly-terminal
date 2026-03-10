use std::path::PathBuf;

use godly_protocol::testing::ArtifactManifest;

/// Collects test artifacts (screenshots, state dumps, logs) into per-run bundles.
pub struct ArtifactCollector {
    base_dir: PathBuf,
}

impl ArtifactCollector {
    /// Create a new artifact collector rooted at the given base directory.
    /// Artifacts for each run are stored under `<base_dir>/<run_id>/`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a new artifact bundle directory for the given run.
    /// Writes stub `manifest.json` and `result.json` files.
    pub fn create_bundle(&self, run_id: &str) -> Result<ArtifactManifest, String> {
        let run_dir = self.base_dir.join(run_id);
        std::fs::create_dir_all(&run_dir)
            .map_err(|e| format!("Failed to create artifact dir: {}", e))?;

        let now = chrono_like_timestamp();

        let manifest = ArtifactManifest {
            run_id: run_id.to_string(),
            created_at: now.clone(),
            artifact_dir: run_dir.to_string_lossy().to_string(),
            files: vec!["manifest.json".to_string(), "result.json".to_string()],
        };

        // Write manifest stub
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        std::fs::write(run_dir.join("manifest.json"), &manifest_json)
            .map_err(|e| format!("Failed to write manifest.json: {}", e))?;

        // Write result stub
        let result_stub = serde_json::json!({
            "run_id": run_id,
            "created_at": now,
            "status": "in_progress",
            "tests": []
        });
        let result_json = serde_json::to_string_pretty(&result_stub)
            .map_err(|e| format!("Failed to serialize result: {}", e))?;
        std::fs::write(run_dir.join("result.json"), &result_json)
            .map_err(|e| format!("Failed to write result.json: {}", e))?;

        Ok(manifest)
    }

    /// Add a file to an existing artifact bundle.
    pub fn add_file(&self, run_id: &str, filename: &str, content: &[u8]) -> Result<(), String> {
        let run_dir = self.base_dir.join(run_id);
        if !run_dir.exists() {
            return Err(format!("Artifact bundle {} does not exist", run_id));
        }

        std::fs::write(run_dir.join(filename), content)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))?;

        Ok(())
    }

    /// Finalize an artifact bundle by updating the manifest with all files.
    pub fn finalize(&self, run_id: &str) -> Result<ArtifactManifest, String> {
        let run_dir = self.base_dir.join(run_id);
        if !run_dir.exists() {
            return Err(format!("Artifact bundle {} does not exist", run_id));
        }

        // Collect all files in the bundle
        let files: Vec<String> = std::fs::read_dir(&run_dir)
            .map_err(|e| format!("Failed to read artifact dir: {}", e))?
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    e.file_name().to_str().map(String::from)
                })
            })
            .collect();

        let manifest = ArtifactManifest {
            run_id: run_id.to_string(),
            created_at: chrono_like_timestamp(),
            artifact_dir: run_dir.to_string_lossy().to_string(),
            files,
        };

        // Update manifest.json
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
        std::fs::write(run_dir.join("manifest.json"), &manifest_json)
            .map_err(|e| format!("Failed to write manifest.json: {}", e))?;

        Ok(manifest)
    }
}

/// Simple ISO 8601-ish timestamp without pulling in chrono.
fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Return epoch seconds as a string — good enough for test artifacts
    format!("{}", secs)
}
