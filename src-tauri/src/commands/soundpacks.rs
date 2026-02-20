use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const SOUNDPACKS_DIR: &str = "soundpacks";
const MAX_SOUND_SIZE: usize = 10 * 1024 * 1024; // 10MB limit
const ALLOWED_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a"];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SoundPackManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub sounds: HashMap<String, Vec<String>>,
}

fn get_soundpacks_dir_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let dir = app_data_dir.join(SOUNDPACKS_DIR);

    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create soundpacks dir: {}", e))?;
    }

    Ok(dir)
}

fn is_audio_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    ALLOWED_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)))
}

fn validate_pack_id(pack_id: &str) -> Result<(), String> {
    if pack_id.is_empty()
        || pack_id.contains('/')
        || pack_id.contains('\\')
        || pack_id.contains("..")
    {
        return Err("Invalid pack ID".to_string());
    }
    Ok(())
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

/// Copy bundled sound packs from the resource dir into the user's soundpacks dir.
/// Preserves existing packs and their contents.
pub fn install_bundled_sound_packs(app_handle: &AppHandle) {
    let resource_dir = match app_handle.path().resource_dir() {
        Ok(d) => d.join(SOUNDPACKS_DIR),
        Err(e) => {
            eprintln!("[soundpacks] Failed to get resource dir: {}", e);
            return;
        }
    };

    if !resource_dir.exists() {
        return;
    }

    let target_dir = match get_soundpacks_dir_path(app_handle) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[soundpacks] Failed to get app soundpacks dir: {}", e);
            return;
        }
    };

    let entries = match fs::read_dir(&resource_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[soundpacks] Failed to read bundled soundpacks: {}", e);
            return;
        }
    };

    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }

        let pack_name = entry.file_name().to_string_lossy().to_string();
        let dest_pack = target_dir.join(&pack_name);

        if !dest_pack.exists() {
            if let Err(e) = fs::create_dir_all(&dest_pack) {
                eprintln!("[soundpacks] Failed to create pack dir {}: {}", pack_name, e);
                continue;
            }
        }

        // Copy all files in the pack dir that don't already exist
        let pack_entries = match fs::read_dir(entry.path()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for file_entry in pack_entries.flatten() {
            if !file_entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }
            let file_name = file_entry.file_name().to_string_lossy().to_string();
            let dest_file = dest_pack.join(&file_name);
            if dest_file.exists() {
                continue;
            }
            if let Err(e) = fs::copy(file_entry.path(), &dest_file) {
                eprintln!(
                    "[soundpacks] Failed to copy {}/{}: {}",
                    pack_name, file_name, e
                );
            } else {
                eprintln!("[soundpacks] Installed: {}/{}", pack_name, file_name);
            }
        }
    }
}

#[tauri::command]
pub fn list_sound_packs(app_handle: AppHandle) -> Result<Vec<SoundPackManifest>, String> {
    let dir = get_soundpacks_dir_path(&app_handle)?;

    let entries =
        fs::read_dir(&dir).map_err(|e| format!("Failed to read soundpacks dir: {}", e))?;

    let mut packs: Vec<SoundPackManifest> = Vec::new();

    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }

        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        match fs::read_to_string(&manifest_path) {
            Ok(contents) => match serde_json::from_str::<SoundPackManifest>(&contents) {
                Ok(manifest) => packs.push(manifest),
                Err(e) => {
                    eprintln!(
                        "[soundpacks] Failed to parse manifest in {:?}: {}",
                        entry.path(),
                        e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "[soundpacks] Failed to read manifest in {:?}: {}",
                    entry.path(),
                    e
                );
            }
        }
    }

    packs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(packs)
}

