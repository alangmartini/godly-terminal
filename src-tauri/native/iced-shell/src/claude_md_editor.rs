use iced::widget::{button, column, container, row, scrollable, text, text_editor, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme::{
    ACCENT, ACCENT_HOVER, BACKDROP, BG_PRIMARY, BG_SECONDARY, BG_TERTIARY, BORDER, TEXT_ACTIVE,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

const DIALOG_RADIUS: f32 = 16.0;
const DIALOG_OUTER_RADIUS: f32 = 17.0;
const PANE_RADIUS: f32 = 10.0;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

/// Mix two colors: result = a * (1 - t) + b * t.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let inv = 1.0 - t;
    Color::from_rgba(
        a.r * inv + b.r * t,
        a.g * inv + b.g * t,
        a.b * inv + b.b * t,
        a.a * inv + b.a * t,
    )
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

/// Build a small label badge (e.g. "Source", "Preview").
fn pane_label<'a, M: 'a>(label: &str) -> Element<'a, M> {
    container(text(label.to_string()).size(10).color(TEXT_SECONDARY()))
        .padding(Padding::from([3, 10]))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_TERTIARY(), 0.45))),
            border: Border {
                color: tint(BORDER(), 0.35),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..container::Style::default()
        })
        .into()
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
    let title_text = filename.to_string();

    // Save button — solid accent fill when dirty, subtle when clean
    let save_label = if state.dirty { "Save" } else { "Saved" };
    let is_dirty = state.dirty;
    let save_btn = button(text(save_label).size(12))
        .on_press(on_save)
        .padding(Padding::from([6, 16]))
        .style(move |_theme, status| {
            if is_dirty {
                let (bg, border_color) = match status {
                    button::Status::Hovered => (tint(ACCENT(), 0.45), ACCENT_HOVER()),
                    button::Status::Pressed => (tint(ACCENT(), 0.55), ACCENT_HOVER()),
                    _ => (tint(ACCENT(), 0.32), ACCENT()),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: TEXT_ACTIVE(),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..button::Style::default()
                }
            } else {
                button::Style {
                    background: Some(Background::Color(tint(BG_TERTIARY(), 0.25))),
                    text_color: TEXT_SECONDARY(),
                    border: Border {
                        color: tint(BORDER(), 0.3),
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..button::Style::default()
                }
            }
        });

    // Close button
    let close_btn = button(text("\u{2715}").size(14))
        .on_press(on_close)
        .padding(Padding::from([5, 9]))
        .style(|_theme, status| {
            let (bg, text_color) = match status {
                button::Status::Hovered => (tint(BG_TERTIARY(), 0.9), TEXT_ACTIVE()),
                button::Status::Pressed => (tint(ACCENT(), 0.2), TEXT_ACTIVE()),
                _ => (Color::TRANSPARENT, TEXT_SECONDARY()),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 8.0.into(),
                },
                ..button::Style::default()
            }
        });

    // Header row — clean with title + save + close
    let header = container(
        row![
            text(title_text).size(14).color(TEXT_ACTIVE()),
            Space::new().width(Length::Fill),
            text("Ctrl+S").size(10).color(tint(TEXT_SECONDARY(), 0.6)),
            save_btn,
            close_btn,
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    )
    .padding(Padding::from([12, 20]))
    .style(|_theme| container::Style {
        border: Border {
            color: tint(BORDER(), 0.25),
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .width(Length::Fill);

    // Thin header separator
    let header_sep = container(Space::new().height(1).width(Length::Fill))
        .padding(Padding::from([0, 16]))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BORDER(), 0.2))),
            ..container::Style::default()
        })
        .width(Length::Fill);

    // Editor pane (left) — darker background, monospace feel
    let editor = text_editor(&state.content)
        .on_action(on_action)
        .padding(14)
        .height(Length::Fill);

    let editor_pane = container(
        column![
            container(pane_label::<M>("SOURCE")).padding(Padding {
                top: 8.0,
                right: 10.0,
                bottom: 0.0,
                left: 10.0
            }),
            editor,
        ]
        .spacing(0),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(Background::Color(tint(BG_PRIMARY(), 0.7))),
        border: Border {
            color: tint(BORDER(), 0.25),
            width: 1.0,
            radius: PANE_RADIUS.into(),
        },
        ..container::Style::default()
    });

    // Preview pane (right) — slightly lighter, more readable
    let preview_content = render_markdown_preview(&state.text());
    let preview_pane = container(
        column![
            container(pane_label::<M>("PREVIEW")).padding(Padding {
                top: 8.0,
                right: 10.0,
                bottom: 0.0,
                left: 10.0
            }),
            scrollable(
                container(preview_content)
                    .padding(Padding {
                        top: 8.0,
                        right: 20.0,
                        bottom: 20.0,
                        left: 20.0
                    })
                    .width(Length::Fill),
            )
            .height(Length::Fill),
        ]
        .spacing(0),
    )
    .width(Length::FillPortion(1))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(Background::Color(mix(BG_SECONDARY(), BG_TERTIARY(), 0.15))),
        border: Border {
            color: tint(BORDER(), 0.25),
            width: 1.0,
            radius: PANE_RADIUS.into(),
        },
        ..container::Style::default()
    });

    let content_area = row![editor_pane, preview_pane]
        .spacing(6)
        .height(Length::Fill)
        .padding(Padding {
            top: 6.0,
            right: 12.0,
            bottom: 12.0,
            left: 12.0,
        });

    // Footer — path + modified indicator
    let path_display = state.file_path.display().to_string();
    let dirty_indicator = if state.dirty {
        "  \u{2022} modified"
    } else {
        ""
    };
    let footer = container(
        row![
            text(path_display)
                .size(10)
                .color(tint(TEXT_SECONDARY(), 0.6)),
            Space::new().width(Length::Fill),
            text(dirty_indicator.to_string())
                .size(10)
                .color(if state.dirty {
                    tint(ACCENT(), 0.8)
                } else {
                    Color::TRANSPARENT
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding(Padding::from([8, 20]))
    .style(|_theme| container::Style {
        border: Border {
            color: tint(BORDER(), 0.15),
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .width(Length::Fill);

    // Dialog surface
    let dialog_surface = container(column![header, header_sep, content_area, footer])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(mix(BG_PRIMARY(), BG_SECONDARY(), 0.6))),
            border: Border {
                color: tint(BORDER(), 0.2),
                width: 1.0,
                radius: DIALOG_RADIUS.into(),
            },
            ..container::Style::default()
        });

    // Outer glow wrapper
    let dialog = container(dialog_surface)
        .padding(1)
        .width(Length::FillPortion(85))
        .height(Length::FillPortion(80))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY(), 0.5))),
            border: Border {
                color: tint(ACCENT(), 0.15),
                width: 1.0,
                radius: DIALOG_OUTER_RADIUS.into(),
            },
            shadow: Shadow {
                color: tint(BACKDROP(), 0.7),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 48.0,
            },
            ..container::Style::default()
        });

    // Backdrop + centered dialog
    container(iced::widget::center(dialog))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BACKDROP(), 0.88))),
            ..container::Style::default()
        })
        .into()
}

