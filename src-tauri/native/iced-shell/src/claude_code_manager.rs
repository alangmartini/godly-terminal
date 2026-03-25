use iced::widget::text_editor;

/// A discovered skill on the filesystem.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub is_directory: bool,
}

/// A scope section in the vertical tab sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillScope {
    User,
    Project {
        workspace_id: String,
        workspace_name: String,
        folder_path: String,
    },
}

impl SkillScope {
    pub fn label(&self) -> String {
        match self {
            SkillScope::User => "User Skills".to_string(),
            SkillScope::Project { workspace_name, .. } => workspace_name.clone(),
        }
    }

    pub fn skills_dir(&self) -> std::path::PathBuf {
        match self {
            SkillScope::User => {
                let home = std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .unwrap_or_else(|_| ".".to_string());
                std::path::PathBuf::from(home).join(".claude").join("skills")
            }
            SkillScope::Project { folder_path, .. } => {
                std::path::PathBuf::from(folder_path).join(".claude").join("skills")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Tip,
}

#[derive(Debug, Clone)]
pub struct SkillDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub struct ClaudeCodeManagerState {
    pub scopes: Vec<SkillScope>,
    pub active_scope: usize,
    pub skills: Vec<SkillEntry>,
    pub editing_skill_index: Option<usize>,
    pub editor_content: text_editor::Content,
    pub editor_dirty: bool,
    pub diagnostics: Vec<SkillDiagnostic>,
    pub new_skill_name: String,
}

impl ClaudeCodeManagerState {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            active_scope: 0,
            skills: Vec::new(),
            editing_skill_index: None,
            editor_content: text_editor::Content::with_text(""),
            editor_dirty: false,
            diagnostics: Vec::new(),
            new_skill_name: String::new(),
        }
    }

    pub fn refresh_scopes(&mut self, workspaces: &[(String, String, String)]) {
        self.scopes.clear();
        self.scopes.push(SkillScope::User);
        let mut seen_paths = std::collections::HashSet::new();
        for (ws_id, ws_name, folder_path) in workspaces {
            if seen_paths.insert(folder_path.clone()) {
                self.scopes.push(SkillScope::Project {
                    workspace_id: ws_id.clone(),
                    workspace_name: ws_name.clone(),
                    folder_path: folder_path.clone(),
                });
            }
        }
        if self.active_scope >= self.scopes.len() {
            self.active_scope = 0;
        }
    }

    pub fn discover_skills(&mut self) {
        self.skills.clear();
        self.editing_skill_index = None;
        self.diagnostics.clear();

        let Some(scope) = self.scopes.get(self.active_scope) else {
            return;
        };
        let dir = scope.skills_dir();
        if !dir.exists() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_md = path.join("SKILL.md");
                if skill_md.exists() {
                    let (name, desc) = parse_frontmatter_from_file(&skill_md);
                    self.skills.push(SkillEntry {
                        name: name.unwrap_or_else(|| {
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        }),
                        description: desc.unwrap_or_default(),
                        file_path: skill_md.to_string_lossy().to_string(),
                        is_directory: true,
                    });
                }
            } else if path.extension().map_or(false, |ext| ext == "md") {
                let (name, desc) = parse_frontmatter_from_file(&path);
                let stem = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                self.skills.push(SkillEntry {
                    name: name.unwrap_or(stem),
                    description: desc.unwrap_or_default(),
                    file_path: path.to_string_lossy().to_string(),
                    is_directory: false,
                });
            }
        }
        self.skills.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn open_skill(&mut self, index: usize) {
        if let Some(skill) = self.skills.get(index) {
            let content = std::fs::read_to_string(&skill.file_path).unwrap_or_default();
            self.editor_content = text_editor::Content::with_text(&content);
            self.editor_dirty = false;
            self.editing_skill_index = Some(index);
            self.diagnostics = analyze_skill(&content);
        }
    }

    pub fn save_current(&mut self) -> Result<(), String> {
        let idx = self.editing_skill_index.ok_or("No skill open")?;
        let skill = self.skills.get(idx).ok_or("Invalid skill index")?;
        let content = self.editor_content.text();
        std::fs::write(&skill.file_path, &content)
            .map_err(|e| format!("Failed to write {}: {}", skill.file_path, e))?;
        self.editor_dirty = false;
        self.diagnostics = analyze_skill(&content);
        Ok(())
    }

