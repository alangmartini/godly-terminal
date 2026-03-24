use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

const SESSIONS_FILE_NAME: &str = "quick-claude-sessions.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickClaudeSessionRecord {
    pub id: String,
    pub prompt: String,
    pub terminal_id: String,
    pub workspace_id: String,
    pub branch: String,
    pub model: String,
    pub mode: String,
    pub status: SessionStatus,
    pub launched_at: String,     // ISO 8601 format
    pub claude_session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub is_clone: bool,
}

pub fn sessions_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let directory_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    base.join(directory_name).join(SESSIONS_FILE_NAME)
}

pub fn load_sessions() -> Vec<QuickClaudeSessionRecord> {
    load_sessions_from_path(&sessions_path())
}

pub fn load_sessions_from_path(path: &Path) -> Vec<QuickClaudeSessionRecord> {
    match std::fs::read_to_string(path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_sessions(sessions: &[QuickClaudeSessionRecord]) -> Result<(), String> {
    save_sessions_to_path(&sessions_path(), sessions)
}

pub fn save_sessions_to_path(path: &Path, sessions: &[QuickClaudeSessionRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create directory {}: {}", parent.display(), error))?;
    }
    let json = serde_json::to_string_pretty(sessions)
        .map_err(|error| format!("Failed to serialize sessions: {}", error))?;
    std::fs::write(path, json)
        .map_err(|error| format!("Failed to write sessions file: {}", error))?;
    Ok(())
}

pub fn add_session(record: QuickClaudeSessionRecord) -> Result<(), String> {
    let mut sessions = load_sessions();
    sessions.push(record);
    save_sessions(&sessions)
}

pub fn update_session_status(session_id: &str, status: SessionStatus) -> Result<(), String> {
    let mut sessions = load_sessions();
    if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
        session.status = status;
    }
    save_sessions(&sessions)
}

/// Remove sessions whose terminal_id is not in the set of live terminal IDs.
pub fn cleanup_stale_sessions(live_terminal_ids: &[String]) -> Result<Vec<QuickClaudeSessionRecord>, String> {
    let mut sessions = load_sessions();
    for session in sessions.iter_mut() {
        if session.status == SessionStatus::Running
            && !live_terminal_ids.contains(&session.terminal_id)
        {
            session.status = SessionStatus::Completed;
        }
    }
    // Keep only the 50 most recent sessions
    if sessions.len() > 50 {
        sessions.drain(0..sessions.len() - 50);
    }
    save_sessions(&sessions)?;
    Ok(sessions)
}

// Generate an ISO 8601 timestamp using std::time (no chrono dependency)
pub fn now_iso8601() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Simple UTC timestamp: YYYY-MM-DDTHH:MM:SSZ
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Calculate year/month/day from days since epoch (1970-01-01)
    let (year, month, day) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

fn days_to_ymd(days_since_epoch: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(id: &str, terminal_id: &str, status: SessionStatus) -> QuickClaudeSessionRecord {
        QuickClaudeSessionRecord {
            id: id.to_string(),
            prompt: "test prompt".to_string(),
            terminal_id: terminal_id.to_string(),
            workspace_id: "ws-1".to_string(),
            branch: "main".to_string(),
            model: "opus".to_string(),
            mode: "code".to_string(),
            status,
            launched_at: "2026-03-20T12:00:00Z".to_string(),
            claude_session_id: None,
            cwd: None,
            is_clone: false,
        }
    }

    fn temp_sessions_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "quick-claude-sessions-test-{}.json",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn test_session_record_serialization() {
        let record = make_record("s-1", "t-1", SessionStatus::Running);
        let json = serde_json::to_string(&record).expect("serialize should succeed");
        let decoded: QuickClaudeSessionRecord =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(decoded.id, record.id);
        assert_eq!(decoded.prompt, record.prompt);
        assert_eq!(decoded.terminal_id, record.terminal_id);
        assert_eq!(decoded.status, record.status);
    }

    #[test]
    fn test_load_missing_file() {
        let path = std::env::temp_dir().join("nonexistent-quick-claude-sessions.json");
        let sessions = load_sessions_from_path(&path);
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let path = temp_sessions_path();
        let records = vec![
            make_record("s-1", "t-1", SessionStatus::Running),
            make_record("s-2", "t-2", SessionStatus::Completed),
        ];

        save_sessions_to_path(&path, &records).expect("save should succeed");
        let loaded = load_sessions_from_path(&path);

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "s-1");
        assert_eq!(loaded[1].id, "s-2");
        assert_eq!(loaded[0].status, SessionStatus::Running);
        assert_eq!(loaded[1].status, SessionStatus::Completed);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_add_session() {
        let path = temp_sessions_path();
        // Start with one session
        let initial = vec![make_record("s-1", "t-1", SessionStatus::Completed)];
        save_sessions_to_path(&path, &initial).expect("save should succeed");

        // Simulate add_session by loading from path, pushing, saving
        let mut sessions = load_sessions_from_path(&path);
        sessions.push(make_record("s-2", "t-2", SessionStatus::Running));
        save_sessions_to_path(&path, &sessions).expect("save should succeed");

        let loaded = load_sessions_from_path(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[1].id, "s-2");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_cleanup_stale() {
        let path = temp_sessions_path();
        let records = vec![
            make_record("s-1", "t-1", SessionStatus::Running),
            make_record("s-2", "t-2", SessionStatus::Running),
            make_record("s-3", "t-3", SessionStatus::Completed),
        ];
        save_sessions_to_path(&path, &records).expect("save should succeed");

        // Only t-1 is still live
        let mut sessions = load_sessions_from_path(&path);
        let live = vec!["t-1".to_string()];
        for session in sessions.iter_mut() {
            if session.status == SessionStatus::Running
                && !live.contains(&session.terminal_id)
            {
                session.status = SessionStatus::Completed;
            }
        }
        save_sessions_to_path(&path, &sessions).expect("save should succeed");

        let loaded = load_sessions_from_path(&path);
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].status, SessionStatus::Running);   // t-1 still live
        assert_eq!(loaded[1].status, SessionStatus::Completed);  // t-2 was stale
        assert_eq!(loaded[2].status, SessionStatus::Completed);  // t-3 already completed

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_cleanup_keeps_recent() {
        let path = temp_sessions_path();
        // Create 60 sessions
        let records: Vec<QuickClaudeSessionRecord> = (0..60)
            .map(|i| make_record(&format!("s-{}", i), &format!("t-{}", i), SessionStatus::Completed))
            .collect();
        save_sessions_to_path(&path, &records).expect("save should succeed");

        let mut sessions = load_sessions_from_path(&path);
        if sessions.len() > 50 {
            sessions.drain(0..sessions.len() - 50);
        }
        save_sessions_to_path(&path, &sessions).expect("save should succeed");

        let loaded = load_sessions_from_path(&path);
        assert_eq!(loaded.len(), 50);
        // Should keep the 50 most recent (indices 10..60)
        assert_eq!(loaded[0].id, "s-10");
        assert_eq!(loaded[49].id, "s-59");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_now_iso8601_format() {
        let timestamp = now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SSZ pattern
        assert_eq!(timestamp.len(), 20, "ISO 8601 timestamp should be 20 chars: {}", timestamp);
        assert!(timestamp.ends_with('Z'), "timestamp should end with Z: {}", timestamp);
        assert_eq!(&timestamp[4..5], "-", "char 4 should be dash: {}", timestamp);
        assert_eq!(&timestamp[7..8], "-", "char 7 should be dash: {}", timestamp);
        assert_eq!(&timestamp[10..11], "T", "char 10 should be T: {}", timestamp);
        assert_eq!(&timestamp[13..14], ":", "char 13 should be colon: {}", timestamp);
        assert_eq!(&timestamp[16..17], ":", "char 16 should be colon: {}", timestamp);
    }

    #[test]
    fn test_session_record_cwd_roundtrip() {
        let mut record = make_record("s-cwd", "t-cwd", SessionStatus::Running);
        record.cwd = Some("/worktree/path".to_string());
        let json = serde_json::to_string(&record).expect("serialize");
        let decoded: QuickClaudeSessionRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.cwd, Some("/worktree/path".to_string()));
    }

    #[test]
    fn test_old_records_without_cwd_deserialize() {
        let json = r#"{"id":"s-1","prompt":"test","terminal_id":"t-1","workspace_id":"ws-1","branch":"main","model":"opus","mode":"code","status":"Running","launched_at":"2026-03-20T12:00:00Z","claude_session_id":null}"#;
        let decoded: QuickClaudeSessionRecord = serde_json::from_str(json).expect("old records should deserialize");
        assert_eq!(decoded.cwd, None);
    }

    #[test]
    fn test_old_records_without_is_clone_deserialize() {
        let json = r#"{"id":"s-1","prompt":"test","terminal_id":"t-1","workspace_id":"ws-1","branch":"main","model":"opus","mode":"code","status":"Running","launched_at":"2026-03-20T12:00:00Z","claude_session_id":null}"#;
        let decoded: QuickClaudeSessionRecord = serde_json::from_str(json).expect("old records should deserialize");
        assert!(!decoded.is_clone);
    }

    #[test]
    fn test_is_clone_roundtrip() {
        let mut record = make_record("s-clone", "t-clone", SessionStatus::Running);
        record.is_clone = true;
        let json = serde_json::to_string(&record).expect("serialize");
        let decoded: QuickClaudeSessionRecord = serde_json::from_str(&json).expect("deserialize");
        assert!(decoded.is_clone);
    }

    #[test]
    fn test_sessions_path_not_empty() {
        let path = sessions_path();
        assert!(!path.as_os_str().is_empty(), "sessions path should not be empty");
        assert!(
            path.to_string_lossy().contains("quick-claude-sessions.json"),
            "path should contain the sessions file name: {:?}",
            path
        );
    }
}
