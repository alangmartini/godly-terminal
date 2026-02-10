use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const SOUNDS_DIR: &str = "sounds";
const MAX_SOUND_SIZE: usize = 10 * 1024 * 1024; // 10MB limit
const ALLOWED_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a"];

fn get_sounds_dir_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let sounds_dir = app_data_dir.join(SOUNDS_DIR);

    if !sounds_dir.exists() {
        fs::create_dir_all(&sounds_dir)
            .map_err(|e| format!("Failed to create sounds dir: {}", e))?;
    }

    Ok(sounds_dir)
}

fn is_audio_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    ALLOWED_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)))
}

fn validate_filename(filename: &str) -> Result<(), String> {
    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.is_empty()
    {
        return Err("Invalid filename".to_string());
    }

    if !is_audio_file(filename) {
        return Err(format!("Unsupported audio format: {}", filename));
    }

    Ok(())
}

fn read_and_encode_sound(dir: &Path, filename: &str) -> Result<String, String> {
    let path = dir.join(filename);

    if !path.exists() {
        return Err(format!("Sound file not found: {}", filename));
    }

    let metadata =
        fs::metadata(&path).map_err(|e| format!("Failed to read file metadata: {}", e))?;

    if metadata.len() as usize > MAX_SOUND_SIZE {
        return Err(format!(
            "Sound file too large ({}MB limit)",
            MAX_SOUND_SIZE / 1024 / 1024
        ));
    }

    let data = fs::read(&path).map_err(|e| format!("Failed to read sound file: {}", e))?;

    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

#[tauri::command]
pub fn get_sounds_dir(app_handle: AppHandle) -> Result<String, String> {
    let dir = get_sounds_dir_path(&app_handle)?;
    dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Sounds dir path is not valid UTF-8".to_string())
}

#[tauri::command]
pub fn list_custom_sounds(app_handle: AppHandle) -> Result<Vec<String>, String> {
    let dir = get_sounds_dir_path(&app_handle)?;

    let entries = fs::read_dir(&dir).map_err(|e| format!("Failed to read sounds dir: {}", e))?;

    let mut files: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) && is_audio_file(&name) {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    files.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(files)
}

#[tauri::command]
pub fn read_sound_file(app_handle: AppHandle, filename: String) -> Result<String, String> {
    validate_filename(&filename)?;
    let dir = get_sounds_dir_path(&app_handle)?;
    read_and_encode_sound(&dir, &filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_is_audio_file_accepts_supported_formats() {
        assert!(is_audio_file("test.mp3"));
        assert!(is_audio_file("test.MP3"));
        assert!(is_audio_file("test.wav"));
        assert!(is_audio_file("test.ogg"));
        assert!(is_audio_file("test.flac"));
        assert!(is_audio_file("test.m4a"));
    }

    #[test]
    fn test_is_audio_file_rejects_unsupported() {
        assert!(!is_audio_file("test.txt"));
        assert!(!is_audio_file("test.exe"));
        assert!(!is_audio_file("mp3")); // no dot
    }

    #[test]
    fn test_validate_filename_rejects_path_traversal() {
        assert_eq!(validate_filename("../evil.mp3"), Err("Invalid filename".to_string()));
        assert_eq!(validate_filename("..\\evil.mp3"), Err("Invalid filename".to_string()));
        assert_eq!(validate_filename("sub/dir.mp3"), Err("Invalid filename".to_string()));
        assert_eq!(validate_filename("sub\\dir.mp3"), Err("Invalid filename".to_string()));
        assert_eq!(validate_filename(""), Err("Invalid filename".to_string()));
    }

    #[test]
    fn test_validate_filename_rejects_non_audio() {
        assert_eq!(
            validate_filename("virus.exe"),
            Err("Unsupported audio format: virus.exe".to_string())
        );
    }

    #[test]
    fn test_validate_filename_accepts_valid_audio() {
        assert_eq!(validate_filename("sound.mp3"), Ok(()));
        assert_eq!(validate_filename("my-sound.wav"), Ok(()));
        assert_eq!(validate_filename("Work Complete.m4a"), Ok(()));
    }

    #[test]
    fn test_read_and_encode_sound_returns_base64() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.mp3");
        let mut f = fs::File::create(&file_path).unwrap();
        f.write_all(b"fake-mp3-data").unwrap();

        let result = read_and_encode_sound(dir.path(), "test.mp3").unwrap();

        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&result)
            .unwrap();
        assert_eq!(decoded, b"fake-mp3-data");
    }

    #[test]
    fn test_read_and_encode_sound_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_and_encode_sound(dir.path(), "missing.mp3");
        assert_eq!(result, Err("Sound file not found: missing.mp3".to_string()));
    }
}
