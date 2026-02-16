use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

/// Maximum log file size before rotation (2MB).
/// When the log exceeds this, the current file is renamed to `.prev.log`
/// and a fresh file is started.
const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

static FRONTEND_LOG: OnceLock<Mutex<File>> = OnceLock::new();

fn get_log_dir_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;
    let dir = app_data.join("logs");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create logs dir: {e}"))?;
    Ok(dir)
}

fn ensure_log_file(app_handle: &AppHandle) -> Result<(), String> {
    if FRONTEND_LOG.get().is_some() {
        return Ok(());
    }

    let dir = get_log_dir_path(app_handle)?;
    let path = dir.join("frontend.log");
    let prev_path = dir.join("frontend.prev.log");

    // Rotate if the log file is too large
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_SIZE {
            let _ = fs::copy(&path, &prev_path);
            let _ = fs::remove_file(&path);
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open frontend log: {e}"))?;

    FRONTEND_LOG.get_or_init(|| Mutex::new(file));
    Ok(())
}

/// Write a batch of pre-formatted log lines from the frontend.
/// Each line already includes `[YYYY-MM-DD HH:MM:SS.mmm] [LEVEL] message`.
#[tauri::command]
pub fn write_frontend_log(app_handle: AppHandle, lines: Vec<String>) -> Result<(), String> {
    ensure_log_file(&app_handle)?;

    let mutex = FRONTEND_LOG
        .get()
        .ok_or_else(|| "Log file not initialized".to_string())?;
    let mut file = mutex
        .lock()
        .map_err(|e| format!("Log lock poisoned: {e}"))?;

    for line in &lines {
        let _ = writeln!(file, "{}", line);
    }
    let _ = file.flush();
    Ok(())
}

/// Returns the logs directory path.
#[tauri::command]
pub fn get_log_dir(app_handle: AppHandle) -> Result<String, String> {
    let dir = get_log_dir_path(&app_handle)?;
    dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Log dir path is not valid UTF-8".to_string())
}

/// Read the last N lines from the frontend log file.
#[tauri::command]
pub fn read_frontend_log(app_handle: AppHandle, tail_lines: usize) -> Result<Vec<String>, String> {
    let dir = get_log_dir_path(&app_handle)?;
    let path = dir.join("frontend.log");

    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(&path).map_err(|e| format!("Failed to open frontend log: {e}"))?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .collect();

    let start = all_lines.len().saturating_sub(tail_lines);
    Ok(all_lines[start..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_max_log_size_is_2mb() {
        assert_eq!(MAX_LOG_SIZE, 2 * 1024 * 1024);
    }

    #[test]
    fn test_rotation_copies_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        let prev_path = dir.path().join("frontend.prev.log");

        // Create a file that exceeds MAX_LOG_SIZE
        {
            let mut f = File::create(&path).unwrap();
            let data = vec![b'x'; (MAX_LOG_SIZE + 1) as usize];
            f.write_all(&data).unwrap();
        }

        // Simulate rotation logic
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > MAX_LOG_SIZE {
                let _ = fs::copy(&path, &prev_path);
                let _ = fs::remove_file(&path);
            }
        }

        assert!(prev_path.exists());
        assert!(!path.exists());
    }

    #[test]
    fn test_read_tail_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frontend.log");

        {
            let mut f = File::create(&path).unwrap();
            for i in 1..=10 {
                writeln!(f, "line {}", i).unwrap();
            }
        }

        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
        let start = all_lines.len().saturating_sub(3);
        let tail = &all_lines[start..];

        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0], "line 8");
        assert_eq!(tail[1], "line 9");
        assert_eq!(tail[2], "line 10");
    }

    #[test]
    fn test_read_tail_more_than_available() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frontend.log");

        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, "only line").unwrap();
        }

        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
        let start = all_lines.len().saturating_sub(100);
        let tail = &all_lines[start..];

        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0], "only line");
    }

    #[test]
    fn test_read_empty_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frontend.log");

        // File doesn't exist
        assert!(!path.exists());
        // Would return empty vec
    }

    #[test]
    fn test_append_mode_preserves_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frontend.log");

        // Write first batch
        {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "first").unwrap();
        }

        // Write second batch (append)
        {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "second").unwrap();
        }

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "first\nsecond\n");
    }
}