    pub fn close_editor(&mut self) {
        self.editing_skill_index = None;
        self.diagnostics.clear();
        self.editor_dirty = false;
    }

    pub fn create_skill(&mut self) -> Result<(), String> {
        let name = self.new_skill_name.trim().to_string();
        if name.is_empty() {
            return Err("Skill name cannot be empty".to_string());
        }
        let Some(scope) = self.scopes.get(self.active_scope) else {
            return Err("No active scope".to_string());
        };
        let dir = scope.skills_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create skills dir: {}", e))?;
        let slug = name.to_lowercase().replace(' ', "-");
        let file_path = dir.join(format!("{}.md", slug));
        if file_path.exists() {
            return Err(format!("Skill '{}' already exists", slug));
        }
        let template = format!("---\nname: {}\ndescription: \n---\n\n# {}\n\n", slug, name);
        std::fs::write(&file_path, &template).map_err(|e| format!("Failed to write: {}", e))?;
        self.new_skill_name.clear();
        self.discover_skills();
        if let Some(idx) = self
            .skills
            .iter()
            .position(|s| s.file_path == file_path.to_string_lossy())
        {
            self.open_skill(idx);
        }
        Ok(())
    }
}

/// Stub -- will be replaced in Task 2 with the real analyzer.
pub fn analyze_skill(_content: &str) -> Vec<SkillDiagnostic> {
    Vec::new()
}

fn parse_frontmatter_from_file(path: &std::path::Path) -> (Option<String>, Option<String>) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return (None, None);
    };
    parse_frontmatter(&content)
}

fn parse_frontmatter(content: &str) -> (Option<String>, Option<String>) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, None);
    }
    let after_open = &trimmed[3..];
    let Some(close_pos) = after_open.find("\n---") else {
        return (None, None);
    };
    let frontmatter = &after_open[..close_pos];
    let mut name = None;
    let mut description = None;
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            let val = val.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                name = Some(val.to_string());
            }
        } else if let Some(val) = line.strip_prefix("description:") {
            let val = val.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                description = Some(val.to_string());
            }
        }
    }
    (name, description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_basic() {
        let content = "---\nname: my-skill\ndescription: Does stuff\n---\n\n# Body";
        let (name, desc) = parse_frontmatter(content);
        assert_eq!(name.as_deref(), Some("my-skill"));
        assert_eq!(desc.as_deref(), Some("Does stuff"));
    }

    #[test]
    fn test_parse_frontmatter_quoted() {
        let content = "---\nname: \"quoted-name\"\ndescription: 'quoted desc'\n---\n";
        let (name, desc) = parse_frontmatter(content);
        assert_eq!(name.as_deref(), Some("quoted-name"));
        assert_eq!(desc.as_deref(), Some("quoted desc"));
    }

    #[test]
    fn test_parse_frontmatter_missing() {
        let (name, desc) = parse_frontmatter("# No frontmatter");
        assert!(name.is_none());
        assert!(desc.is_none());
    }

    #[test]
    fn test_parse_frontmatter_empty_values() {
        let content = "---\nname: \ndescription: \n---\n";
        let (name, desc) = parse_frontmatter(content);
        assert!(name.is_none());
        assert!(desc.is_none());
    }

    #[test]
    fn test_skill_scope_label() {
        assert_eq!(SkillScope::User.label(), "User Skills");
        let proj = SkillScope::Project {
            workspace_id: "w1".into(),
            workspace_name: "My Project".into(),
            folder_path: "/tmp".into(),
        };
        assert_eq!(proj.label(), "My Project");
    }

    #[test]
    fn test_state_new() {
        let state = ClaudeCodeManagerState::new();
        assert!(state.scopes.is_empty());
        assert!(state.skills.is_empty());
        assert_eq!(state.active_scope, 0);
    }

    #[test]
    fn test_refresh_scopes_deduplicates() {
        let mut state = ClaudeCodeManagerState::new();
        state.refresh_scopes(&[
            ("w1".into(), "Proj A".into(), "/proj/a".into()),
            ("w2".into(), "Proj A (wt)".into(), "/proj/a".into()),
            ("w3".into(), "Proj B".into(), "/proj/b".into()),
        ]);
        // User + Proj A + Proj B = 3 (duplicate /proj/a deduplicated)
        assert_eq!(state.scopes.len(), 3);
    }
}
