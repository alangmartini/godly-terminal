use std::path::PathBuf;

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        // Return empty string for non-existent files (editor will start empty)
        return Ok(String::new());
    }
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {e}"))
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    let path = PathBuf::from(&path);
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {e}"))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {e}"))
}

#[tauri::command]
pub fn get_user_claude_md_path() -> Result<String, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Could not determine home directory".to_string())?;
    let path = PathBuf::from(home).join(".claude").join("CLAUDE.md");
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path encoding".to_string())
}
