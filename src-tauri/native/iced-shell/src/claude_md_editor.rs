use iced::widget::{
    button, column, container, row, rule, scrollable, text, text_editor, Space,
};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme::{
    ACCENT, ACCENT_HOVER, BACKDROP, BG_PRIMARY, BG_SECONDARY, BG_TERTIARY, BORDER, TEXT_ACTIVE,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

const DIALOG_RADIUS: f32 = 13.0;
const DIALOG_OUTER_RADIUS: f32 = 14.0;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

/// State for the CLAUDE.md editor dialog.
#[derive(Debug)]
pub struct ClaudeMdEditorState {
    pub content: text_editor::Content,
    pub file_path: std::path::PathBuf,
    pub dirty: bool,
}

impl ClaudeMdEditorState {
    pub fn new(text: &str, path: std::path::PathBuf) -> Self {
        Self {
            content: text_editor::Content::with_text(text),
            file_path: path,
            dirty: false,
        }
    }

    pub fn text(&self) -> String {
        self.content.text()
    }
}

/// Render the editor dialog as a modal overlay.
pub fn view_claude_md_editor<'a, M: Clone + 'a>(
    state: &'a ClaudeMdEditorState,
    on_action: impl Fn(text_editor::Action) -> M + 'a,
    on_save: M,
    on_close: M,
) -> Element<'a, M> {
    let filename = state
        .file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let title_text = format!("Editing: {}", filename);

    // Save button
    let save_label = if state.dirty { "Save *" } else { "Save" };
    let save_btn = button(text(save_label).size(13))
        .on_press(on_save)
        .padding(Padding::from([6, 14]))
        .style(|_theme, status| {
            let (bg, border_color) = match status {
                button::Status::Hovered => (tint(ACCENT(), 0.30), ACCENT_HOVER()),
                button::Status::Pressed => (tint(ACCENT(), 0.40), ACCENT_HOVER()),
                _ => (tint(ACCENT(), 0.18), ACCENT()),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: TEXT_ACTIVE(),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: 7.0.into(),
                },
                ..button::Style::default()
            }
        });

    // Close button
    let close_btn = button(text("\u{2715}").size(16))
        .on_press(on_close)
        .padding(Padding::from([4, 8]))
        .style(|_theme, status| {
            let (bg, border_color, text_color) = match status {
                button::Status::Hovered => {
                    (tint(BG_TERTIARY(), 0.95), tint(BORDER(), 0.9), TEXT_ACTIVE())
                }
                button::Status::Pressed => {
                    (tint(ACCENT(), 0.18), tint(ACCENT(), 0.6), TEXT_ACTIVE())
                }
                _ => (
                    tint(BG_PRIMARY(), 0.35),
                    tint(BORDER(), 0.7),
                    TEXT_PRIMARY(),
                ),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: 7.0.into(),
                },
                ..button::Style::default()
            }
        });

    // Header row
    let header = container(
        row![
            text(title_text).size(16).color(TEXT_ACTIVE()),
            Space::new().width(Length::Fill),
            text("Ctrl+S to save").size(11).color(TEXT_SECONDARY()),
            save_btn,
            close_btn,
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding(Padding::from([10, 16])),
    )
    .style(|_theme| container::Style {
        background: Some(Background::Color(tint(BG_PRIMARY(), 0.97))),
        ..container::Style::default()
    })
    .width(Length::Fill);

    // Editor pane (left)
    let editor = text_editor(&state.content)
        .on_action(on_action)
        .padding(12)
        .height(Length::Fill);

    let editor_pane = container(editor)
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY(), 0.85))),
            border: Border {
                color: tint(BORDER(), 0.6),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    // Preview pane (right)
    let preview_content = render_markdown_preview(&state.text());
    let preview_pane = container(
        scrollable(
            container(preview_content)
                .padding(12)
                .width(Length::Fill),
        )
        .height(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(Background::Color(tint(BG_SECONDARY(), 0.95))),
        border: Border {
            color: tint(BORDER(), 0.6),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    });

    let content_area = row![editor_pane, preview_pane]
        .spacing(4)
        .height(Length::Fill)
        .padding(Padding::from([0, 8]));

    // Footer
    let path_display = state.file_path.display().to_string();
    let dirty_indicator = if state.dirty { " (modified)" } else { "" };
    let footer = container(
        text(format!("{}{}", path_display, dirty_indicator))
            .size(11)
            .color(TEXT_SECONDARY()),
    )
    .padding(Padding::from([6, 16]))
    .style(|_theme| container::Style {
        background: Some(Background::Color(tint(BG_PRIMARY(), 0.68))),
        ..container::Style::default()
    })
    .width(Length::Fill);

    // Dialog surface
    let dialog_surface = container(column![header, content_area, footer])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(BG_SECONDARY())),
            border: Border {
                color: tint(BG_PRIMARY(), 0.88),
                width: 1.0,
                radius: DIALOG_RADIUS.into(),
            },
            ..container::Style::default()
        });

    // Outer border glow
    let dialog = container(dialog_surface)
        .padding(1)
        .width(Length::FillPortion(85))
        .height(Length::FillPortion(80))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY(), 0.84))),
            border: Border {
                color: tint(ACCENT(), 0.26),
                width: 1.0,
                radius: DIALOG_OUTER_RADIUS.into(),
            },
            shadow: Shadow {
                color: tint(BACKDROP(), 0.65),
                offset: Vector::new(0.0, 14.0),
                blur_radius: 34.0,
            },
            ..container::Style::default()
        });

    // Backdrop + centered dialog
    container(iced::widget::center(dialog))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BACKDROP(), 0.84))),
            ..container::Style::default()
        })
        .into()
}

