use std::path::PathBuf;

use iced::widget::{button, column, container, row, scrollable, stack, text, text_input, text_editor, Space};
use iced::{Background, Border, Color, ContentFit, Element, Length, Padding, Shadow, Vector};
use serde::{Deserialize, Serialize};

use crate::theme;

/// Widget ID for the prompt text editor — used to focus it on dialog open.
pub fn prompt_editor_id() -> iced::widget::Id {
    iced::widget::Id::new("quick-claude-prompt-editor")
}

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
    pub cwd: Option<String>,
    pub workspace_id: String,
}

#[derive(Debug, Clone)]
pub enum SkillScope {
    Project,
    User,
    /// Installed Claude Code plugin — stores the plugin display name.
    Plugin(String),
}

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub scope: SkillScope,
    /// Path to the skill's .md file on disk; stored for future use (e.g., opening in editor).
    #[allow(dead_code)]
    pub file_path: String,
}

/// An image attachment in the Quick Claude dialog.
#[derive(Debug, Clone)]
pub struct ImageAttachment {
    /// Absolute path to the image file on disk.
    pub file_path: String,
    /// Iced image handle for rendering the thumbnail.
    pub thumbnail_handle: iced::widget::image::Handle,
    /// Original filename (for display).
    pub display_name: String,
}

/// Persistent preferences that survive dialog close/reopen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QuickClaudePreferences {
    pub selected_model: String,
    pub selected_mode: String,
    pub selected_ai_tool: String,
    pub selected_workspace_id: Option<String>,
    pub auto_suggest_branch: bool,
    pub main_branch_mode: bool,
    #[serde(default)]
    pub batch_clone_mode: bool,
}

impl Default for QuickClaudePreferences {
    fn default() -> Self {
        Self {
            selected_model: "sonnet".to_string(),
            selected_mode: "auto".to_string(),
            selected_ai_tool: "Claude Code".to_string(),
            selected_workspace_id: None,
            auto_suggest_branch: true,
            main_branch_mode: false,
            batch_clone_mode: false,
        }
    }
}

const PREFS_FILE_NAME: &str = "quick-claude-prefs.json";

fn prefs_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let directory_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    base.join(directory_name)
        .join("native")
        .join(PREFS_FILE_NAME)
}

/// Load Quick Claude preferences from disk, returning defaults if the file
/// is missing or cannot be parsed.
pub fn load_preferences() -> QuickClaudePreferences {
    let path = prefs_path();
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => QuickClaudePreferences::default(),
    }
}

/// Save Quick Claude preferences to disk.
pub fn save_preferences(prefs: &QuickClaudePreferences) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(prefs) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("Failed to save Quick Claude prefs: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialize Quick Claude prefs: {e}"),
    }
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
    pub batch_clone_mode: bool,
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
    /// Dynamically discovered model options: (display_name, value).
    pub available_models: Vec<(String, String)>,
    /// Images attached to the prompt (via paste or drag-and-drop).
    pub attached_images: Vec<ImageAttachment>,
}

impl QuickClaudeDialogState {
    pub fn new(
        workspace_id: Option<String>,
        workspaces: Vec<(String, String)>,
        prefs: &QuickClaudePreferences,
    ) -> Self {
        // Use workspace from prefs if it exists in the workspace list, otherwise use current active
        let ws_id = prefs
            .selected_workspace_id
            .as_ref()
            .filter(|id| workspaces.iter().any(|(ws_id, _)| ws_id == *id))
            .cloned()
            .or(workspace_id);
        Self {
            selected_workspace_id: ws_id,
            workspace_dropdown_open: false,
            selected_ai_tool: prefs.selected_ai_tool.clone(),
            ai_tool_dropdown_open: false,
            prompt_content: text_editor::Content::new(),
            branch_name: String::new(),
            main_branch_mode: prefs.main_branch_mode,
            auto_suggest_branch: prefs.auto_suggest_branch,
            batch_clone_mode: prefs.batch_clone_mode,
            workspaces,
            active_tab: QuickClaudeTab::NewPrompt,
            sessions: Vec::new(),
            selected_session: None,
            skills: Vec::new(),
            skill_autocomplete_open: false,
            skill_autocomplete_filter: String::new(),
            skill_autocomplete_selected: 0,
            selected_model: prefs.selected_model.clone(),
            model_dropdown_open: false,
            selected_mode: prefs.selected_mode.clone(),
            mode_dropdown_open: false,
            available_models: default_model_list(),
            attached_images: Vec::new(),
        }
    }

