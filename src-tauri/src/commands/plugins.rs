use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const PLUGINS_DIR: &str = "plugins";
const MAX_PLUGIN_JS_SIZE: usize = 5 * 1024 * 1024; // 5MB limit
const MAX_PLUGIN_ICON_SIZE: usize = 1 * 1024 * 1024; // 1MB limit

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegistryEntry {
    pub id: String,
    pub repo: String,
    pub description: String,
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginRegistry {
    pub version: u32,
    pub plugins: Vec<RegistryEntry>,
}

fn get_plugins_dir_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let dir = app_data_dir.join(PLUGINS_DIR);

    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create plugins dir: {}", e))?;
    }

    Ok(dir)
}

fn validate_plugin_id(plugin_id: &str) -> Result<(), String> {
    if plugin_id.is_empty()
        || plugin_id.contains('/')
        || plugin_id.contains('\\')
        || plugin_id.contains("..")
    {
        return Err("Invalid plugin ID".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn get_plugins_dir(app_handle: AppHandle) -> Result<String, String> {
    let dir = get_plugins_dir_path(&app_handle)?;
    dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Plugins dir path is not valid UTF-8".to_string())
}

#[tauri::command]
pub fn list_installed_plugins(app_handle: AppHandle) -> Result<Vec<PluginManifest>, String> {
    let dir = get_plugins_dir_path(&app_handle)?;

    let entries =
        fs::read_dir(&dir).map_err(|e| format!("Failed to read plugins dir: {}", e))?;

    let mut plugins: Vec<PluginManifest> = Vec::new();

    for entry in entries.flatten() {
        if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            continue;
        }

        let manifest_path = entry.path().join("godly-plugin.json");
        if !manifest_path.exists() {
            continue;
        }

        match fs::read_to_string(&manifest_path) {
            Ok(contents) => match serde_json::from_str::<PluginManifest>(&contents) {
                Ok(manifest) => plugins.push(manifest),
                Err(e) => {
                    eprintln!(
                        "[plugins] Failed to parse manifest in {:?}: {}",
                        entry.path(),
                        e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "[plugins] Failed to read manifest in {:?}: {}",
                    entry.path(),
                    e
                );
            }
        }
    }

    plugins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(plugins)
}

#[tauri::command]
pub fn read_plugin_js(app_handle: AppHandle, plugin_id: String) -> Result<String, String> {
    validate_plugin_id(&plugin_id)?;

    let dir = get_plugins_dir_path(&app_handle)?;
    let js_path = dir.join(&plugin_id).join("dist").join("index.js");

    if !js_path.exists() {
        return Err(format!("Plugin JS not found: {}/dist/index.js", plugin_id));
    }

    let metadata =
        fs::metadata(&js_path).map_err(|e| format!("Failed to read file metadata: {}", e))?;

    if metadata.len() as usize > MAX_PLUGIN_JS_SIZE {
        return Err(format!(
            "Plugin JS too large ({}MB limit)",
            MAX_PLUGIN_JS_SIZE / 1024 / 1024
        ));
    }

    fs::read_to_string(&js_path).map_err(|e| format!("Failed to read plugin JS: {}", e))
}

#[tauri::command]
pub fn read_plugin_icon(app_handle: AppHandle, plugin_id: String) -> Result<Option<String>, String> {
    validate_plugin_id(&plugin_id)?;

    let dir = get_plugins_dir_path(&app_handle)?;
    let icon_path = dir.join(&plugin_id).join("icon.png");

    if !icon_path.exists() {
        return Ok(None);
    }

    let metadata =
        fs::metadata(&icon_path).map_err(|e| format!("Failed to read icon metadata: {}", e))?;

    if metadata.len() as usize > MAX_PLUGIN_ICON_SIZE {
        return Err(format!(
            "Plugin icon too large ({}MB limit)",
            MAX_PLUGIN_ICON_SIZE / 1024 / 1024
        ));
    }

    let data = fs::read(&icon_path).map_err(|e| format!("Failed to read plugin icon: {}", e))?;

    use base64::Engine;
    Ok(Some(base64::engine::general_purpose::STANDARD.encode(&data)))
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[tauri::command]
pub async fn install_plugin(
    app_handle: AppHandle,
    owner: String,
    repo: String,
) -> Result<PluginManifest, String> {
    let client = reqwest::Client::builder()
        .user_agent("godly-terminal")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Fetch latest release
    let release_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );
    let release: GitHubRelease = client
        .get(&release_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch release: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse release JSON: {}", e))?;

    // Find the first .zip asset
    let zip_asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".zip"))
        .ok_or_else(|| "No .zip asset found in latest release".to_string())?;

    // Download the zip
    let zip_bytes = client
        .get(&zip_asset.browser_download_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download plugin zip: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Failed to read plugin zip bytes: {}", e))?;

    // Extract to a temp dir
    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let reader = std::io::Cursor::new(&zip_bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("Failed to read zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath: PathBuf = match file.enclosed_name() {
            Some(path) => temp_dir.path().join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent dir: {}", e))?;
                }
            }
            let mut outfile = fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    // Find godly-plugin.json in the extracted contents
    let manifest_path = find_manifest_in_dir(temp_dir.path())
        .ok_or_else(|| "No godly-plugin.json found in zip".to_string())?;

    let manifest_contents = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let manifest: PluginManifest = serde_json::from_str(&manifest_contents)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    // The plugin files are in the same directory as the manifest
    let source_dir = manifest_path
        .parent()
        .ok_or_else(|| "Manifest has no parent directory".to_string())?;

    // Move extracted contents to plugins_dir/plugin_id/
    let plugins_dir = get_plugins_dir_path(&app_handle)?;
    let plugin_dir = plugins_dir.join(&manifest.id);

    // Remove existing plugin dir if it exists (upgrade)
    if plugin_dir.exists() {
        fs::remove_dir_all(&plugin_dir)
            .map_err(|e| format!("Failed to remove existing plugin: {}", e))?;
    }

    copy_dir_recursive(source_dir, &plugin_dir)
        .map_err(|e| format!("Failed to install plugin: {}", e))?;

    Ok(manifest)
}

/// Recursively search for godly-plugin.json in a directory
fn find_manifest_in_dir(dir: &std::path::Path) -> Option<PathBuf> {
    let manifest = dir.join("godly-plugin.json");
    if manifest.exists() {
        return Some(manifest);
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                if let Some(found) = find_manifest_in_dir(&entry.path()) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn uninstall_plugin(app_handle: AppHandle, plugin_id: String) -> Result<(), String> {
    validate_plugin_id(&plugin_id)?;

    let dir = get_plugins_dir_path(&app_handle)?;
    let plugin_dir = dir.join(&plugin_id);

    if !plugin_dir.exists() {
        return Err(format!("Plugin not found: {}", plugin_id));
    }

    fs::remove_dir_all(&plugin_dir)
        .map_err(|e| format!("Failed to uninstall plugin: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn check_plugin_update(
    owner: String,
    repo: String,
    installed_version: String,
) -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("godly-terminal")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let release_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );

    let release: GitHubRelease = client
        .get(&release_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch release: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse release JSON: {}", e))?;

    let latest = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name);

    if latest != installed_version {
        Ok(Some(latest.to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn fetch_plugin_registry(app_handle: AppHandle) -> Result<PluginRegistry, String> {
    // Try fetching from remote first
    let client = reqwest::Client::builder()
        .user_agent("godly-terminal")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let remote_url = "https://raw.githubusercontent.com/alangmartini/godly-terminal/master/src/plugins/registry.json";

    match client.get(remote_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<PluginRegistry>().await {
                    Ok(registry) => return Ok(registry),
                    Err(e) => {
                        eprintln!("[plugins] Failed to parse remote registry: {}", e);
                    }
                }
            } else {
                eprintln!(
                    "[plugins] Remote registry returned status: {}",
                    response.status()
                );
            }
        }
        Err(e) => {
            eprintln!("[plugins] Failed to fetch remote registry: {}", e);
        }
    }

    // Fallback: read bundled registry from resource dir
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource dir: {}", e))?;

    let registry_path = resource_dir.join("plugins").join("registry.json");

    if registry_path.exists() {
        let contents = fs::read_to_string(&registry_path)
            .map_err(|e| format!("Failed to read bundled registry: {}", e))?;
        let registry: PluginRegistry = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse bundled registry: {}", e))?;
        return Ok(registry);
    }

    Err("Failed to fetch plugin registry from remote and no bundled registry found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plugin_id_rejects_traversal() {
        assert_eq!(
            validate_plugin_id("../evil"),
            Err("Invalid plugin ID".to_string())
        );
        assert_eq!(
            validate_plugin_id("..\\evil"),
            Err("Invalid plugin ID".to_string())
        );
        assert_eq!(
            validate_plugin_id("sub/dir"),
            Err("Invalid plugin ID".to_string())
        );
        assert_eq!(
            validate_plugin_id(""),
            Err("Invalid plugin ID".to_string())
        );
    }

    #[test]
    fn test_validate_plugin_id_accepts_valid() {
        assert_eq!(validate_plugin_id("peon-ping"), Ok(()));
        assert_eq!(validate_plugin_id("my-plugin"), Ok(()));
        assert_eq!(validate_plugin_id("smollm2"), Ok(()));
    }

    #[test]
    fn test_parse_manifest() {
        let json = r#"{
            "id": "peon-ping",
            "name": "Peon Ping",
            "description": "Warcraft sound notifications",
            "author": "Godly Terminal",
            "version": "1.0.0",
            "icon": "icon.png",
            "main": "dist/index.js",
            "minAppVersion": "0.4.0",
            "permissions": ["notifications"],
            "tags": ["sound", "notification"]
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.id, "peon-ping");
        assert_eq!(manifest.name, "Peon Ping");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.icon, Some("icon.png".to_string()));
        assert_eq!(manifest.main, Some("dist/index.js".to_string()));
        assert_eq!(
            manifest.min_app_version,
            Some("0.4.0".to_string())
        );
        assert_eq!(manifest.permissions, vec!["notifications"]);
        assert_eq!(manifest.tags, vec!["sound", "notification"]);
    }

    #[test]
    fn test_parse_manifest_minimal() {
        let json = r#"{
            "id": "test",
            "name": "Test Plugin",
            "description": "A test",
            "author": "Test",
            "version": "0.1.0"
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.id, "test");
        assert_eq!(manifest.icon, None);
        assert_eq!(manifest.main, None);
        assert_eq!(manifest.min_app_version, None);
        assert!(manifest.permissions.is_empty());
        assert!(manifest.tags.is_empty());
    }

    #[test]
    fn test_parse_registry() {
        let json = r#"{
            "version": 1,
            "plugins": [
                {
                    "id": "peon-ping",
                    "repo": "alangmartini/godly-plugin-peon-ping",
                    "description": "Warcraft sound notifications",
                    "author": "Godly Terminal",
                    "tags": ["sound", "notification"],
                    "featured": true
                },
                {
                    "id": "smollm2",
                    "repo": "alangmartini/godly-plugin-smollm2",
                    "description": "Local AI branch naming",
                    "author": "Godly Terminal",
                    "tags": ["ai"],
                    "featured": false
                }
            ]
        }"#;

        let registry: PluginRegistry = serde_json::from_str(json).unwrap();
        assert_eq!(registry.version, 1);
        assert_eq!(registry.plugins.len(), 2);
        assert_eq!(registry.plugins[0].id, "peon-ping");
        assert!(registry.plugins[0].featured);
        assert_eq!(registry.plugins[1].id, "smollm2");
        assert!(!registry.plugins[1].featured);
    }
}