/// Simple line-by-line markdown to styled text.
fn render_markdown_preview<'a, M: 'a>(markdown: &str) -> Element<'a, M> {
    let mut items: Vec<Element<'a, M>> = Vec::new();
    let mut in_code_block = false;
    let mut code_lines: Vec<String> = Vec::new();

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            if in_code_block {
                // Close code block
                let code_text = code_lines.join("\n");
                let code_elem = container(text(code_text).size(12).color(TEXT_PRIMARY()))
                    .padding(8)
                    .width(Length::Fill)
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(tint(BG_PRIMARY(), 0.90))),
                        border: Border {
                            color: tint(BORDER(), 0.5),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..container::Style::default()
                    });
                items.push(code_elem.into());
                code_lines.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        let trimmed = line.trim();

        if trimmed.is_empty() {
            items.push(Space::new().height(Length::Fixed(6.0)).into());
            continue;
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            items.push(rule::horizontal(1).into());
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("### ") {
            items.push(text(rest.to_string()).size(15).color(TEXT_ACTIVE()).into());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            items.push(text(rest.to_string()).size(17).color(TEXT_ACTIVE()).into());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            items.push(text(rest.to_string()).size(20).color(TEXT_ACTIVE()).into());
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("- ") {
            let bullet = format!("  \u{2022} {}", render_inline_markdown(rest));
            items.push(text(bullet).size(13).color(TEXT_PRIMARY()).into());
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("* ") {
            let bullet = format!("  \u{2022} {}", render_inline_markdown(rest));
            items.push(text(bullet).size(13).color(TEXT_PRIMARY()).into());
            continue;
        }

        items.push(
            text(render_inline_markdown(trimmed))
                .size(13)
                .color(TEXT_PRIMARY())
                .into(),
        );
    }

    // Flush unclosed code block
    if in_code_block && !code_lines.is_empty() {
        let code_text = code_lines.join("\n");
        let code_elem = container(text(code_text).size(12).color(TEXT_PRIMARY()))
            .padding(8)
            .width(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(tint(BG_PRIMARY(), 0.90))),
                border: Border {
                    color: tint(BORDER(), 0.5),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            });
        items.push(code_elem.into());
    }

    if items.is_empty() {
        items.push(
            text("(empty)")
                .size(13)
                .color(TEXT_SECONDARY())
                .into(),
        );
    }

    column(items).spacing(2).width(Length::Fill).into()
}

/// Strip **bold** markers for display (iced text widget doesn't support inline styles).
fn render_inline_markdown(s: &str) -> String {
    s.replace("**", "").replace("__", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_new() {
        let state = ClaudeMdEditorState::new("hello world", "test.md".into());
        assert_eq!(state.text().trim(), "hello world");
        assert!(!state.dirty);
    }

    #[test]
    fn test_state_dirty_default() {
        let state = ClaudeMdEditorState::new("", "CLAUDE.md".into());
        assert!(!state.dirty);
    }

    #[test]
    fn test_render_inline_markdown_strips_bold() {
        assert_eq!(render_inline_markdown("**bold** text"), "bold text");
        assert_eq!(render_inline_markdown("__also__ bold"), "also bold");
    }

    #[test]
    fn test_view_does_not_panic() {
        let state = ClaudeMdEditorState::new("# Hello\n\nSome text", "test.md".into());
        #[derive(Debug, Clone)]
        enum Msg {
            Action(text_editor::Action),
            Save,
            Close,
        }
        let _el: Element<'_, Msg> =
            view_claude_md_editor(&state, Msg::Action, Msg::Save, Msg::Close);
    }

    #[test]
    fn test_preview_empty() {
        #[derive(Debug, Clone)]
        enum Msg {}
        let _el: Element<'_, Msg> = render_markdown_preview("");
    }

    #[test]
    fn test_preview_code_block() {
        #[derive(Debug, Clone)]
        enum Msg {}
        let _el: Element<'_, Msg> = render_markdown_preview("```\ncode\n```");
    }

    #[test]
    fn test_preview_headers_and_lists() {
        #[derive(Debug, Clone)]
        enum Msg {}
        let _el: Element<'_, Msg> =
            render_markdown_preview("# H1\n## H2\n### H3\n- item\n* item2\n---");
    }
}
