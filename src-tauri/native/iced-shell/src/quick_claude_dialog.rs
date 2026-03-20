use iced::widget::{button, column, container, row, scrollable, text, text_input, text_editor, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

#[derive(Debug, Clone)]
pub enum SkillScope {
    Project,
    User,
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub scope: SkillScope,
    pub file_path: String,
}

/// State for the Quick Claude dialog.
#[derive(Debug)]
pub struct QuickClaudeDialogState {
    pub selected_workspace_id: Option<String>,
    pub workspace_dropdown_open: bool,
    pub selected_ai_tool: String,
    pub ai_tool_dropdown_open: bool,
    pub prompt_content: text_editor::Content,
    pub branch_name: String,
    pub main_branch_mode: bool,
    pub auto_suggest_branch: bool,
    /// Snapshot of available workspaces (id, name) at dialog open time.
    pub workspaces: Vec<(String, String)>,
    pub skills: Vec<SkillEntry>,
    pub skill_autocomplete_open: bool,
    pub skill_autocomplete_filter: String,
    pub skill_autocomplete_selected: usize,
}

impl QuickClaudeDialogState {
    pub fn new(workspace_id: Option<String>, workspaces: Vec<(String, String)>) -> Self {
        Self {
            selected_workspace_id: workspace_id,
            workspace_dropdown_open: false,
            selected_ai_tool: "Claude Code".to_string(),
            ai_tool_dropdown_open: false,
            prompt_content: text_editor::Content::new(),
            branch_name: String::new(),
            main_branch_mode: false,
            auto_suggest_branch: true,
            workspaces,
            skills: Vec::new(),
            skill_autocomplete_open: false,
            skill_autocomplete_filter: String::new(),
            skill_autocomplete_selected: 0,
        }
    }

    pub fn prompt_text(&self) -> String {
        self.prompt_content.text()
    }

    pub fn filtered_skills(&self) -> Vec<&SkillEntry> {
        if self.skill_autocomplete_filter.is_empty() {
            self.skills.iter().collect()
        } else {
            let filter = self.skill_autocomplete_filter.to_lowercase();
            self.skills
                .iter()
                .filter(|s| s.name.to_lowercase().contains(&filter))
                .collect()
        }
    }
}

/// Render the Quick Claude dialog as a modal overlay.
pub fn view_quick_claude_dialog<'a, M: Clone + 'a>(
    state: &'a QuickClaudeDialogState,
    on_workspace_selected: impl Fn(String) -> M + 'a,
    on_workspace_dropdown_toggle: M,
    on_ai_tool_selected: impl Fn(String) -> M + 'a,
    on_ai_tool_dropdown_toggle: M,
    on_prompt_action: impl Fn(text_editor::Action) -> M + 'a,
    on_branch_changed: impl Fn(String) -> M + 'a,
    on_main_branch_toggled: impl Fn(bool) -> M + 'a,
    on_auto_suggest_toggled: impl Fn(bool) -> M + 'a,
    on_launch: M,
    on_voice: M,
    on_cancel: M,
    on_skill_selected: impl Fn(usize) -> M + 'a,
    _on_skill_autocomplete_navigate: impl Fn(i32) -> M + 'a,
    _on_skill_autocomplete_dismiss: M,
) -> Element<'a, M> {
    let accent = theme::ACCENT();
    let border_color = theme::BORDER();
    let bg_secondary = theme::BG_SECONDARY();
    let bg_primary = theme::BG_PRIMARY();
    let text_active = theme::TEXT_ACTIVE();
    let text_primary = theme::TEXT_PRIMARY();
    let text_secondary = theme::TEXT_SECONDARY();
    let backdrop = theme::BACKDROP();

    // ── Title ────────────────────────────────────────────────────────────
    let title = text("Quick Claude").size(18).color(text_active);
    let subtitle = text("Ctrl+Enter to launch \u{00B7} Escape to cancel")
        .size(11)
        .color(text_secondary);
    let step_indicator = text("\u{2460} Workspace \u{2192} \u{2461} Prompt \u{2192} \u{2462} Launch")
        .size(12)
        .color(text_secondary);

    // ── Workspace dropdown ───────────────────────────────────────────────
    let current_ws_name = state
        .selected_workspace_id
        .as_ref()
        .and_then(|id| {
            state.workspaces
                .iter()
                .find(|(ws_id, _)| ws_id == id)
                .map(|(_, name)| name.as_str())
        })
        .unwrap_or("Select workspace...");

    let ws_toggle = on_workspace_dropdown_toggle.clone();
    let ws_button = button(
        row![
            text(current_ws_name).size(13).color(text_primary),
            Space::new().width(Length::Fill),
            text(if state.workspace_dropdown_open { "\u{25B2}" } else { "\u{25BC}" })
                .size(10)
                .color(text_secondary),
        ]
        .align_y(iced::Alignment::Center),
    )
    .on_press(ws_toggle)
    .padding(Padding::from([8, 12]))
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: Some(Background::Color(bg_primary)),
        text_color: text_primary,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..button::Style::default()
    });

    let mut workspace_section = column![ws_button].spacing(0);

    if state.workspace_dropdown_open {
        let mut ws_list = column![].spacing(2);
        for (ws_id, ws_name) in &state.workspaces {
            let is_selected = state.selected_workspace_id.as_deref() == Some(ws_id.as_str());
            let id_clone = ws_id.clone();
            let ws_name_ref = ws_name.as_str();
            let ws_item = button(
                text(ws_name_ref)
                    .size(13)
                    .color(if is_selected { text_active } else { text_primary }),
            )
            .on_press(on_workspace_selected(id_clone))
            .padding(Padding::from([5, 10]))
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(if is_selected {
                    tint(accent, 0.12)
                } else {
                    Color::TRANSPARENT
                })),
                text_color: text_primary,
                border: Border::default(),
                ..button::Style::default()
            });
            ws_list = ws_list.push(ws_item);
        }
        workspace_section = workspace_section.push(
            container(scrollable(ws_list).height(Length::Shrink))
                .max_height(140.0)
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(bg_primary)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    // ── AI tool dropdown ─────────────────────────────────────────────────
    let ai_tools = ["Claude Code", "Codex", "Custom"];

    let ai_toggle = on_ai_tool_dropdown_toggle.clone();
    let ai_button = button(
        row![
            text(state.selected_ai_tool.as_str()).size(13).color(text_primary),
            Space::new().width(Length::Fill),
            text(if state.ai_tool_dropdown_open { "\u{25B2}" } else { "\u{25BC}" })
                .size(10)
                .color(text_secondary),
        ]
        .align_y(iced::Alignment::Center),
    )
    .on_press(ai_toggle)
    .padding(Padding::from([8, 12]))
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: Some(Background::Color(bg_primary)),
        text_color: text_primary,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..button::Style::default()
    });

    let mut ai_tool_section = column![
        text("AI Tool").size(11).color(text_secondary),
        ai_button,
    ]
    .spacing(4);

    if state.ai_tool_dropdown_open {
        let mut ai_list = column![].spacing(2);
        for tool_name in ai_tools {
            let is_selected = state.selected_ai_tool == tool_name;
            let tool_str = tool_name.to_string();
            let ai_item = button(
                text(tool_name)
                    .size(13)
                    .color(if is_selected { text_active } else { text_primary }),
            )
            .on_press(on_ai_tool_selected(tool_str))
            .padding(Padding::from([5, 10]))
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(if is_selected {
                    tint(accent, 0.12)
                } else {
                    Color::TRANSPARENT
                })),
                text_color: text_primary,
                border: Border::default(),
                ..button::Style::default()
            });
            ai_list = ai_list.push(ai_item);
        }
        ai_tool_section = ai_tool_section.push(
            container(ai_list)
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(bg_primary)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    // ── Prompt textarea ──────────────────────────────────────────────────
    let editor = text_editor(&state.prompt_content)
        .on_action(on_prompt_action)
        .padding(12)
        .height(Length::Fixed(140.0));

    let editor_container = container(editor)
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_primary)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    let mut prompt_section = column![
        text("Prompt").size(11).color(text_secondary),
        editor_container,
    ]
    .spacing(4);

    // Skill autocomplete popup
    if state.skill_autocomplete_open && state.selected_ai_tool == "Claude Code" {
        let filtered = state.filtered_skills();
        if !filtered.is_empty() {
            let mut skill_list = column![].spacing(1);
            for (i, skill) in filtered.iter().enumerate().take(8) {
                let is_selected = i == state.skill_autocomplete_selected;
                let idx = i;
                let scope_label = match skill.scope {
                    SkillScope::Project => "project",
                    SkillScope::User => "user",
                };
                let skill_item = button(
                    row![
                        text(format!("/{}", skill.name)).size(12).color(if is_selected { text_active } else { text_primary }),
                        Space::new().width(Length::Fill),
                        text(scope_label).size(10).color(text_secondary),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(on_skill_selected(idx))
                .padding(Padding::from([4, 8]))
                .width(Length::Fill)
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(if is_selected {
                        tint(accent, 0.15)
                    } else {
                        Color::TRANSPARENT
                    })),
                    text_color: text_primary,
                    border: Border::default(),
                    ..button::Style::default()
                });
                skill_list = skill_list.push(skill_item);
            }

            // Show description of selected skill
            if let Some(skill) = filtered.get(state.skill_autocomplete_selected) {
                if !skill.description.is_empty() {
                    let desc_text = text(&skill.description).size(11).color(text_secondary);
                    skill_list = skill_list.push(
                        container(desc_text)
                            .padding(Padding::from([4, 8]))
                            .width(Length::Fill),
                    );
                }
            }

            let popup = container(skill_list)
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(bg_secondary)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                        offset: Vector::new(0.0, 2.0),
                        blur_radius: 8.0,
                    },
                    ..container::Style::default()
                });
            prompt_section = prompt_section.push(popup);
        }
    }

    // ── Branch name input ────────────────────────────────────────────────
    let branch_input = text_input(
        "Branch name (optional, auto-generated if empty)",
        &state.branch_name,
    )
    .on_input(on_branch_changed)
    .size(13)
    .padding(Padding::from([6, 10]));

    let branch_section = column![
        text("Branch").size(11).color(text_secondary),
        branch_input,
    ]
    .spacing(4);

    // ── Checkbox row ─────────────────────────────────────────────────────
    let main_branch_indicator = if state.main_branch_mode {
        "\u{2611}"
    } else {
        "\u{2610}"
    };
    let main_branch_btn = button(
        row![
            text(main_branch_indicator).size(14).color(accent),
            text("Open in main branch (no worktree)")
                .size(12)
                .color(text_primary)
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .on_press(on_main_branch_toggled(!state.main_branch_mode))
    .padding(Padding::from([4, 8]))
    .style(move |_theme, _status| button::Style {
        background: None,
        border: Border::default(),
        ..button::Style::default()
    });

    let auto_suggest_indicator = if state.auto_suggest_branch {
        "\u{2611}"
    } else {
        "\u{2610}"
    };
    let auto_suggest_btn = button(
        row![
            text(auto_suggest_indicator).size(14).color(accent),
            text("Auto-suggest branch name")
                .size(12)
                .color(text_primary)
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .on_press(on_auto_suggest_toggled(!state.auto_suggest_branch))
    .padding(Padding::from([4, 8]))
    .style(move |_theme, _status| button::Style {
        background: None,
        border: Border::default(),
        ..button::Style::default()
    });

    let checkbox_row = row![main_branch_btn, auto_suggest_btn].spacing(12);

    // ── Footer buttons ───────────────────────────────────────────────────
    let cancel_btn = button(text("Cancel").size(13))
        .on_press(on_cancel)
        .padding(Padding::from([6, 16]));

    let voice_btn = button(text("Voice").size(13))
        .on_press(on_voice)
        .padding(Padding::from([6, 16]));

    let launch_btn = button(text("Launch").size(13).color(text_active))
        .on_press(on_launch)
        .padding(Padding::from([6, 16]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(accent)),
            text_color: Color::WHITE,
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });

    let footer = row![
        cancel_btn,
        Space::new().width(Length::Fill),
        voice_btn,
        launch_btn,
    ]
    .spacing(8);

    // ── Compose dialog ───────────────────────────────────────────────────
    let dialog_content = column![
        title,
        subtitle,
        step_indicator,
        workspace_section,
        ai_tool_section,
        prompt_section,
        branch_section,
        checkbox_row,
        footer,
    ]
    .spacing(12);

    let dialog = container(dialog_content)
        .padding(Padding::from([20, 24]))
        .width(Length::Fixed(520.0))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_secondary)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        });

    // Backdrop + centered dialog
    container(iced::widget::center(dialog))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(backdrop)),
            ..container::Style::default()
        })
        .into()
}

