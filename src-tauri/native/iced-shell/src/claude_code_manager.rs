use iced::widget::{
    button, column, container, row, scrollable, text, text_editor, text_input, Space,
};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::theme::{
    ACCENT, ACCENT_HOVER, BG_PRIMARY, BG_SECONDARY, BG_TERTIARY, BORDER, BORDER_FOCUSED,
    GHOST_HOVER, GHOST_SELECTED, RADIUS_MD, RADIUS_SM, TEXT_ACTIVE, TEXT_PRIMARY, TEXT_SECONDARY,
};

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

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

/// Analyze a SKILL.md file content and produce diagnostics.
pub fn analyze_skill(content: &str) -> Vec<SkillDiagnostic> {
    let mut diags = Vec::new();
    let (name, description) = parse_frontmatter(content);

    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        diags.push(SkillDiagnostic {
            severity: DiagnosticSeverity::Error,
            message: "Missing YAML frontmatter block (---).".to_string(),
            suggestion: Some("Add a frontmatter block at the top:\n---\nname: my-skill\ndescription: When to use this skill\n---".to_string()),
        });
        return diags;
    }

    if name.is_none() {
        diags.push(SkillDiagnostic {
            severity: DiagnosticSeverity::Error,
            message: "Missing required 'name' field in frontmatter.".to_string(),
            suggestion: Some("Add: name: my-skill-name".to_string()),
        });
    }

    if description.is_none() {
        diags.push(SkillDiagnostic {
            severity: DiagnosticSeverity::Error,
            message: "Missing required 'description' field in frontmatter.".to_string(),
            suggestion: Some("Add: description: Use this skill when...".to_string()),
        });
    }

    if let Some(ref desc) = description {
        let word_count = desc.split_whitespace().count();
        if word_count < 5 {
            diags.push(SkillDiagnostic {
                severity: DiagnosticSeverity::Warning,
                message: format!(
                    "Description is too short ({} words). Aim for 10-30 words to help with skill triggering.",
                    word_count
                ),
                suggestion: Some("Describe WHEN to use this skill: 'Use this skill when...'".to_string()),
            });
        } else if word_count > 100 {
            diags.push(SkillDiagnostic {
                severity: DiagnosticSeverity::Warning,
                message: format!(
                    "Description is very long ({} words). Keep it under ~100 words for the autocomplete menu.",
                    word_count
                ),
                suggestion: Some("Move detailed instructions to the body; keep description as a concise trigger guide.".to_string()),
            });
        }

        if !desc.contains("when") && !desc.contains("Use") && !desc.contains("use") {
            diags.push(SkillDiagnostic {
                severity: DiagnosticSeverity::Tip,
                message: "Description doesn't mention when to use this skill.".to_string(),
                suggestion: Some("Start with 'Use when...' or 'Use this skill when...' for better discoverability.".to_string()),
            });
        }
    }

    let body = extract_body(content);
    if body.trim().is_empty() {
        diags.push(SkillDiagnostic {
            severity: DiagnosticSeverity::Warning,
            message: "Skill has no body content after frontmatter.".to_string(),
            suggestion: Some("Add instructions that tell the agent what to do when this skill is invoked.".to_string()),
        });
    } else {
        for line in body.lines() {
            let line = line.trim();
            if let Some(heading) = line.strip_prefix("# ") {
                let heading = heading.trim();
                let words: Vec<&str> = heading.split_whitespace().collect();
                if words.len() <= 3 {
                    let first = words.first().map(|w| w.to_lowercase()).unwrap_or_default();
                    let action_starters = [
                        "run", "create", "build", "fix", "analyze", "check", "deploy",
                        "test", "generate", "implement", "design", "write", "update",
                        "scan", "audit", "review", "debug", "profile", "monitor",
                        "diagnose", "validate", "verify", "configure", "setup", "install",
                    ];
                    if !action_starters.iter().any(|v| first == *v) {
                        diags.push(SkillDiagnostic {
                            severity: DiagnosticSeverity::Tip,
                            message: format!(
                                "Top-level heading '# {}' looks like a noun phrase. The # heading becomes the skill description in autocomplete.",
                                heading
                            ),
                            suggestion: Some("Write it as a usage-oriented sentence: '# Run analysis and fix issues' instead of '# Fix Contract'.".to_string()),
                        });
                    }
                }
                break;
            }
        }

        let body_lines = body.lines().count();
        if body_lines > 500 {
            diags.push(SkillDiagnostic {
                severity: DiagnosticSeverity::Warning,
                message: format!(
                    "Skill body is {} lines long. Very long skills consume context window budget.",
                    body_lines
                ),
                suggestion: Some("Consider splitting into sub-skills or moving reference docs to a references/ subdirectory.".to_string()),
            });
        }
    }

    if diags.is_empty() {
        diags.push(SkillDiagnostic {
            severity: DiagnosticSeverity::Tip,
            message: "Skill looks well-structured! No issues found.".to_string(),
            suggestion: None,
        });
    }

    diags
}

