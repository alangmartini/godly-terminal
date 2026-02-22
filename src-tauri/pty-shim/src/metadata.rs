use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShimMetadata {
    pub session_id: String,
    pub shim_pid: u32,
    pub shim_pipe_name: String,
    pub shell_pid: u32,
    pub shell_type: String,
    pub cwd: Option<String>,
    pub rows: u16,
    pub cols: u16,
    pub created_at: u64,
}

/// Returns the directory where shim metadata files are stored.
/// On Windows: `%APPDATA%/com.godly.terminal/shims/`
pub fn shim_metadata_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    PathBuf::from(base)
        .join("com.godly.terminal")
        .join("shims")
}

/// Write metadata for a shim session to disk.
pub fn write_metadata(meta: &ShimMetadata) -> std::io::Result<()> {
    let dir = shim_metadata_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", meta.session_id));
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)
}

/// Read metadata for a specific session.
pub fn read_metadata(session_id: &str) -> std::io::Result<ShimMetadata> {
    let path = shim_metadata_dir().join(format!("{}.json", session_id));
    let json = fs::read_to_string(path)?;
    serde_json::from_str(&json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Remove the metadata file for a session.
pub fn remove_metadata(session_id: &str) -> std::io::Result<()> {
    let path = shim_metadata_dir().join(format!("{}.json", session_id));
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// List all shim metadata files, returning all successfully parsed entries.
pub fn list_metadata() -> std::io::Result<Vec<ShimMetadata>> {
    let dir = shim_metadata_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut results = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match fs::read_to_string(&path) {
                Ok(json) => {
                    if let Ok(meta) = serde_json::from_str::<ShimMetadata>(&json) {
                        results.push(meta);
                    }
                }
                Err(_) => continue,
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn test_metadata() -> ShimMetadata {
        ShimMetadata {
            session_id: format!("test-{}", std::process::id()),
            shim_pid: 1234,
            shim_pipe_name: r"\\.\pipe\godly-shim-test-123".to_string(),
            shell_pid: 5678,
            shell_type: "windows".to_string(),
            cwd: Some("C:\\Users\\test".to_string()),
            rows: 24,
            cols: 80,
            created_at: 1700000000,
        }
    }

    #[test]
    fn test_metadata_serialization_roundtrip() {
        let meta = test_metadata();
        let json = serde_json::to_string_pretty(&meta).unwrap();
        let parsed: ShimMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn test_metadata_serialization_with_none_cwd() {
        let mut meta = test_metadata();
        meta.cwd = None;
        let json = serde_json::to_string_pretty(&meta).unwrap();
        let parsed: ShimMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cwd, None);
    }

    #[test]
    fn test_metadata_json_fields() {
        let meta = test_metadata();
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"session_id\""));
        assert!(json.contains("\"shim_pid\""));
        assert!(json.contains("\"shim_pipe_name\""));
        assert!(json.contains("\"shell_pid\""));
        assert!(json.contains("\"shell_type\""));
        assert!(json.contains("\"rows\""));
        assert!(json.contains("\"cols\""));
        assert!(json.contains("\"created_at\""));
    }

    #[test]
    fn test_write_read_remove_metadata() {
        // Use a unique session ID to avoid collisions with concurrent tests
        let session_id = format!("unit-test-{}-{}", std::process::id(), line!());
        let mut meta = test_metadata();
        meta.session_id = session_id.clone();

        // Write
        write_metadata(&meta).unwrap();

        // Read back
        let read_back = read_metadata(&session_id).unwrap();
        assert_eq!(read_back, meta);

        // Remove
        remove_metadata(&session_id).unwrap();

        // Should not be readable anymore
        assert!(read_metadata(&session_id).is_err());
    }

    #[test]
    fn test_remove_nonexistent_metadata_is_ok() {
        let result = remove_metadata("nonexistent-session-id-12345");
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_nonexistent_metadata_errors() {
        let result = read_metadata("nonexistent-session-id-67890");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_metadata_includes_written() {
        let session_id = format!("unit-test-list-{}-{}", std::process::id(), line!());
        let mut meta = test_metadata();
        meta.session_id = session_id.clone();

        write_metadata(&meta).unwrap();

        let all = list_metadata().unwrap();
        assert!(all.iter().any(|m| m.session_id == session_id));

        // Cleanup
        remove_metadata(&session_id).unwrap();
    }

    #[test]
    fn test_shim_metadata_dir_uses_appdata() {
        if env::var("APPDATA").is_ok() {
            let dir = shim_metadata_dir();
            assert!(dir.to_string_lossy().contains("com.godly.terminal"));
            assert!(dir.to_string_lossy().contains("shims"));
        }
    }
}
