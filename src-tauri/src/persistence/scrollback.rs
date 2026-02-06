use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const SCROLLBACK_DIR: &str = "scrollback";
const MAX_SCROLLBACK_SIZE: usize = 5 * 1024 * 1024; // 5MB limit per terminal

fn log_info(msg: &str) {
    eprintln!("[scrollback] {}", msg);
}

#[allow(dead_code)]
fn log_error(msg: &str) {
    eprintln!("[scrollback] ERROR: {}", msg);
}

/// Get the scrollback directory path
fn get_scrollback_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let scrollback_dir = app_data_dir.join(SCROLLBACK_DIR);

    // Create directory if it doesn't exist
    if !scrollback_dir.exists() {
        fs::create_dir_all(&scrollback_dir)
            .map_err(|e| format!("Failed to create scrollback dir: {}", e))?;
    }

    Ok(scrollback_dir)
}

/// Get the path for a terminal's scrollback file
fn get_scrollback_path(app_handle: &AppHandle, terminal_id: &str) -> Result<PathBuf, String> {
    let dir = get_scrollback_dir(app_handle)?;
    // Sanitize terminal ID to be safe for filenames
    let safe_id = terminal_id.replace(|c: char| !c.is_alphanumeric() && c != '-', "_");
    Ok(dir.join(format!("{}.dat", safe_id)))
}

/// Save scrollback data for a terminal
#[tauri::command]
pub fn save_scrollback(
    app_handle: AppHandle,
    terminal_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let path = get_scrollback_path(&app_handle, &terminal_id)?;

    // Truncate if too large (keep the most recent data)
    let data_to_save = if data.len() > MAX_SCROLLBACK_SIZE {
        log_info(&format!(
            "Truncating scrollback for {} from {} to {} bytes",
            terminal_id,
            data.len(),
            MAX_SCROLLBACK_SIZE
        ));
        data[data.len() - MAX_SCROLLBACK_SIZE..].to_vec()
    } else {
        data
    };

    let file = File::create(&path)
        .map_err(|e| format!("Failed to create scrollback file: {}", e))?;
    let mut writer = BufWriter::new(file);

    writer
        .write_all(&data_to_save)
        .map_err(|e| format!("Failed to write scrollback: {}", e))?;

    writer
        .flush()
        .map_err(|e| format!("Failed to flush scrollback: {}", e))?;

    log_info(&format!(
        "Saved {} bytes of scrollback for {}",
        data_to_save.len(),
        terminal_id
    ));

    Ok(())
}

/// Load scrollback data for a terminal
#[tauri::command]
pub fn load_scrollback(app_handle: AppHandle, terminal_id: String) -> Result<Vec<u8>, String> {
    let path = get_scrollback_path(&app_handle, &terminal_id)?;

    if !path.exists() {
        log_info(&format!("No scrollback found for {}", terminal_id));
        return Ok(Vec::new());
    }

    let file = File::open(&path).map_err(|e| format!("Failed to open scrollback file: {}", e))?;
    let mut reader = BufReader::new(file);

    let mut data = Vec::new();
    reader
        .read_to_end(&mut data)
        .map_err(|e| format!("Failed to read scrollback: {}", e))?;

    log_info(&format!(
        "Loaded {} bytes of scrollback for {}",
        data.len(),
        terminal_id
    ));

    Ok(data)
}

/// Delete scrollback data for a terminal
#[tauri::command]
pub fn delete_scrollback(app_handle: AppHandle, terminal_id: String) -> Result<(), String> {
    let path = get_scrollback_path(&app_handle, &terminal_id)?;

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete scrollback file: {}", e))?;
        log_info(&format!("Deleted scrollback for {}", terminal_id));
    }

    Ok(())
}

/// Clean up orphaned scrollback files (terminals that no longer exist)
#[allow(dead_code)]
pub fn cleanup_orphaned_scrollback(
    app_handle: &AppHandle,
    active_terminal_ids: &[String],
) -> Result<(), String> {
    let scrollback_dir = get_scrollback_dir(app_handle)?;

    let entries = fs::read_dir(&scrollback_dir)
        .map_err(|e| format!("Failed to read scrollback dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(stem) = path.file_stem() {
            let file_id = stem.to_string_lossy().to_string();
            if !active_terminal_ids.iter().any(|id| {
                let safe_id = id.replace(|c: char| !c.is_alphanumeric() && c != '-', "_");
                safe_id == file_id
            }) {
                if let Err(e) = fs::remove_file(&path) {
                    log_error(&format!("Failed to delete orphaned scrollback {:?}: {}", path, e));
                } else {
                    log_info(&format!("Cleaned up orphaned scrollback: {:?}", path));
                }
            }
        }
    }

    Ok(())
}
