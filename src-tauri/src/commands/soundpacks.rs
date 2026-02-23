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

// Embedded default sound pack for first-run bootstrap.
// In production builds, the resource dir has these files, but in dev mode
// resource_dir() points to target/debug/ which doesn't have them.
// Embedding the full Orc Peon pack (~700KB) guarantees the default pack always exists.
const DEFAULT_MANIFEST: &str = include_str!("../../soundpacks/default/manifest.json");

const EMBEDDED_SOUNDS: &[(&str, &[u8])] = &[
    (
        "PeonAngry1.wav",
        include_bytes!("../../soundpacks/default/PeonAngry1.wav"),
    ),
    (
        "PeonAngry2.wav",
        include_bytes!("../../soundpacks/default/PeonAngry2.wav"),
    ),
    (
        "PeonAngry3.wav",
        include_bytes!("../../soundpacks/default/PeonAngry3.wav"),
    ),
    (
        "PeonAngry4.wav",
        include_bytes!("../../soundpacks/default/PeonAngry4.wav"),
    ),
    (
        "PeonDeath.wav",
        include_bytes!("../../soundpacks/default/PeonDeath.wav"),
    ),
    (
        "PeonReady1.wav",
        include_bytes!("../../soundpacks/default/PeonReady1.wav"),
    ),
    (
        "PeonWarcry1.wav",
        include_bytes!("../../soundpacks/default/PeonWarcry1.wav"),
    ),
    (
        "PeonWhat1.wav",
        include_bytes!("../../soundpacks/default/PeonWhat1.wav"),
    ),
    (
        "PeonWhat2.wav",
        include_bytes!("../../soundpacks/default/PeonWhat2.wav"),
    ),
    (
        "PeonWhat3.wav",
        include_bytes!("../../soundpacks/default/PeonWhat3.wav"),
    ),
    (
        "PeonWhat4.wav",
        include_bytes!("../../soundpacks/default/PeonWhat4.wav"),
    ),
    (
        "PeonYes1.wav",
        include_bytes!("../../soundpacks/default/PeonYes1.wav"),
    ),
    (
        "PeonYes2.wav",
        include_bytes!("../../soundpacks/default/PeonYes2.wav"),
    ),
    (
        "PeonYes3.wav",
        include_bytes!("../../soundpacks/default/PeonYes3.wav"),
    ),
    (
        "PeonYes4.wav",
        include_bytes!("../../soundpacks/default/PeonYes4.wav"),
    ),
    (
        "PeonYesAttack1.wav",
        include_bytes!("../../soundpacks/default/PeonYesAttack1.wav"),
    ),
    (
        "PeonYesAttack2.wav",
        include_bytes!("../../soundpacks/default/PeonYesAttack2.wav"),
    ),
    (
        "PeonYesAttack3.wav",
        include_bytes!("../../soundpacks/default/PeonYesAttack3.wav"),
    ),
];

