use iced::widget::{button, column, container, row, scrollable, text, text_input, text_editor, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuickClaudeTab {
    NewPrompt,
    ResumeSession,
}

#[derive(Debug, Clone)]
pub struct ClaudeSession {
    pub session_id: String,
    pub first_message: String,
    pub model: String,
    pub timestamp: String,
    pub branch: String,
    pub file_path: String,
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
    pub active_tab: QuickClaudeTab,
    pub sessions: Vec<ClaudeSession>,
    pub selected_session: Option<usize>,
    pub skills: Vec<SkillEntry>,
    pub skill_autocomplete_open: bool,
    pub skill_autocomplete_filter: String,
    pub skill_autocomplete_selected: usize,
    pub selected_model: String,
    pub model_dropdown_open: bool,
    pub selected_mode: String,
    pub mode_dropdown_open: bool,
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
            active_tab: QuickClaudeTab::NewPrompt,
            sessions: Vec::new(),
            selected_session: None,
            skills: Vec::new(),
            skill_autocomplete_open: false,
            skill_autocomplete_filter: String::new(),
            skill_autocomplete_selected: 0,
            selected_model: "sonnet".to_string(),
            model_dropdown_open: false,
            selected_mode: "default".to_string(),
            mode_dropdown_open: false,
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
    on_model_selected: impl Fn(String) -> M + 'a,
    on_model_dropdown_toggle: M,
    on_mode_selected: impl Fn(String) -> M + 'a,
    on_mode_dropdown_toggle: M,
    on_launch: M,
    on_voice: M,
    on_cancel: M,
    on_skill_selected: impl Fn(usize) -> M + 'a,
    _on_skill_autocomplete_navigate: impl Fn(i32) -> M + 'a,
    _on_skill_autocomplete_dismiss: M,
    on_tab_selected: impl Fn(QuickClaudeTab) -> M + 'a,
    on_session_selected: impl Fn(usize) -> M + 'a,
    on_resume: M,
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

    // ── Tab bar ───────────────────────────────────────────────────────────
    let new_prompt_tab = {
        let is_active = state.active_tab == QuickClaudeTab::NewPrompt;
        button(
            text("New Prompt").size(12).color(if is_active { text_active } else { text_secondary })
        )
        .on_press(on_tab_selected(QuickClaudeTab::NewPrompt))
        .padding(Padding::from([6, 16]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(if is_active { tint(accent, 0.15) } else { Color::TRANSPARENT })),
            border: Border {
                color: if is_active { accent } else { Color::TRANSPARENT },
                width: 0.0,
                radius: 4.0.into(),
            },
            ..button::Style::default()
        })
    };

    let resume_tab = {
        let is_active = state.active_tab == QuickClaudeTab::ResumeSession;
        let session_count = state.sessions.len();
        let label = if session_count > 0 {
            format!("Resume ({})", session_count)
        } else {
            "Resume".to_string()
        };
        button(
            text(label).size(12).color(if is_active { text_active } else { text_secondary })
        )
        .on_press(on_tab_selected(QuickClaudeTab::ResumeSession))
        .padding(Padding::from([6, 16]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(if is_active { tint(accent, 0.15) } else { Color::TRANSPARENT })),
            border: Border {
                color: if is_active { accent } else { Color::TRANSPARENT },
                width: 0.0,
                radius: 4.0.into(),
            },
            ..button::Style::default()
        })
    };

    let tab_bar = container(
        row![new_prompt_tab, resume_tab].spacing(4)
    )
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        border: Border {
            color: border_color,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    });

    // ── Footer buttons ───────────────────────────────────────────────────
    let cancel_btn = button(text("Cancel").size(13))
        .on_press(on_cancel)
        .padding(Padding::from([6, 16]));

    let voice_btn = button(text("Voice").size(13))
        .on_press(on_voice)
        .padding(Padding::from([6, 16]));

    // ── Tab content ──────────────────────────────────────────────────────
    let dialog_content = match state.active_tab {
        QuickClaudeTab::NewPrompt => {
            // ── Workspace dropdown ───────────────────────────────────────
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

            // ── AI tool dropdown ─────────────────────────────────────────
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

            // ── Model & Mode dropdowns (Claude Code only) ───────────────
            let model_mode_section: Option<Element<'a, M>> = if state.selected_ai_tool == "Claude Code" {
                let models = [("Opus", "opus"), ("Sonnet", "sonnet"), ("Haiku", "haiku")];
                let modes = [("Default", "default"), ("Plan", "plan"), ("Auto", "auto")];

                let model_display = models
                    .iter()
                    .find(|(_, v)| *v == state.selected_model.as_str())
                    .map(|(d, _)| *d)
                    .unwrap_or("Sonnet");

                let model_toggle = on_model_dropdown_toggle.clone();
                let model_button = button(
                    row![
                        text(model_display).size(13).color(text_primary),
                        Space::new().width(Length::Fill),
                        text(if state.model_dropdown_open { "\u{25B2}" } else { "\u{25BC}" })
                            .size(10)
                            .color(text_secondary),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(model_toggle)
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

                let mut model_col = column![
                    text("Model").size(11).color(text_secondary),
                    model_button,
                ]
                .spacing(4);

                if state.model_dropdown_open {
                    let mut model_list = column![].spacing(2);
                    for (display, value) in models {
                        let is_selected = state.selected_model == value;
                        let val = value.to_string();
                        let item = button(
                            text(display)
                                .size(13)
                                .color(if is_selected { text_active } else { text_primary }),
                        )
                        .on_press(on_model_selected(val))
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
                        model_list = model_list.push(item);
                    }
                    model_col = model_col.push(
                        container(model_list)
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

                let mode_display = modes
                    .iter()
                    .find(|(_, v)| *v == state.selected_mode.as_str())
                    .map(|(d, _)| *d)
                    .unwrap_or("Default");

                let mode_toggle = on_mode_dropdown_toggle.clone();
                let mode_button = button(
                    row![
                        text(mode_display).size(13).color(text_primary),
                        Space::new().width(Length::Fill),
                        text(if state.mode_dropdown_open { "\u{25B2}" } else { "\u{25BC}" })
                            .size(10)
                            .color(text_secondary),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .on_press(mode_toggle)
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

                let mut mode_col = column![
                    text("Permission Mode").size(11).color(text_secondary),
                    mode_button,
                ]
                .spacing(4);

                if state.mode_dropdown_open {
                    let mut mode_list = column![].spacing(2);
                    for (display, value) in modes {
                        let is_selected = state.selected_mode == value;
                        let val = value.to_string();
                        let item = button(
                            text(display)
                                .size(13)
                                .color(if is_selected { text_active } else { text_primary }),
                        )
                        .on_press(on_mode_selected(val))
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
                        mode_list = mode_list.push(item);
                    }
                    mode_col = mode_col.push(
                        container(mode_list)
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

                let model_mode_row = row![
                    container(model_col).width(Length::FillPortion(1)),
                    container(mode_col).width(Length::FillPortion(1)),
                ]
                .spacing(12);

                Some(model_mode_row.into())
            } else {
                None
            };

            // ── Prompt textarea ──────────────────────────────────────────
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

            // ── Branch name input ────────────────────────────────────────
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

            // ── Checkbox row ─────────────────────────────────────────────
            let main_branch_indicator = if state.main_branch_mode { "\u{2611}" } else { "\u{2610}" };
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

            let auto_suggest_indicator = if state.auto_suggest_branch { "\u{2611}" } else { "\u{2610}" };
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

            let mut content = column![
                title,
                subtitle,
                step_indicator,
                tab_bar,
                workspace_section,
                ai_tool_section,
            ]
            .spacing(12);

            if let Some(mm_section) = model_mode_section {
                content = content.push(mm_section);
            }

            content
                .push(prompt_section)
                .push(branch_section)
                .push(checkbox_row)
                .push(footer)
        }
        QuickClaudeTab::ResumeSession => {
            // ── Resume session content ───────────────────────────────────
            let mut session_list = column![].spacing(4);

            if state.sessions.is_empty() {
                session_list = session_list.push(
                    container(
                        text("No recent Claude sessions found").size(13).color(text_secondary)
                    )
                    .padding(20)
                    .width(Length::Fill)
                    .center_x(Length::Fill)
                );
            } else {
                for (i, session) in state.sessions.iter().enumerate() {
                    let is_selected = state.selected_session == Some(i);
                    let idx = i;

                    let msg_preview = if session.first_message.len() > 80 {
                        format!("{}...", &session.first_message[..77])
                    } else {
                        session.first_message.clone()
                    };

                    let session_id_display = format!(
                        "ID: {}...{}",
                        &session.session_id[..4.min(session.session_id.len())],
                        &session.session_id[session.session_id.len().saturating_sub(4)..]
                    );

                    let timestamp_display = session.timestamp.clone();
                    let model_display = format!("Model: {}", session.model);
                    let session_item = button(
                        column![
                            row![
                                text(msg_preview).size(12).color(if is_selected { text_active } else { text_primary }),
                                Space::new().width(Length::Fill),
                                text(timestamp_display).size(10).color(text_secondary),
                            ].align_y(iced::Alignment::Center),
                            row![
                                text(model_display).size(10).color(text_secondary),
                                Space::new().width(8),
                                text(session_id_display).size(10).color(text_secondary),
                            ].spacing(8),
                        ].spacing(2)
                    )
                    .on_press(on_session_selected(idx))
                    .padding(Padding::from([8, 12]))
                    .width(Length::Fill)
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(if is_selected {
                            tint(accent, 0.12)
                        } else {
                            Color::TRANSPARENT
                        })),
                        border: Border {
                            color: if is_selected { accent } else { border_color },
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..button::Style::default()
                    });

                    session_list = session_list.push(session_item);
                }
            }

            let resume_content = column![
                scrollable(session_list).height(Length::Fixed(300.0)),
            ]
            .spacing(8);

            let action_btn = if state.selected_session.is_some() {
                button(text("Resume").size(13).color(text_active))
                    .on_press(on_resume)
                    .padding(Padding::from([6, 16]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(accent)),
                        text_color: Color::WHITE,
                        border: Border {
                            radius: 6.0.into(),
                            ..Border::default()
                        },
                        ..button::Style::default()
                    })
            } else {
                button(text("Resume").size(13).color(text_secondary))
                    .padding(Padding::from([6, 16]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(tint(accent, 0.3))),
                        text_color: text_secondary,
                        border: Border {
                            radius: 6.0.into(),
                            ..Border::default()
                        },
                        ..button::Style::default()
                    })
            };

            let footer = row![
                cancel_btn,
                Space::new().width(Length::Fill),
                action_btn,
            ]
            .spacing(8);

            column![
                title,
                subtitle,
                tab_bar,
                resume_content,
                footer,
            ]
            .spacing(12)
        }
    };

    // ── Compose dialog ───────────────────────────────────────────────────
    let dialog = container(dialog_content)
        .padding(Padding::from([20, 24]))
        .width(Length::Fixed(600.0))
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

    if let Some(folder) = workspace_folder {
        let project_skills_dir = std::path::Path::new(folder).join(".claude").join("skills");
        if project_skills_dir.exists() {
            collect_skills_from_dir(&project_skills_dir, SkillScope::Project, &mut skills);
        }
    }

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
                path.parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            } else {
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

/// Discover recent Claude Code sessions from JSONL files.
pub fn discover_sessions(workspace_folder: Option<&str>) -> Vec<ClaudeSession> {
    let home = match std::env::var("USERPROFILE").ok().or_else(|| std::env::var("HOME").ok()) {
        Some(h) => h,
        None => return Vec::new(),
    };

    let project_key = match workspace_folder {
        Some(folder) => folder.replace(['\\', ':', '/'], "-"),
        None => return Vec::new(),
    };

    let sessions_dir = std::path::Path::new(&home)
        .join(".claude")
        .join("projects")
        .join(&project_key);

    if !sessions_dir.exists() {
        return Vec::new();
    }

    let mut sessions: Vec<(ClaudeSession, std::time::SystemTime)> = Vec::new();

    let Ok(entries) = std::fs::read_dir(&sessions_dir) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "jsonl") {
            continue;
        }

        let session_id = path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let mtime = entry.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let (first_message, model) = parse_session_jsonl(&path);

        let timestamp = format_relative_time(mtime);

        sessions.push((ClaudeSession {
            session_id,
            first_message,
            model,
            timestamp,
            branch: String::new(),
            file_path: path.to_string_lossy().to_string(),
        }, mtime));
    }

    sessions.sort_by(|a, b| b.1.cmp(&a.1));
    sessions.into_iter().take(20).map(|(s, _)| s).collect()
}

fn parse_session_jsonl(path: &std::path::Path) -> (String, String) {
    use std::io::BufRead;

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (String::new(), String::new()),
    };

    let reader = std::io::BufReader::new(file);
    let mut first_user_message = String::new();
    let mut model = String::new();

    for line in reader.lines().take(50) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if first_user_message.is_empty() {
            if value.get("type").and_then(|t| t.as_str()) == Some("human") {
                if let Some(msg) = value.get("message").and_then(|m| m.get("content")) {
                    if let Some(text) = msg.as_str() {
                        first_user_message = text.to_string();
                    } else if let Some(arr) = msg.as_array() {
                        for item in arr {
                            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    first_user_message = text.to_string();
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if model.is_empty() {
            if value.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                if let Some(m) = value.get("message")
                    .and_then(|m| m.get("model"))
                    .and_then(|m| m.as_str())
                {
                    model = m.to_string();
                }
            }
        }

        if !first_user_message.is_empty() && !model.is_empty() {
            break;
        }
    }

    (first_user_message, model)
}

fn format_relative_time(time: std::time::SystemTime) -> String {
    let elapsed = time.elapsed().unwrap_or_default();
    let secs = elapsed.as_secs();

    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 604800 {
        format!("{}d ago", secs / 86400)
    } else {
        format!("{}w ago", secs / 604800)
    }
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
        assert_eq!(state.active_tab, QuickClaudeTab::NewPrompt);
        assert!(state.sessions.is_empty());
        assert!(state.selected_session.is_none());
        assert!(state.skills.is_empty());
        assert!(!state.skill_autocomplete_open);
        assert!(state.skill_autocomplete_filter.is_empty());
        assert_eq!(state.skill_autocomplete_selected, 0);
        assert_eq!(state.selected_model, "sonnet");
        assert!(!state.model_dropdown_open);
        assert_eq!(state.selected_mode, "default");
        assert!(!state.mode_dropdown_open);
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
            ModelSelected(String),
            ModelDropdown,
            ModeSelected(String),
            ModeDropdown,
            Launch,
            Voice,
            Cancel,
            SkillSelected(usize),
            SkillNav(i32),
            SkillDismiss,
            TabSelected(QuickClaudeTab),
            SessionSelected(usize),
            Resume,
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
            Msg::ModelSelected,
            Msg::ModelDropdown,
            Msg::ModeSelected,
            Msg::ModeDropdown,
            Msg::Launch,
            Msg::Voice,
            Msg::Cancel,
            Msg::SkillSelected,
            Msg::SkillNav,
            Msg::SkillDismiss,
            Msg::TabSelected,
            Msg::SessionSelected,
            Msg::Resume,
        );
    }

    #[test]
    fn view_resume_tab_does_not_panic() {
        let mut state = QuickClaudeDialogState::new(Some("ws-1".into()), sample_workspaces());
        state.active_tab = QuickClaudeTab::ResumeSession;
        state.sessions = vec![
            ClaudeSession {
                session_id: "abc12345def67890".to_string(),
                first_message: "Help me fix a bug in the terminal".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: "3h ago".to_string(),
                branch: String::new(),
                file_path: "/tmp/test.jsonl".to_string(),
            },
        ];
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
            ModelSelected(String),
            ModelDropdown,
            ModeSelected(String),
            ModeDropdown,
            Launch,
            Voice,
            Cancel,
            SkillSelected(usize),
            SkillNav(i32),
            SkillDismiss,
            TabSelected(QuickClaudeTab),
            SessionSelected(usize),
            Resume,
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
            Msg::ModelSelected,
            Msg::ModelDropdown,
            Msg::ModeSelected,
            Msg::ModeDropdown,
            Msg::Launch,
            Msg::Voice,
            Msg::Cancel,
            Msg::SkillSelected,
            Msg::SkillNav,
            Msg::SkillDismiss,
            Msg::TabSelected,
            Msg::SessionSelected,
            Msg::Resume,
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
        let skills = discover_skills(Some("/nonexistent/path/for/test"));
        for skill in &skills {
            assert!(
                !matches!(skill.scope, SkillScope::Project),
                "should not find project skills for non-existent workspace"
            );
        }
    }

    #[test]
    fn discover_sessions_no_workspace_returns_empty() {
        let result = discover_sessions(None);
        assert!(result.is_empty());
    }

    #[test]
    fn discover_sessions_nonexistent_path_returns_empty() {
        let result = discover_sessions(Some("/nonexistent/path/to/project"));
        assert!(result.is_empty());
    }

    #[test]
    fn format_relative_time_just_now() {
        let time = std::time::SystemTime::now();
        let result = format_relative_time(time);
        assert_eq!(result, "just now");
    }

    #[test]
    fn format_relative_time_minutes() {
        let time = std::time::SystemTime::now() - std::time::Duration::from_secs(300);
        let result = format_relative_time(time);
        assert_eq!(result, "5m ago");
    }

    #[test]
    fn format_relative_time_hours() {
        let time = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        let result = format_relative_time(time);
        assert_eq!(result, "2h ago");
    }

    #[test]
    fn format_relative_time_days() {
        let time = std::time::SystemTime::now() - std::time::Duration::from_secs(172800);
        let result = format_relative_time(time);
        assert_eq!(result, "2d ago");
    }

    #[test]
    fn format_relative_time_weeks() {
        let time = std::time::SystemTime::now() - std::time::Duration::from_secs(1209600);
        let result = format_relative_time(time);
        assert_eq!(result, "2w ago");
    }
}