/// Render a styled code block with a left accent border.
fn render_code_block<'a, M: 'a>(code_text: String) -> Element<'a, M> {
    // Inner code content
    let code_inner = container(text(code_text).size(12).color(TEXT_PRIMARY()))
        .padding(Padding::from([10, 14]))
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY(), 0.65))),
            border: Border {
                color: tint(BORDER(), 0.3),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        });

    // Left accent bar + code content
    let accent_bar =
        container(Space::new().width(3).height(Length::Fill)).style(|_theme| container::Style {
            background: Some(Background::Color(tint(ACCENT(), 0.5))),
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    row![accent_bar, code_inner].width(Length::Fill).into()
}

/// Render H1 heading with bottom separator.
fn render_h1<'a, M: 'a>(heading: &str) -> Element<'a, M> {
    let sep = container(Space::new().height(1).width(Length::Fill))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BORDER(), 0.25))),
            ..container::Style::default()
        })
        .width(Length::Fill);

    column![
        text(heading.to_string()).size(22).color(TEXT_ACTIVE()),
        Space::new().height(6),
        sep,
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

/// Render H2 heading with subtle accent tint.
fn render_h2<'a, M: 'a>(heading: &str) -> Element<'a, M> {
    column![
        Space::new().height(6),
        text(heading.to_string())
            .size(18)
            .color(mix(TEXT_ACTIVE(), ACCENT(), 0.2)),
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

/// Render H3 heading.
fn render_h3<'a, M: 'a>(heading: &str) -> Element<'a, M> {
    column![
        Space::new().height(4),
        text(heading.to_string()).size(15).color(TEXT_ACTIVE()),
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

/// Render a bullet list item with left padding.
fn render_bullet<'a, M: 'a>(content: &str) -> Element<'a, M> {
    let dot = text("\u{2022}").size(12).color(tint(ACCENT(), 0.6));
    let body = text(render_inline_markdown(content))
        .size(13)
        .color(TEXT_PRIMARY());
    container(row![dot, body].spacing(8).align_y(iced::Alignment::Start))
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 12.0,
        })
        .width(Length::Fill)
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
                let code_text = code_lines.join("\n");
                items.push(render_code_block(code_text));
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
            items.push(Space::new().height(Length::Fixed(8.0)).into());
            continue;
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            items.push(
                container(
                    container(Space::new().height(1).width(Length::Fill))
                        .style(|_theme| container::Style {
                            background: Some(Background::Color(tint(BORDER(), 0.2))),
                            ..container::Style::default()
                        })
                        .width(Length::Fill),
                )
                .padding(Padding::from([8, 0]))
                .width(Length::Fill)
                .into(),
            );
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("### ") {
            items.push(render_h3(rest));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            items.push(render_h2(rest));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            items.push(render_h1(rest));
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("- ") {
            items.push(render_bullet(rest));
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("* ") {
            items.push(render_bullet(rest));
            continue;
        }

        // Numbered list items (e.g. "1. text")
        if let Some(pos) = trimmed.find(". ") {
            if pos <= 3 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                let rest = &trimmed[pos + 2..];
                let num = &trimmed[..pos + 1];
                let num_label = text(num.to_string()).size(12).color(tint(ACCENT(), 0.6));
                let body = text(render_inline_markdown(rest))
                    .size(13)
                    .color(TEXT_PRIMARY());
                items.push(
                    container(
                        row![num_label, body]
                            .spacing(6)
                            .align_y(iced::Alignment::Start),
                    )
                    .padding(Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: 12.0,
                    })
                    .width(Length::Fill)
                    .into(),
                );
                continue;
            }
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
        items.push(render_code_block(code_text));
    }

    if items.is_empty() {
        items.push(
            container(
                column![
                    text("No content yet").size(14).color(TEXT_SECONDARY()),
                    Space::new().height(4),
                    text("Start typing in the source pane to see a preview here.")
                        .size(12)
                        .color(tint(TEXT_SECONDARY(), 0.6)),
                ]
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding(Padding::from([40, 20]))
            .center_x(Length::Fill)
            .into(),
        );
    }

    column(items).spacing(4).width(Length::Fill).into()
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