/// Extract the body content after the frontmatter block.
fn extract_body(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }
    let after_open = &trimmed[3..];
    if let Some(close_pos) = after_open.find("\n---") {
        let after_close = &after_open[close_pos + 4..];
        after_close.strip_prefix('\n').unwrap_or(after_close)
    } else {
        ""
    }
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

/// Renders the full Claude Code Manager tab content.
/// Layout: [ Vertical Scope Tabs | Content (list or editor) ]
pub fn view_claude_code_manager<'a, M: Clone + 'a>(
    state: &'a ClaudeCodeManagerState,
    on_scope_click: impl Fn(usize) -> M + 'a,
    on_skill_click: impl Fn(usize) -> M + 'a,
    on_editor_action: impl Fn(text_editor::Action) -> M + 'a,
    on_save: M,
    on_close_editor: M,
    on_new_skill_name: impl Fn(String) -> M + 'a,
    on_create_skill: M,
) -> Element<'a, M> {
    let mut scope_col = column![
        text("Scope").size(11).color(TEXT_SECONDARY()),
    ]
    .spacing(4)
    .width(Length::Fixed(160.0));

    for (i, scope) in state.scopes.iter().enumerate() {
        let is_active = i == state.active_scope;
        let label = scope.label();
        let icon = match scope {
            SkillScope::User => "\u{1F464} ",
            SkillScope::Project { .. } => "\u{1F4C1} ",
        };

        let btn = button(
            text(format!("{}{}", icon, label)).size(12)
        )
        .on_press(on_scope_click(i))
        .width(Length::Fill)
        .padding(Padding::from([6, 10]))
        .style(move |_theme, status| {
            let (bg, border_color) = if is_active {
                (GHOST_SELECTED(), BORDER_FOCUSED())
            } else {
                match status {
                    button::Status::Hovered => (GHOST_HOVER(), Color::TRANSPARENT),
                    _ => (Color::TRANSPARENT, Color::TRANSPARENT),
                }
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() },
                border: Border {
                    color: border_color,
                    width: if is_active { 1.0 } else { 0.0 },
                    radius: RADIUS_SM.into(),
                },
                ..button::Style::default()
            }
        });
        scope_col = scope_col.push(btn);
    }

    let scope_panel = container(scrollable(scope_col).height(Length::Fill))
        .padding(Padding::from([8, 8]))
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY(), 0.6))),
            border: Border {
                color: tint(BORDER(), 0.5),
                width: 0.0,
                radius: RADIUS_MD.into(),
            },
            ..container::Style::default()
        });

    let content_panel = if let Some(edit_idx) = state.editing_skill_index {
        view_skill_editor(state, edit_idx, on_editor_action, on_save, on_close_editor)
    } else {
        view_skill_list(state, on_skill_click, on_new_skill_name, on_create_skill)
    };

    row![scope_panel, content_panel]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_skill_list<'a, M: Clone + 'a>(
    state: &'a ClaudeCodeManagerState,
    on_skill_click: impl Fn(usize) -> M + 'a,
    on_new_skill_name: impl Fn(String) -> M + 'a,
    on_create_skill: M,
) -> Element<'a, M> {
    let scope_label = state
        .scopes
        .get(state.active_scope)
        .map(|s| s.label())
        .unwrap_or_else(|| "Skills".to_string());

    let mut skill_cards = column![].spacing(6).width(Length::Fill);

    if state.skills.is_empty() {
        skill_cards = skill_cards.push(
            container(
                text("No skills found in this scope.")
                    .size(12)
                    .color(TEXT_SECONDARY()),
            )
            .padding(20),
        );
    } else {
        for (i, skill) in state.skills.iter().enumerate() {
            let type_badge = if skill.is_directory { "dir" } else { "file" };
            let desc_preview = if skill.description.chars().count() > 80 {
                let truncated: String = skill.description.chars().take(77).collect();
                format!("{}...", truncated)
            } else if skill.description.is_empty() {
                "(no description)".to_string()
            } else {
                skill.description.clone()
            };

            let card = button(
                column![
                    row![
                        text(&skill.name).size(13).color(TEXT_ACTIVE()),
                        Space::new().width(Length::Fill),
                        text(type_badge).size(10).color(TEXT_SECONDARY()),
                    ]
                    .align_y(iced::Alignment::Center),
                    text(desc_preview).size(11).color(TEXT_PRIMARY()),
                ]
                .spacing(3)
                .width(Length::Fill),
            )
            .on_press(on_skill_click(i))
            .width(Length::Fill)
            .padding(Padding::from([8, 12]))
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered => BG_TERTIARY(),
                    _ => BG_SECONDARY(),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: TEXT_PRIMARY(),
                    border: Border {
                        color: tint(BORDER(), 0.6),
                        width: 1.0,
                        radius: RADIUS_SM.into(),
                    },
                    ..button::Style::default()
                }
            });
            skill_cards = skill_cards.push(card);
        }
    }

    let create_row = row![
        text_input("New skill name...", &state.new_skill_name)
            .on_input(on_new_skill_name)
            .padding(Padding::from([4, 8]))
            .size(12)
            .width(Length::Fill),
        button(text("Create").size(12).color(TEXT_PRIMARY()))
            .on_press(on_create_skill)
            .padding(Padding::from([4, 10])),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    container(
        column![
            text(scope_label).size(14).color(TEXT_ACTIVE()),
            create_row,
            scrollable(skill_cards).height(Length::Fill),
        ]
        .spacing(10)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::from([0, 4]))
    .into()
}