/// Discover Claude Code skills from project and user directories.
pub fn discover_skills(workspace_folder: Option<&str>) -> Vec<SkillEntry> {
    let mut skills = Vec::new();

    // Project skills: {workspace}/.claude/skills/**/*.md
    if let Some(folder) = workspace_folder {
        let project_skills_dir = std::path::Path::new(folder).join(".claude").join("skills");
        if project_skills_dir.exists() {
            collect_skills_from_dir(&project_skills_dir, SkillScope::Project, &mut skills);
        }
    }

    // User skills: ~/.claude/skills/**/*.md
    if let Some(home) = std::env::var("USERPROFILE")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
    {
        let user_skills_dir = std::path::Path::new(&home).join(".claude").join("skills");
        if user_skills_dir.exists() {
            collect_skills_from_dir(&user_skills_dir, SkillScope::User, &mut skills);
        }
    }

    skills
}

fn collect_skills_from_dir(
    dir: &std::path::Path,
    scope: SkillScope,
    skills: &mut Vec<SkillEntry>,
) {
    let Ok(entries) = walkdir_recursive(dir) else {
        return;
    };
    for path in entries {
        if path.extension().map_or(false, |e| e == "md") {
            let name = if path.file_name().map_or(false, |f| f == "SKILL.md") {
                // Use parent directory name for SKILL.md files
                path.parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
                // Use filename without extension
                path.file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            };

            if name.is_empty() {
                continue;
            }

            let description = read_first_heading(&path).unwrap_or_default();

            skills.push(SkillEntry {
                name,
                description,
                scope: scope.clone(),
                file_path: path.to_string_lossy().to_string(),
            });
        }
    }
}