/// Copy bundled sound packs from the resource dir into the user's soundpacks dir.
/// Preserves existing packs and their contents. Falls back to embedded defaults
/// when the resource directory is unavailable (dev mode).
pub fn install_bundled_sound_packs(app_handle: &AppHandle) {
    let target_dir = match get_soundpacks_dir_path(app_handle) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[soundpacks] Failed to get app soundpacks dir: {}", e);
            return;
        }
    };

    // Try copying from the resource directory first (production builds)
    let mut copied_from_resources = false;
    if let Ok(res) = app_handle.path().resource_dir() {
        let resource_dir = res.join(SOUNDPACKS_DIR);
        if resource_dir.exists() {
            if let Ok(entries) = fs::read_dir(&resource_dir) {
                for entry in entries.flatten() {
                    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                        continue;
                    }

                    let pack_name = entry.file_name().to_string_lossy().to_string();
                    let dest_pack = target_dir.join(&pack_name);

                    if !dest_pack.exists() {
                        if let Err(e) = fs::create_dir_all(&dest_pack) {
                            eprintln!(
                                "[soundpacks] Failed to create pack dir {}: {}",
                                pack_name, e
                            );
                            continue;
                        }
                    }

                    let pack_entries = match fs::read_dir(entry.path()) {
                        Ok(e) => e,
                        Err(_) => continue,
                    };

                    for file_entry in pack_entries.flatten() {
                        if !file_entry
                            .file_type()
                            .map(|ft| ft.is_file())
                            .unwrap_or(false)
                        {
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
                            copied_from_resources = true;
                        }
                    }
                }
            }
        }
    }

    // Ensure the default pack exists and is up-to-date using embedded data.
    // This covers dev mode and upgrades from older versions.
    let default_pack_dir = target_dir.join("default");
    let manifest_path = default_pack_dir.join("manifest.json");

    // Check if installed version is outdated
    let needs_install = if manifest_path.exists() {
        match fs::read_to_string(&manifest_path) {
            Ok(contents) => {
                match serde_json::from_str::<SoundPackManifest>(&contents) {
                    Ok(installed) => installed.version != "2.0.0", // Upgrade from old version
                    Err(_) => true,
                }
            }
            Err(_) => true,
        }
    } else {
        true
    };

    if needs_install && !copied_from_resources {
        eprintln!("[soundpacks] Installing/upgrading default pack with embedded Orc Peon sounds");

        if let Err(e) = fs::create_dir_all(&default_pack_dir) {
            eprintln!("[soundpacks] Failed to create default pack dir: {}", e);
            return;
        }

        // Write manifest
        if let Err(e) = fs::write(&manifest_path, DEFAULT_MANIFEST) {
            eprintln!("[soundpacks] Failed to write default manifest: {}", e);
        } else {
            eprintln!("[soundpacks] Installed embedded: default/manifest.json");
        }

        // Write all embedded sound files
        for (filename, data) in EMBEDDED_SOUNDS {
            let sound_path = default_pack_dir.join(filename);
            if let Err(e) = fs::write(&sound_path, data) {
                eprintln!("[soundpacks] Failed to write {}: {}", filename, e);
            }
        }
        eprintln!(
            "[soundpacks] Installed {} embedded sound files",
            EMBEDDED_SOUNDS.len()
        );

        // Clean up old work_complete.mp3 from previous version
        let old_sound = default_pack_dir.join("work_complete.mp3");
        if old_sound.exists() {
            let _ = fs::remove_file(&old_sound);
            eprintln!("[soundpacks] Removed legacy work_complete.mp3");
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

/// Install a sound pack from frontend-provided data (used by PeonPing registry download).
/// Takes a manifest JSON string and a list of (filename, base64_data) sound files.
#[tauri::command]
pub fn install_sound_pack(
    app_handle: AppHandle,
    pack_id: String,
    manifest_json: String,
    files: Vec<(String, String)>,
) -> Result<(), String> {
    validate_pack_id(&pack_id)?;

    // Verify manifest is valid JSON
    let _: serde_json::Value =
        serde_json::from_str(&manifest_json).map_err(|e| format!("Invalid manifest JSON: {}", e))?;

    let dir = get_soundpacks_dir_path(&app_handle)?;
    let pack_dir = dir.join(&pack_id);

    fs::create_dir_all(&pack_dir)
        .map_err(|e| format!("Failed to create pack directory: {}", e))?;

    // Write manifest
    fs::write(pack_dir.join("manifest.json"), &manifest_json)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

    // Write sound files
    use base64::Engine;
    for (filename, b64_data) in &files {
        validate_filename(filename)?;

        let data = base64::engine::general_purpose::STANDARD
            .decode(b64_data)
            .map_err(|e| format!("Failed to decode {}: {}", filename, e))?;

        if data.len() > MAX_SOUND_SIZE {
            return Err(format!(
                "Sound file {} too large ({}MB limit)",
                filename,
                MAX_SOUND_SIZE / 1024 / 1024
            ));
        }

        fs::write(pack_dir.join(filename), &data)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))?;
    }

    eprintln!(
        "[soundpacks] Installed pack '{}' with {} sound files",
        pack_id,
        files.len()
    );
    Ok(())
}

/// Delete an installed sound pack. Cannot delete the default pack.
#[tauri::command]
pub fn delete_sound_pack(app_handle: AppHandle, pack_id: String) -> Result<(), String> {
    validate_pack_id(&pack_id)?;

    if pack_id == "default" {
        return Err("Cannot delete the default sound pack".to_string());
    }

    let dir = get_soundpacks_dir_path(&app_handle)?;
    let pack_dir = dir.join(&pack_id);

    if !pack_dir.exists() {
        return Err(format!("Sound pack not found: {}", pack_id));
    }

    fs::remove_dir_all(&pack_dir)
        .map_err(|e| format!("Failed to delete sound pack: {}", e))?;

    eprintln!("[soundpacks] Deleted pack '{}'", pack_id);
    Ok(())
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
