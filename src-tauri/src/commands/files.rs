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
        let claude_dir = PathBuf::from(home).join(".claude");
        for skill in scan_skills_dir(&claude_dir.join("skills"), "global") {
            skills_map.insert(skill.name.clone(), skill);
        }
        for skill in scan_skills_dir(&claude_dir.join("commands"), "global") {
            skills_map.insert(skill.name.clone(), skill);
        }
    }

    let project_claude_dir = PathBuf::from(&project_path).join(".claude");
    for skill in scan_skills_dir(&project_claude_dir.join("skills"), "project") {
        skills_map.insert(skill.name.clone(), skill);
    }
    for skill in scan_skills_dir(&project_claude_dir.join("commands"), "project") {
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
pub fn save_clipboard_image(image_data: Vec<u8>, extension: String) -> Result<String, String> {
    use std::time::SystemTime;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get timestamp: {e}"))?
        .as_millis();

    // Sanitize extension to prevent path traversal
    let ext = extension
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();
    let ext = if ext.is_empty() { "png".to_string() } else { ext };

    let filename = format!("godly-clipboard-{timestamp}.{ext}");
    let path = std::env::temp_dir().join(filename);

    std::fs::write(&path, &image_data)
        .map_err(|e| format!("Failed to write clipboard image: {e}"))?;

    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path encoding".to_string())
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".next",
    "dist",
    "build",
    "__pycache__",
    ".tox",
    ".venv",
    "venv",
    ".cache",
];

#[tauri::command]
pub fn list_directory(path: String) -> Vec<DirEntry> {
    let dir = PathBuf::from(&path);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut results: Vec<DirEntry> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if is_dir && EXCLUDED_DIRS.contains(&name.as_str()) {
                return None;
            }
            Some(DirEntry { name, is_dir })
        })
        .collect();

    // Sort: directories first, then files, each group alphabetical (case-insensitive)
    results.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    results
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a minimal skill .md file in the given directory.
    fn write_skill_file(dir: &std::path::Path, name: &str, description: &str) {
        fs::create_dir_all(dir).unwrap();
        let content = format!(
            "# {name}\n\n{description}\n\n## Usage\n\n```\n/{name} <args>\n```\n"
        );
        fs::write(dir.join(format!("{name}.md")), content).unwrap();
    }

    #[test]
    fn list_skills_returns_skills_from_skills_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();

        write_skill_file(
            &project.join(".claude").join("skills"),
            "deploy",
            "Deploy to production",
        );

        let results = list_skills(project.to_string_lossy().to_string());
        let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"deploy"),
            "Expected 'deploy' from .claude/skills/ but got: {:?}",
            names
        );
    }

    #[test]
    fn list_skills_returns_commands_from_commands_dir() {
        // Bug #292: skills in .claude/commands/ must also be scanned
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();

        write_skill_file(
            &project.join(".claude").join("commands"),
            "create-reproducible-issue",
            "Create a test that reproduces a bug",
        );

        let results = list_skills(project.to_string_lossy().to_string());
        let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"create-reproducible-issue"),
            "Bug #292: Expected 'create-reproducible-issue' from .claude/commands/ but got: {:?}",
            names
        );
    }

    #[test]
    fn list_skills_merges_skills_and_commands_dirs() {
        // Bug #292: both .claude/skills/ and .claude/commands/ should be scanned
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();

        write_skill_file(
            &project.join(".claude").join("skills"),
            "build",
            "Build the project",
        );
        write_skill_file(
            &project.join(".claude").join("commands"),
            "release",
            "Create a release",
        );

        let results = list_skills(project.to_string_lossy().to_string());
        let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"build"),
            "Expected 'build' from .claude/skills/ but got: {:?}",
            names
        );
        assert!(
            names.contains(&"release"),
            "Bug #292: Expected 'release' from .claude/commands/ but got: {:?}",
            names
        );
    }

    #[test]
    fn list_skills_commands_dir_has_project_source() {
        // Bug #292: project .claude/commands/ entries should have source="project"
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();

        write_skill_file(
            &project.join(".claude").join("commands"),
            "test-hygiene",
            "Project-specific test hygiene",
        );

        let results = list_skills(project.to_string_lossy().to_string());
        let found = results.iter().find(|s| s.name == "test-hygiene");

        assert!(
            found.is_some(),
            "Bug #292: Expected 'test-hygiene' from .claude/commands/ but not found"
        );
        assert_eq!(
            found.unwrap().source,
            "project",
            "Project-level command should have source='project'"
        );
    }

    #[test]
    fn list_skills_global_commands_dir_is_scanned() {
        // Bug #292: global ~/.claude/commands/ should be scanned
        let tmp = tempfile::tempdir().unwrap();
        let fake_home = tmp.path().join("fakehome");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        write_skill_file(
            &fake_home.join(".claude").join("commands"),
            "global-command",
            "A global command from commands dir",
        );
        write_skill_file(
            &fake_home.join(".claude").join("skills"),
            "global-skill",
            "A global skill from skills dir",
        );

        // Temporarily override USERPROFILE
        let old_profile = std::env::var("USERPROFILE").ok();
        let old_home = std::env::var("HOME").ok();
        unsafe {
            std::env::set_var("USERPROFILE", fake_home.to_string_lossy().to_string());
            std::env::remove_var("HOME");
        }

        let results = list_skills(project.to_string_lossy().to_string());

        // Restore env
        unsafe {
            match old_profile {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
            match old_home {
                Some(v) => std::env::set_var("HOME", v),
                None => {}
            }
        }

        let names: Vec<&str> = results.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"global-skill"),
            "Expected 'global-skill' from ~/.claude/skills/ but got: {:?}",
            names
        );
        assert!(
            names.contains(&"global-command"),
            "Bug #292: Expected 'global-command' from ~/.claude/commands/ but got: {:?}",
            names
        );
    }

    // -- list_directory tests --

    #[test]
    fn list_directory_sorts_dirs_first_then_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::write(root.join("app.ts"), "").unwrap();

        let entries = list_directory(root.to_string_lossy().to_string());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        // Dirs first (alphabetical), then files (alphabetical)
        assert_eq!(names, vec!["docs", "src", "app.ts", "README.md"]);
        assert!(entries[0].is_dir);
        assert!(entries[1].is_dir);
        assert!(!entries[2].is_dir);
        assert!(!entries[3].is_dir);
    }

    #[test]
    fn list_directory_excludes_noise_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("index.ts"), "").unwrap();

        let entries = list_directory(root.to_string_lossy().to_string());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"src"));
        assert!(names.contains(&"index.ts"));
        assert!(!names.contains(&"node_modules"));
        assert!(!names.contains(&".git"));
        assert!(!names.contains(&"target"));
    }

    #[test]
    fn list_directory_returns_empty_for_nonexistent_path() {
        let entries = list_directory("/nonexistent/path/that/does/not/exist".to_string());
        assert!(entries.is_empty());
    }

    #[test]
    fn list_directory_reads_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let sub = root.join("src").join("components");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("App.ts"), "").unwrap();
        fs::write(sub.join("Header.ts"), "").unwrap();
        fs::create_dir(sub.join("utils")).unwrap();

        let entries = list_directory(sub.to_string_lossy().to_string());
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        assert_eq!(names, vec!["utils", "App.ts", "Header.ts"]);
    }
}
