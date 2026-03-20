use iced::widget::{button, column, container, row, scrollable, text, text_input, text_editor, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
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
            selected_model: "sonnet".to_string(),
            model_dropdown_open: false,
            selected_mode: "default".to_string(),
            mode_dropdown_open: false,
        }
    }

    pub fn prompt_text(&self) -> String {
        self.prompt_content.text()
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

    // ── Model & Mode dropdowns (Claude Code only) ──────────────────────
    let model_mode_section: Option<Element<'a, M>> = if state.selected_ai_tool == "Claude Code" {
        let models = [("Opus", "opus"), ("Sonnet", "sonnet"), ("Haiku", "haiku")];
        let modes = [("Default", "default"), ("Plan", "plan"), ("Auto", "auto")];

        // Model dropdown
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

        // Mode dropdown
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

    let prompt_section = column![
        text("Prompt").size(11).color(text_secondary),
        editor_container,
    ]
    .spacing(4);

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
    let mut dialog_content = column![
        title,
        subtitle,
        step_indicator,
        workspace_section,
        ai_tool_section,
    ]
    .spacing(12);

    if let Some(mm_section) = model_mode_section {
        dialog_content = dialog_content.push(mm_section);
    }

    dialog_content = dialog_content
        .push(prompt_section)
        .push(branch_section)
        .push(checkbox_row)
        .push(footer);

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
        );
    }
}