fn view_skill_editor<'a, M: Clone + 'a>(
    state: &'a ClaudeCodeManagerState,
    edit_idx: usize,
    on_editor_action: impl Fn(text_editor::Action) -> M + 'a,
    on_save: M,
    on_close_editor: M,
) -> Element<'a, M> {
    let skill = &state.skills[edit_idx];
    let title = format!("Editing: {}", skill.name);
    let save_label = if state.editor_dirty { "Save *" } else { "Save" };

    let header = row![
        button(text("\u{2190} Back").size(12).color(TEXT_PRIMARY()))
            .on_press(on_close_editor)
            .padding(Padding::from([4, 8]))
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered => GHOST_HOVER(),
                    _ => Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: TEXT_PRIMARY(),
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: RADIUS_SM.into(),
                    },
                    ..button::Style::default()
                }
            }),
        text(title).size(14).color(TEXT_ACTIVE()),
        Space::new().width(Length::Fill),
        text("Ctrl+S to save").size(10).color(TEXT_SECONDARY()),
        button(text(save_label).size(12).color(TEXT_PRIMARY()))
            .on_press(on_save)
            .padding(Padding::from([4, 10]))
            .style(|_theme, status| {
                let (bg, border_c) = match status {
                    button::Status::Hovered => (tint(ACCENT(), 0.30), ACCENT_HOVER()),
                    _ => (tint(ACCENT(), 0.18), ACCENT()),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: TEXT_ACTIVE(),
                    border: Border {
                        color: border_c,
                        width: 1.0,
                        radius: RADIUS_SM.into(),
                    },
                    ..button::Style::default()
                }
            }),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let editor = text_editor(&state.editor_content)
        .on_action(on_editor_action)
        .padding(10)
        .height(Length::Fill);

    let editor_pane = container(editor)
        .width(Length::Fill)
        .height(Length::FillPortion(3))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY(), 0.85))),
            border: Border {
                color: tint(BORDER(), 0.6),
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..container::Style::default()
        });

    let mut diag_items = column![
        text("Skill Analysis").size(12).color(TEXT_SECONDARY()),
    ]
    .spacing(4)
    .width(Length::Fill);

    for diag in &state.diagnostics {
        let (icon, color) = match diag.severity {
            DiagnosticSeverity::Error => ("\u{2716}", Color::from_rgb(0.9, 0.3, 0.3)),
            DiagnosticSeverity::Warning => ("\u{26A0}", Color::from_rgb(0.9, 0.7, 0.2)),
            DiagnosticSeverity::Tip => ("\u{2139}", Color::from_rgb(0.3, 0.7, 0.9)),
        };

        let mut card_content = column![
            text(format!("{} {}", icon, diag.message)).size(12).color(color),
        ]
        .spacing(2);

        if let Some(ref suggestion) = diag.suggestion {
            card_content = card_content.push(
                text(format!("  \u{2192} {}", suggestion))
                    .size(11)
                    .color(TEXT_SECONDARY()),
            );
        }

        let card = container(card_content)
            .padding(Padding::from([6, 10]))
            .width(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(tint(BG_TERTIARY(), 0.5))),
                border: Border {
                    color: tint(BORDER(), 0.4),
                    width: 1.0,
                    radius: RADIUS_SM.into(),
                },
                ..container::Style::default()
            });
        diag_items = diag_items.push(card);
    }

    let diag_panel = container(scrollable(diag_items).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::FillPortion(1))
        .padding(Padding::from([4, 0]));

    let footer = text(format!("{}", skill.file_path))
        .size(10)
        .color(TEXT_SECONDARY());

    container(
        column![header, editor_pane, diag_panel, footer]
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding::from([0, 4]))
    .into()
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

    #[test]
    fn test_analyze_missing_frontmatter() {
        let diags = analyze_skill("# Just a heading\nNo frontmatter.");
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Error
            && d.message.contains("frontmatter")));
    }

    #[test]
    fn test_analyze_missing_name() {
        let diags = analyze_skill("---\ndescription: foo\n---\n# Body");
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Error
            && d.message.contains("name")));
    }

    #[test]
    fn test_analyze_missing_description() {
        let diags = analyze_skill("---\nname: my-skill\n---\n# Body");
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Error
            && d.message.contains("description")));
    }

    #[test]
    fn test_analyze_short_description() {
        let diags = analyze_skill("---\nname: x\ndescription: Short\n---\n");
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Warning
            && d.message.contains("Description") && d.message.contains("short")));
    }

    #[test]
    fn test_analyze_bad_heading() {
        let content = "---\nname: helper\ndescription: Helps with things quickly for the project\n---\n\n# My Helper\n";
        let diags = analyze_skill(content);
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Tip
            && d.message.contains("heading")));
    }

    #[test]
    fn test_analyze_good_skill() {
        let content = "---\nname: my-skill\ndescription: Use this skill when you need to do something important and complex that requires careful analysis\n---\n\n# Run analysis and produce a detailed report\n\nInstructions here.";
        let diags = analyze_skill(content);
        assert!(!diags.iter().any(|d| d.severity == DiagnosticSeverity::Error));
    }

    #[test]
    fn test_analyze_no_body() {
        let content = "---\nname: empty\ndescription: Has a reasonable description for this\n---\n";
        let diags = analyze_skill(content);
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Warning
            && d.message.contains("body")));
    }

    #[test]
    fn test_analyze_long_description() {
        let long_desc = "word ".repeat(200);
        let content = format!("---\nname: verbose\ndescription: {}\n---\n# Do stuff\nBody.", long_desc);
        let diags = analyze_skill(&content);
        assert!(diags.iter().any(|d| d.severity == DiagnosticSeverity::Warning
            && d.message.contains("long")));
    }

    #[test]
    fn test_view_does_not_panic() {
        #[derive(Debug, Clone)]
        enum Msg {
            Scope(usize),
            Skill(usize),
            EditorAction(text_editor::Action),
            Save,
            CloseEditor,
            NewName(String),
            Create,
        }

        let mut state = ClaudeCodeManagerState::new();
        state.scopes.push(SkillScope::User);
        state.skills.push(SkillEntry {
            name: "test".into(),
            description: "A test skill".into(),
            file_path: "/tmp/test.md".into(),
            is_directory: false,
        });

        let _el: Element<'_, Msg> = view_claude_code_manager(
            &state,
            Msg::Scope,
            Msg::Skill,
            Msg::EditorAction,
            Msg::Save,
            Msg::CloseEditor,
            Msg::NewName,
            Msg::Create,
        );
    }
}
