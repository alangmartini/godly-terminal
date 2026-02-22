//! Shim metadata file management for daemon startup recovery.
//!
//! When the daemon starts, it scans for metadata files left behind by
//! surviving pty-shim processes. Each file describes a shim that may
//! still be alive and holding a terminal session. The daemon reconnects
//! to live shims and cleans up stale metadata.

use std::fs;

use godly_protocol::{shim_metadata_dir, ShimMetadata};

use crate::debug_log::daemon_log;
use crate::shim_client::is_process_alive;

/// Write shim metadata to disk.
pub fn write_metadata(meta: &ShimMetadata) -> std::io::Result<()> {
    let dir = shim_metadata_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", meta.session_id));
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

/// Remove shim metadata from disk.
pub fn remove_metadata(session_id: &str) {
    let path = shim_metadata_dir().join(format!("{}.json", session_id));
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
}

/// Discover surviving shim processes by scanning metadata files.
/// Returns metadata for shims whose processes are still alive.
/// Removes stale metadata files for dead shims.
pub fn discover_surviving_shims() -> Vec<ShimMetadata> {
    let dir = shim_metadata_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            daemon_log!("Failed to read shim metadata dir: {}", e);
            return Vec::new();
        }
    };

    let mut survivors = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let json = match fs::read_to_string(&path) {
            Ok(j) => j,
            Err(_) => continue,
        };

        let meta: ShimMetadata = match serde_json::from_str(&json) {
            Ok(m) => m,
            Err(e) => {
                daemon_log!("Invalid shim metadata in {:?}: {}", path, e);
                let _ = fs::remove_file(&path);
                continue;
            }
        };

        if is_process_alive(meta.shim_pid) {
            daemon_log!(
                "Found surviving shim: session={}, pid={}",
                meta.session_id,
                meta.shim_pid
            );
            survivors.push(meta);
        } else {
            daemon_log!(
                "Cleaning stale shim metadata: session={}, pid={}",
                meta.session_id,
                meta.shim_pid
            );
            let _ = fs::remove_file(&path);
        }
    }

    daemon_log!("Discovered {} surviving shim(s)", survivors.len());
    survivors
}