#[tauri::command]
pub fn list_sound_pack_files(
    app_handle: AppHandle,
    pack_id: String,
) -> Result<Vec<String>, String> {
    validate_pack_id(&pack_id)?;
    let dir = get_soundpacks_dir_path(&app_handle)?;
    let pack_dir = dir.join(&pack_id);

    if !pack_dir.exists() {
        return Err(format!("Sound pack not found: {}", pack_id));
    }

    let entries =
        fs::read_dir(&pack_dir).map_err(|e| format!("Failed to read pack dir: {}", e))?;

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
pub fn read_sound_pack_file(
    app_handle: AppHandle,
    pack_id: String,
    filename: String,
) -> Result<String, String> {
    validate_pack_id(&pack_id)?;
    validate_filename(&filename)?;

    let dir = get_soundpacks_dir_path(&app_handle)?;
    let file_path = dir.join(&pack_id).join(&filename);

    if !file_path.exists() {
        return Err(format!("Sound file not found: {}/{}", pack_id, filename));
    }

    let metadata =
        fs::metadata(&file_path).map_err(|e| format!("Failed to read file metadata: {}", e))?;

    if metadata.len() as usize > MAX_SOUND_SIZE {
        return Err(format!(
            "Sound file too large ({}MB limit)",
            MAX_SOUND_SIZE / 1024 / 1024
        ));
    }

    let data = fs::read(&file_path).map_err(|e| format!("Failed to read sound file: {}", e))?;

    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}

#[tauri::command]
pub fn get_sound_packs_dir(app_handle: AppHandle) -> Result<String, String> {
    let dir = get_soundpacks_dir_path(&app_handle)?;
    dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Sound packs dir path is not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_validate_pack_id_rejects_traversal() {
        assert_eq!(validate_pack_id("../evil"), Err("Invalid pack ID".to_string()));
        assert_eq!(validate_pack_id("..\\evil"), Err("Invalid pack ID".to_string()));
        assert_eq!(validate_pack_id("sub/dir"), Err("Invalid pack ID".to_string()));
        assert_eq!(validate_pack_id(""), Err("Invalid pack ID".to_string()));
    }

    #[test]
    fn test_validate_pack_id_accepts_valid() {
        assert_eq!(validate_pack_id("default"), Ok(()));
        assert_eq!(validate_pack_id("my-pack"), Ok(()));
        assert_eq!(validate_pack_id("warcraft_sounds"), Ok(()));
    }

    #[test]
    fn test_validate_filename_rejects_path_traversal() {
        assert_eq!(validate_filename("../evil.mp3"), Err("Invalid filename".to_string()));
        assert_eq!(validate_filename("sub/dir.mp3"), Err("Invalid filename".to_string()));
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
    fn test_validate_filename_accepts_valid() {
        assert_eq!(validate_filename("work-complete.mp3"), Ok(()));
        assert_eq!(validate_filename("error.wav"), Ok(()));
        assert_eq!(validate_filename("ready.ogg"), Ok(()));
    }

    #[test]
    fn test_is_audio_file() {
        assert!(is_audio_file("test.mp3"));
        assert!(is_audio_file("test.WAV"));
        assert!(is_audio_file("test.ogg"));
        assert!(!is_audio_file("test.txt"));
        assert!(!is_audio_file("test.exe"));
    }

    #[test]
    fn test_parse_manifest() {
        let json = r#"{
            "id": "default",
            "name": "Default Pack",
            "description": "Built-in sounds",
            "author": "Godly Terminal",
            "version": "1.0.0",
            "sounds": {
                "complete": ["work_complete.mp3"],
                "error": ["error.mp3"],
                "ready": ["ready.mp3"]
            }
        }"#;

        let manifest: SoundPackManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.id, "default");
        assert_eq!(manifest.sounds.get("complete").unwrap().len(), 1);
        assert_eq!(manifest.sounds.get("error").unwrap().len(), 1);
    }

    #[test]
    fn test_read_sound_pack_file_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join("test-pack");
        fs::create_dir_all(&pack_dir).unwrap();

        let file_path = pack_dir.join("sound.mp3");
        let mut f = fs::File::create(&file_path).unwrap();
        f.write_all(b"fake-mp3-data").unwrap();

        // Read and encode
        let data = fs::read(&file_path).unwrap();
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        assert_eq!(decoded, b"fake-mp3-data");
    }
}
