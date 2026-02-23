use std::path::PathBuf;

#[derive(serde::Serialize, Clone)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub source: String, // "project" or "global"
}

/// Scan a skills directory and return SkillInfo for each .md file found.
fn scan_skills_dir(dir: &PathBuf, source: &str) -> Vec<SkillInfo> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().take(20).collect();

        let description = lines
            .get(2)
            .map(|l| l.trim().to_string())
            .unwrap_or_default();

        let mut usage = String::new();
        let mut in_usage_section = false;
        let mut in_code_block = false;
        for line in &lines {
            if line.starts_with("## Usage") {
                in_usage_section = true;
                continue;
            }
            if in_usage_section {
                if line.starts_with("```") {
                    if in_code_block {
                        break;
                    }
                    in_code_block = true;
                    continue;
                }
                if in_code_block {
                    usage = line.trim().to_string();
                }
            }
        }

        skills.push(SkillInfo {
            name,
            description,
            usage,
            source: source.to_string(),
        });
    }
    skills
}

#[tauri::command]
pub fn list_skills(project_path: String) -> Vec<SkillInfo> {
    let mut skills_map = std::collections::HashMap::new();

    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let global_dir = PathBuf::from(home).join(".claude").join("skills");
        for skill in scan_skills_dir(&global_dir, "global") {
            skills_map.insert(skill.name.clone(), skill);
        }
    }

    let project_dir = PathBuf::from(&project_path)
        .join(".claude")
        .join("skills");
    for skill in scan_skills_dir(&project_dir, "project") {
        skills_map.insert(skill.name.clone(), skill);
    }

    let mut result: Vec<SkillInfo> = skills_map.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {e}"))
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    let path = PathBuf::from(&path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {e}"))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("Failed to write file: {e}"))
}

#[tauri::command]
pub fn write_remote_config(config: serde_json::Value) -> Result<(), String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "APPDATA not set".to_string())?;
    let dir = PathBuf::from(appdata).join("com.godly.terminal");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create config directory: {e}"))?;
    let path = dir.join("remote-config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write remote config: {e}"))
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