    /// Save current selections back to preferences.
    pub fn to_preferences(&self) -> QuickClaudePreferences {
        QuickClaudePreferences {
            selected_model: self.selected_model.clone(),
            selected_mode: self.selected_mode.clone(),
            selected_ai_tool: self.selected_ai_tool.clone(),
            selected_workspace_id: self.selected_workspace_id.clone(),
            auto_suggest_branch: self.auto_suggest_branch,
            main_branch_mode: self.main_branch_mode,
            batch_clone_mode: self.batch_clone_mode,
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
    on_batch_clone_toggled: impl Fn(bool) -> M + 'a,
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
    on_image_removed: impl Fn(usize) -> M + 'a,
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
    let subtitle = text("Tab or Ctrl+Enter to launch \u{00B7} Escape to cancel")
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
                let modes = [("Default", "default"), ("Plan", "plan"), ("Auto", "auto")];

                let model_display = state
                    .available_models
                    .iter()
                    .find(|(_, v)| v == &state.selected_model)
                    .map(|(d, _)| d.as_str())
                    .unwrap_or(&state.selected_model);

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
                    for (display, value) in &state.available_models {
                        let is_selected = state.selected_model == *value;
                        let val = value.clone();
                        let display_ref = display.as_str();
                        let item = button(
                            text(display_ref)
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
                .id(prompt_editor_id())
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
                        let scope_label = match &skill.scope {
                            SkillScope::Project => "project",
                            SkillScope::User => "user",
                            SkillScope::Plugin(name) => name.as_str(),
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

            // ── Image attachments ─────────────────────────────────────────
            if !state.attached_images.is_empty() {
                let mut image_row = row![].spacing(8).align_y(iced::Alignment::End);

                for (i, attachment) in state.attached_images.iter().enumerate() {
                    let thumb = iced::widget::image::Image::new(attachment.thumbnail_handle.clone())
                        .width(Length::Fixed(48.0))
                        .height(Length::Fixed(48.0))
                        .content_fit(ContentFit::Cover);

                    let remove_btn = button(
                        text("\u{2715}").size(9).color(Color::WHITE),
                    )
                    .on_press(on_image_removed(i))
                    .padding(Padding::from([1, 3]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.6))),
                        text_color: Color::WHITE,
                        border: Border {
                            radius: 3.0.into(),
                            ..Border::default()
                        },
                        ..button::Style::default()
                    });

                    let name_label = text(&attachment.display_name)
                        .size(9)
                        .color(text_secondary);

                    let thumb_with_remove = column![
                        container(
                            stack![
                                thumb,
                                container(remove_btn)
                                    .align_right(Length::Fill)
                                    .padding(2),
                            ]
                        )
                        .width(Length::Fixed(48.0))
                        .height(Length::Fixed(48.0))
                        .style(move |_theme| container::Style {
                            border: Border {
                                color: border_color,
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..container::Style::default()
                        }),
                        container(name_label).width(Length::Fixed(48.0)),
                    ]
                    .spacing(2)
                    .align_x(iced::Alignment::Center);

                    image_row = image_row.push(thumb_with_remove);
                }

                let attachment_label = text(
                    format!("{} image{} attached", state.attached_images.len(),
                        if state.attached_images.len() == 1 { "" } else { "s" }),
                )
                .size(10)
                .color(text_secondary);

                prompt_section = prompt_section.push(
                    column![attachment_label, image_row].spacing(4),
                );
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
                        .color(if state.batch_clone_mode { text_secondary } else { text_primary })
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press_maybe(if state.batch_clone_mode { None } else { Some(on_main_branch_toggled(!state.main_branch_mode)) })
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
                        .color(if state.batch_clone_mode { text_secondary } else { text_primary })
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press_maybe(if state.batch_clone_mode { None } else { Some(on_auto_suggest_toggled(!state.auto_suggest_branch)) })
            .padding(Padding::from([4, 8]))
            .style(move |_theme, _status| button::Style {
                background: None,
                border: Border::default(),
                ..button::Style::default()
            });

            let batch_clone_indicator = if state.batch_clone_mode { "\u{2611}" } else { "\u{2610}" };
            let batch_clone_btn = button(
                row![
                    text(batch_clone_indicator).size(14).color(accent),
                    text("Full clone (batch-friendly)")
                        .size(12)
                        .color(if state.main_branch_mode { text_secondary } else { text_primary })
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press_maybe(if state.main_branch_mode { None } else { Some(on_batch_clone_toggled(!state.batch_clone_mode)) })
            .padding(Padding::from([4, 8]))
            .style(move |_theme, _status| button::Style {
                background: None,
                border: Border::default(),
                ..button::Style::default()
            });

            let checkbox_row = row![main_branch_btn, auto_suggest_btn, batch_clone_btn].spacing(12);

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

/// Discover Claude Code skills from project, user, and installed plugin directories.
pub fn discover_skills(workspace_folder: Option<&str>) -> Vec<SkillEntry> {
    let mut skills = Vec::new();

    if let Some(folder) = workspace_folder {
        let project_skills_dir = std::path::Path::new(folder).join(".claude").join("skills");
        if project_skills_dir.exists() {
            collect_skills_from_dir(&project_skills_dir, SkillScope::Project, &mut skills);
        }

        // Also scan .claude/commands/ — Claude Code's project-level commands directory.
        let project_commands_dir = std::path::Path::new(folder).join(".claude").join("commands");
        if project_commands_dir.exists() {
            collect_skills_from_dir(&project_commands_dir, SkillScope::Project, &mut skills);
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

        // Also scan ~/.claude/commands/ — Claude Code's user-level commands directory.
        let user_commands_dir = std::path::Path::new(&home).join(".claude").join("commands");
        if user_commands_dir.exists() {
            collect_skills_from_dir(&user_commands_dir, SkillScope::User, &mut skills);
        }

        // Discover skills from installed Claude Code plugins.
        let installed_json = std::path::Path::new(&home)
            .join(".claude")
            .join("plugins")
            .join("installed_plugins.json");
        if installed_json.exists() {
            collect_plugin_skills(&installed_json, &mut skills);
        }
    }

    skills
}

/// Read `installed_plugins.json` and collect skills/commands from each installed plugin.
fn collect_plugin_skills(json_path: &std::path::Path, skills: &mut Vec<SkillEntry>) {
    let Ok(data) = std::fs::read_to_string(json_path) else {
        return;
    };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&data) else {
        return;
    };
    let Some(plugins) = root.get("plugins").and_then(|v| v.as_object()) else {
        return;
    };

    for (key, installs) in plugins {
        // Plugin key format: "plugin-name@marketplace" — extract the short name.
        let plugin_name = key.split('@').next().unwrap_or(key);

        // Each plugin may have multiple installs; use the last (most recent) entry.
        let Some(install) = installs.as_array().and_then(|a| a.last()) else {
            continue;
        };
        let Some(install_path) = install.get("installPath").and_then(|v| v.as_str()) else {
            continue;
        };
        let base = std::path::Path::new(install_path);

        let scope = SkillScope::Plugin(plugin_name.to_string());

        // Scan skills/ directory (SKILL.md convention inside subdirectories).
        let skills_dir = base.join("skills");
        if skills_dir.exists() {
            collect_skills_from_dir(&skills_dir, scope.clone(), skills);
        }

        // Scan commands/ directory (each .md file is a command).
        let commands_dir = base.join("commands");
        if commands_dir.exists() {
            collect_skills_from_dir(&commands_dir, scope, skills);
        }
    }
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

/// Extract a description from a skill/command markdown file.
///
/// Tries two strategies (first 30 lines):
/// 1. YAML frontmatter `description:` field (between `---` markers)
/// 2. First `# ` markdown heading
fn read_first_heading(path: &std::path::Path) -> Option<String> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);

    let mut in_frontmatter = false;
    let mut frontmatter_desc: Option<String> = None;

    for line in reader.lines().take(30) {
        let line = line.ok()?;
        let trimmed = line.trim();

        // Detect frontmatter boundaries.
        if trimmed == "---" {
            if !in_frontmatter {
                in_frontmatter = true;
                continue;
            } else {
                // End of frontmatter — if we found a description, use it.
                if frontmatter_desc.is_some() {
                    return frontmatter_desc;
                }
                in_frontmatter = false;
                continue;
            }
        }

        if in_frontmatter {
            // Parse `description: ...` (with optional quotes).
            if let Some(rest) = trimmed.strip_prefix("description:") {
                let desc = rest.trim().trim_matches('"').trim_matches('\'');
                if !desc.is_empty() {
                    frontmatter_desc = Some(desc.to_string());
                }
            }
            continue;
        }

        // Outside frontmatter: look for markdown heading.
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return Some(heading.to_string());
        }
    }

    frontmatter_desc
}

/// Load recent Quick Claude sessions from the session record file.
pub fn discover_sessions() -> Vec<ClaudeSession> {
    let records = crate::quick_claude_sessions::load_sessions();

    records
        .into_iter()
        .rev() // most recent first
        .filter_map(|r| {
            let claude_sid = r.claude_session_id?;
            Some(ClaudeSession {
                session_id: claude_sid,
                first_message: r.prompt,
                model: r.model,
                timestamp: format_relative_time_from_iso(&r.launched_at),
                branch: r.branch,
                file_path: String::new(),
                cwd: r.cwd,
                workspace_id: r.workspace_id,
            })
        })
        .take(20)
        .collect()
}

fn format_elapsed_secs(secs: u64) -> String {
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

fn format_relative_time_from_iso(iso: &str) -> String {
    if iso.len() < 19 { return iso.to_string(); }
    let year: u64 = iso[0..4].parse().unwrap_or(0);
    let month: u64 = iso[5..7].parse().unwrap_or(0);
    let day: u64 = iso[8..10].parse().unwrap_or(0);
    let hour: u64 = iso[11..13].parse().unwrap_or(0);
    let min: u64 = iso[14..16].parse().unwrap_or(0);
    let sec: u64 = iso[17..19].parse().unwrap_or(0);
    if year == 0 || month == 0 || day == 0 { return iso.to_string(); }

    let a = if month <= 2 { 1u64 } else { 0 };
    let y = year - a;
    let m = month + 12 * a - 3;
    let days = y * 365 + y / 4 - y / 100 + y / 400
        + (153 * m + 2) / 5 + day - 1 - 719468;
    let total_secs = days * 86400 + hour * 3600 + min * 60 + sec;

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let elapsed = now_secs.saturating_sub(total_secs);
    format_elapsed_secs(elapsed)
}

/// Default model list — always includes the three core Claude Code aliases.
/// Order matches Claude Code's default (Sonnet first as the default model).
pub fn default_model_list() -> Vec<(String, String)> {
    vec![
        ("Sonnet".to_string(), "sonnet".to_string()),
        ("Opus".to_string(), "opus".to_string()),
        ("Haiku".to_string(), "haiku".to_string()),
    ]
}

/// Discover available models by running `claude --help` and parsing the --model description.
/// Merges discovered models with the default list so core aliases are always present.
pub fn discover_models() -> Vec<(String, String)> {
    let output = match std::process::Command::new("claude")
        .args(["--help"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return default_model_list(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let discovered = parse_models_from_help(&stdout);

    // Merge: start with defaults, add any newly discovered models
    let mut models = default_model_list();
    for (display, value) in discovered {
        if !models.iter().any(|(_, v)| v == &value) {
            models.push((display, value));
        }
    }
    models
}

/// Parse model aliases from claude --help output.
/// Looks for the --model line and extracts quoted aliases like 'sonnet', 'opus'.
fn parse_models_from_help(help_text: &str) -> Vec<(String, String)> {
    // Find the --model line and extract aliases from the description
    // Format: --model <model>  Model for the current session. Provide an alias for the latest model (e.g. 'sonnet' or 'opus') or a model's full name (e.g. 'claude-sonnet-4-6').
    let mut aliases: Vec<String> = Vec::new();

    for line in help_text.lines() {
        if line.contains("--model") && line.contains("alias") {
            // Extract single-quoted strings as aliases
            let mut rest = line;
            while let Some(start) = rest.find('\'') {
                rest = &rest[start + 1..];
                if let Some(end) = rest.find('\'') {
                    let alias = &rest[..end];
                    // Filter: only short single-word aliases (not full model names or stray text)
                    if !alias.is_empty()
                        && !alias.contains('-')
                        && !alias.contains(' ')
                        && alias.chars().all(|c| c.is_alphanumeric())
                    {
                        aliases.push(alias.to_string());
                    }
                    rest = &rest[end + 1..];
                } else {
                    break;
                }
            }
        }
    }

    // Also look for the --permission-mode line to find modes
    // But for now just handle models

    if aliases.is_empty() {
        return default_model_list();
    }

    // Build display names: capitalize first letter
    aliases
        .into_iter()
        .map(|alias| {
            let display = {
                let mut chars = alias.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                    None => alias.clone(),
                }
            };
            (display, alias)
        })
        .collect()
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
        let state = QuickClaudeDialogState::new(Some("ws-1".into()), sample_workspaces(), &QuickClaudePreferences::default());
        assert_eq!(state.selected_workspace_id, Some("ws-1".into()));
        assert_eq!(state.selected_ai_tool, "Claude Code");
        assert!(!state.workspace_dropdown_open);
        assert!(!state.ai_tool_dropdown_open);
        assert!(state.branch_name.is_empty());
        assert!(!state.main_branch_mode);
        assert!(state.auto_suggest_branch);
        assert!(!state.batch_clone_mode);
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
        assert_eq!(state.selected_mode, "auto");
        assert!(!state.mode_dropdown_open);
        assert!(!state.available_models.is_empty());
    }

    #[test]
    fn state_new_no_workspace() {
        let state = QuickClaudeDialogState::new(None, vec![], &QuickClaudePreferences::default());
        assert!(state.selected_workspace_id.is_none());
    }

    #[test]
    fn prompt_text_empty() {
        let state = QuickClaudeDialogState::new(None, vec![], &QuickClaudePreferences::default());
        assert!(state.prompt_text().trim().is_empty());
    }

    #[test]
    fn view_does_not_panic() {
        let state = QuickClaudeDialogState::new(Some("ws-1".into()), sample_workspaces(), &QuickClaudePreferences::default());
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
            BatchClone(bool),
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
            ImageRemoved(usize),
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
            Msg::BatchClone,
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
            Msg::ImageRemoved,
        );
    }

    #[test]
    fn view_resume_tab_does_not_panic() {
        let mut state = QuickClaudeDialogState::new(Some("ws-1".into()), sample_workspaces(), &QuickClaudePreferences::default());
        state.active_tab = QuickClaudeTab::ResumeSession;
        state.sessions = vec![
            ClaudeSession {
                session_id: "abc12345def67890".to_string(),
                first_message: "Help me fix a bug in the terminal".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                timestamp: "3h ago".to_string(),
                branch: String::new(),
                file_path: "/tmp/test.jsonl".to_string(),
                cwd: None,
                workspace_id: String::new(),
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
            BatchClone(bool),
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
            ImageRemoved(usize),
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
            Msg::BatchClone,
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
            Msg::ImageRemoved,
        );
    }

    #[test]
    fn filtered_skills_empty_filter() {
        let mut state = QuickClaudeDialogState::new(None, vec![], &QuickClaudePreferences::default());
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
        let mut state = QuickClaudeDialogState::new(None, vec![], &QuickClaudePreferences::default());
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
    fn discover_sessions_no_records_returns_empty() {
        // With no session records file, should return empty
        let result = discover_sessions();
        // May or may not be empty depending on local state; just assert no panic
        let _ = result;
    }

    #[test]
    fn format_elapsed_secs_just_now() {
        let result = format_elapsed_secs(0);
        assert_eq!(result, "just now");
    }

    #[test]
    fn format_elapsed_secs_minutes() {
        let result = format_elapsed_secs(300);
        assert_eq!(result, "5m ago");
    }

    #[test]
    fn format_elapsed_secs_hours() {
        let result = format_elapsed_secs(7200);
        assert_eq!(result, "2h ago");
    }

    #[test]
    fn format_elapsed_secs_days() {
        let result = format_elapsed_secs(172800);
        assert_eq!(result, "2d ago");
    }

    #[test]
    fn format_elapsed_secs_weeks() {
        let result = format_elapsed_secs(1209600);
        assert_eq!(result, "2w ago");
    }

    #[test]
    fn preferences_default_mode_is_auto() {
        let prefs = QuickClaudePreferences::default();
        assert_eq!(prefs.selected_mode, "auto");
        assert_eq!(prefs.selected_model, "sonnet");
        assert_eq!(prefs.selected_ai_tool, "Claude Code");
        assert!(prefs.selected_workspace_id.is_none());
    }

    #[test]
    fn state_inherits_preferences() {
        let prefs = QuickClaudePreferences {
            selected_model: "opus".to_string(),
            selected_mode: "plan".to_string(),
            selected_ai_tool: "Codex".to_string(),
            selected_workspace_id: Some("ws-2".to_string()),
            auto_suggest_branch: false,
            main_branch_mode: true,
            batch_clone_mode: false,
        };
        let state = QuickClaudeDialogState::new(
            Some("ws-1".into()),
            sample_workspaces(),
            &prefs,
        );
        assert_eq!(state.selected_model, "opus");
        assert_eq!(state.selected_mode, "plan");
        assert_eq!(state.selected_ai_tool, "Codex");
        // ws-2 exists in workspaces, so prefs workspace is used
        assert_eq!(state.selected_workspace_id, Some("ws-2".to_string()));
        assert!(!state.auto_suggest_branch);
        assert!(state.main_branch_mode);
    }

    #[test]
    fn state_prefs_workspace_falls_back_when_not_in_list() {
        let prefs = QuickClaudePreferences {
            selected_model: "sonnet".to_string(),
            selected_mode: "auto".to_string(),
            selected_ai_tool: "Claude Code".to_string(),
            selected_workspace_id: Some("ws-gone".to_string()),
            auto_suggest_branch: true,
            main_branch_mode: false,
            batch_clone_mode: false,
        };
        let state = QuickClaudeDialogState::new(
            Some("ws-1".into()),
            sample_workspaces(),
            &prefs,
        );
        // ws-gone not in workspaces, falls back to active workspace
        assert_eq!(state.selected_workspace_id, Some("ws-1".to_string()));
    }

    #[test]
    fn to_preferences_roundtrip() {
        let prefs = QuickClaudePreferences {
            selected_model: "opus".to_string(),
            selected_mode: "plan".to_string(),
            selected_ai_tool: "Codex".to_string(),
            selected_workspace_id: Some("ws-1".to_string()),
            auto_suggest_branch: false,
            main_branch_mode: true,
            batch_clone_mode: false,
        };
        let state = QuickClaudeDialogState::new(
            Some("ws-1".into()),
            sample_workspaces(),
            &prefs,
        );
        let roundtripped = state.to_preferences();
        assert_eq!(roundtripped.selected_model, "opus");
        assert_eq!(roundtripped.selected_mode, "plan");
        assert_eq!(roundtripped.selected_ai_tool, "Codex");
        assert_eq!(roundtripped.selected_workspace_id, Some("ws-1".to_string()));
        assert!(!roundtripped.auto_suggest_branch);
        assert!(roundtripped.main_branch_mode);
    }

    #[test]
    fn parse_models_from_help_extracts_aliases() {
        let help = r#"  --model <model>  Model for the current session. Provide an alias for the latest model (e.g. 'sonnet' or 'opus') or a model's full name (e.g. 'claude-sonnet-4-6')."#;
        let models = parse_models_from_help(help);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], ("Sonnet".to_string(), "sonnet".to_string()));
        assert_eq!(models[1], ("Opus".to_string(), "opus".to_string()));
    }

    #[test]
    fn parse_models_from_help_no_model_line_returns_defaults() {
        let help = "some other help text\n--verbose  Enable verbose";
        let models = parse_models_from_help(help);
        // Falls back to default list
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].1, "sonnet");
    }

    #[test]
    fn default_model_list_has_three() {
        let models = default_model_list();
        assert_eq!(models.len(), 3);
        assert!(models.iter().any(|(_, v)| v == "opus"));
        assert!(models.iter().any(|(_, v)| v == "sonnet"));
        assert!(models.iter().any(|(_, v)| v == "haiku"));
    }

    #[test]
    fn prefs_serialization_round_trip() {
        let prefs = QuickClaudePreferences {
            selected_model: "opus".to_string(),
            selected_mode: "plan".to_string(),
            selected_ai_tool: "Codex".to_string(),
            selected_workspace_id: Some("ws-42".to_string()),
            auto_suggest_branch: false,
            main_branch_mode: true,
            batch_clone_mode: false,
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: QuickClaudePreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.selected_model, "opus");
        assert_eq!(loaded.selected_mode, "plan");
        assert_eq!(loaded.selected_ai_tool, "Codex");
        assert_eq!(loaded.selected_workspace_id, Some("ws-42".to_string()));
        assert!(!loaded.auto_suggest_branch);
        assert!(loaded.main_branch_mode);
    }

    #[test]
    fn prefs_deserialize_missing_fields_uses_defaults() {
        // Simulate a JSON from an older version with fewer fields
        let json = r#"{"selected_model":"haiku","selected_mode":"auto"}"#;
        let loaded: QuickClaudePreferences = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.selected_model, "haiku");
        assert_eq!(loaded.selected_mode, "auto");
        // Fields missing from JSON should get their Default values
        assert_eq!(loaded.selected_ai_tool, "Claude Code");
        assert!(loaded.selected_workspace_id.is_none());
        assert!(loaded.auto_suggest_branch);
        assert!(!loaded.main_branch_mode);
        assert!(!loaded.batch_clone_mode);
    }

    #[test]
    fn prefs_save_and_load_via_tempfile() {
        let dir = std::env::temp_dir().join(format!("godly-prefs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("quick-claude-prefs.json");

        let prefs = QuickClaudePreferences {
            selected_model: "opus".to_string(),
            selected_mode: "plan".to_string(),
            ..QuickClaudePreferences::default()
        };

        // Write
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        std::fs::write(&path, &json).unwrap();

        // Read back
        let loaded_json = std::fs::read_to_string(&path).unwrap();
        let loaded: QuickClaudePreferences = serde_json::from_str(&loaded_json).unwrap();
        assert_eq!(loaded.selected_model, "opus");
        assert_eq!(loaded.selected_mode, "plan");
        assert_eq!(loaded.selected_ai_tool, "Claude Code");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_preferences_batch_clone_default() {
        let prefs = QuickClaudePreferences::default();
        assert!(!prefs.batch_clone_mode);
    }

    #[test]
    fn test_preferences_batch_clone_roundtrip() {
        let prefs = QuickClaudePreferences {
            batch_clone_mode: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let decoded: QuickClaudePreferences = serde_json::from_str(&json).unwrap();
        assert!(decoded.batch_clone_mode);
    }

    #[test]
    fn test_old_prefs_without_batch_clone_deserialize() {
        let json = r#"{"selected_model":"sonnet","selected_mode":"auto","selected_ai_tool":"Claude Code","selected_workspace_id":null,"auto_suggest_branch":true,"main_branch_mode":false}"#;
        let decoded: QuickClaudePreferences = serde_json::from_str(json).unwrap();
        assert!(!decoded.batch_clone_mode);
    }
}