/// Recursively collect files from a directory.
fn walkdir_recursive(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut results = Vec::new();
    fn walk(
        dir: &std::path::Path,
        results: &mut Vec<std::path::PathBuf>,
    ) -> Result<(), std::io::Error> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, results)?;
            } else {
                results.push(path);
            }
        }
        Ok(())
    }
    walk(dir, &mut results)?;
    Ok(results)
}

/// Read the first `# Heading` line from a markdown file.
fn read_first_heading(path: &std::path::Path) -> Option<String> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().take(20) {
        let line = line.ok()?;
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return Some(heading.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_workspaces() -> Vec<(String, String)> {
        vec![
            ("ws-1".to_string(), "My Workspace".to_string()),
            ("ws-2".to_string(), "Other".to_string()),
        ]
    }

    #[test]
    fn state_new_defaults() {
        let state = QuickClaudeDialogState::new(Some("ws-1".into()), sample_workspaces());
        assert_eq!(state.selected_workspace_id, Some("ws-1".into()));
        assert_eq!(state.selected_ai_tool, "Claude Code");
        assert!(!state.workspace_dropdown_open);
        assert!(!state.ai_tool_dropdown_open);
        assert!(state.branch_name.is_empty());
        assert!(!state.main_branch_mode);
        assert!(state.auto_suggest_branch);
        assert_eq!(state.workspaces.len(), 2);
        assert!(state.skills.is_empty());
        assert!(!state.skill_autocomplete_open);
        assert!(state.skill_autocomplete_filter.is_empty());
        assert_eq!(state.skill_autocomplete_selected, 0);
    }

    #[test]
    fn state_new_no_workspace() {
        let state = QuickClaudeDialogState::new(None, vec![]);
        assert!(state.selected_workspace_id.is_none());
    }

    #[test]
    fn prompt_text_empty() {
        let state = QuickClaudeDialogState::new(None, vec![]);
        assert!(state.prompt_text().trim().is_empty());
    }

    #[test]
    fn view_does_not_panic() {
        let state = QuickClaudeDialogState::new(Some("ws-1".into()), sample_workspaces());
        #[derive(Debug, Clone)]
        enum Msg {
            WsSelected(String),
            WsDropdown,
            AiSelected(String),
            AiDropdown,
            PromptAction(text_editor::Action),
            BranchChanged(String),
            MainBranch(bool),
            AutoSuggest(bool),
            Launch,
            Voice,
            Cancel,
            SkillSelected(usize),
            SkillNav(i32),
            SkillDismiss,
        }
        let _el: Element<'_, Msg> = view_quick_claude_dialog(
            &state,
            Msg::WsSelected,
            Msg::WsDropdown,
            Msg::AiSelected,
            Msg::AiDropdown,
            Msg::PromptAction,
            Msg::BranchChanged,
            Msg::MainBranch,
            Msg::AutoSuggest,
            Msg::Launch,
            Msg::Voice,
            Msg::Cancel,
            Msg::SkillSelected,
            Msg::SkillNav,
            Msg::SkillDismiss,
        );
    }

    #[test]
    fn filtered_skills_empty_filter() {
        let mut state = QuickClaudeDialogState::new(None, vec![]);
        state.skills = vec![
            SkillEntry {
                name: "commit".to_string(),
                description: "Create a commit".to_string(),
                scope: SkillScope::User,
                file_path: "/test/commit.md".to_string(),
            },
            SkillEntry {
                name: "review-pr".to_string(),
                description: "Review a PR".to_string(),
                scope: SkillScope::Project,
                file_path: "/test/review-pr.md".to_string(),
            },
        ];
        assert_eq!(state.filtered_skills().len(), 2);
    }

    #[test]
    fn filtered_skills_with_filter() {
        let mut state = QuickClaudeDialogState::new(None, vec![]);
        state.skills = vec![
            SkillEntry {
                name: "commit".to_string(),
                description: "Create a commit".to_string(),
                scope: SkillScope::User,
                file_path: "/test/commit.md".to_string(),
            },
            SkillEntry {
                name: "review-pr".to_string(),
                description: "Review a PR".to_string(),
                scope: SkillScope::Project,
                file_path: "/test/review-pr.md".to_string(),
            },
        ];
        state.skill_autocomplete_filter = "comm".to_string();
        let filtered = state.filtered_skills();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "commit");
    }

    #[test]
    fn discover_skills_no_project_dir() {
        // Non-existent workspace folder should not add project skills
        let skills = discover_skills(Some("/nonexistent/path/for/test"));
        // Only user-level skills (if any) should be present — no project skills
        for skill in &skills {
            assert!(
                !matches!(skill.scope, SkillScope::Project),
                "should not find project skills for non-existent workspace"
            );
        }
    }
}
