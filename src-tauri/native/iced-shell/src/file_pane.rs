use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Element, Length, Padding};
use godly_layout_core::{PaneContent, FileViewerType};
use crate::theme::{BG_SECONDARY, BG_TERTIARY, BORDER, TEXT_ACTIVE, TEXT_PRIMARY, TEXT_SECONDARY};

/// Main render function for a file pane.
///
/// Extracts file metadata from `content`, renders a header bar with filename
/// and close button, and delegates to the appropriate content renderer based
/// on file type.
pub fn render_file_pane<'a, M: Clone + 'a>(
    content: &PaneContent,
    file_content: &str,
    on_close: M,
) -> Element<'a, M> {
    let (file_path, file_type) = match content {
        PaneContent::FileViewer {
            file_path,
            file_type,
            ..
        } => (file_path.as_str(), *file_type),
        _ => {
            return container(text(""))
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
    };

    let basename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);

    let header = render_header(basename, on_close);

    let body: Element<'a, M> = match file_type {
        FileViewerType::Code => render_code_pane(file_content),
        FileViewerType::Markdown => render_markdown_pane(file_content),
        FileViewerType::Image => render_image_pane(file_path),
    };

    column![header, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Renders the header bar with filename and close button.
fn render_header<'a, M: Clone + 'a>(
    filename: &str,
    on_close: M,
) -> Element<'a, M> {
    let name_label = text(filename.to_string())
        .size(13)
        .font(iced::Font::MONOSPACE)
        .color(TEXT_PRIMARY());

    let close_btn = button(
        text("X").size(12).color(TEXT_SECONDARY()),
    )
    .on_press(on_close)
    .padding(Padding::from([2, 6]))
    .style(|_theme, _status| button::Style {
        background: None,
        text_color: TEXT_SECONDARY(),
        ..button::Style::default()
    });

    container(
        row![name_label, Space::new().width(Length::Fill), close_btn]
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([6, 12])),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(BG_SECONDARY())),
        border: Border {
            color: BORDER(),
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

/// Renders a code file with line numbers in a scrollable view.
fn render_code_pane<'a, M: Clone + 'a>(content: &str) -> Element<'a, M> {
    let lines: Vec<Element<M>> = content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let line_num = text(format!("{:>4} ", i + 1))
                .size(13)
                .font(iced::Font::MONOSPACE)
                .color(TEXT_SECONDARY());
            let line_text = text(line.to_string())
                .size(13)
                .font(iced::Font::MONOSPACE)
                .color(TEXT_PRIMARY());
            row![line_num, line_text].spacing(8).into()
        })
        .collect();

    scrollable(column(lines).spacing(0).padding(Padding::from([8, 12])))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Renders a markdown file with basic styled headings, paragraphs, and code blocks.
fn render_markdown_pane<'a, M: Clone + 'a>(content: &str) -> Element<'a, M> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let parser = Parser::new(content);
    let mut elements: Vec<Element<M>> = Vec::new();
    let mut current_text = String::new();
    let mut heading_level = 0u8;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_paragraph(&mut current_text, &mut elements);
                heading_level = level as u8;
            }
            Event::End(TagEnd::Heading(_)) => {
                let size = match heading_level {
                    1 => 24,
                    2 => 20,
                    3 => 17,
                    _ => 15,
                };
                elements.push(
                    text(std::mem::take(&mut current_text))
                        .size(size)
                        .color(TEXT_ACTIVE())
                        .font(iced::Font {
                            weight: iced::font::Weight::Bold,
                            ..iced::Font::DEFAULT
                        })
                        .into(),
                );
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_paragraph(&mut current_text, &mut elements);
            }
            Event::End(TagEnd::CodeBlock) => {
                elements.push(
                    container(
                        text(std::mem::take(&mut current_text))
                            .size(13)
                            .font(iced::Font::MONOSPACE)
                            .color(TEXT_PRIMARY()),
                    )
                    .padding(8)
                    .style(|_| container::Style {
                        background: Some(Background::Color(BG_TERTIARY())),
                        border: Border {
                            radius: 4.0.into(),
                            ..Border::default()
                        },
                        ..container::Style::default()
                    })
                    .width(Length::Fill)
                    .into(),
                );
            }
            Event::Text(t) => {
                current_text.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => {
                current_text.push('\n');
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_paragraph(&mut current_text, &mut elements);
                elements.push(Space::new().height(8).into());
            }
            _ => {}
        }
    }

    // Flush any remaining text.
    flush_paragraph(&mut current_text, &mut elements);

    scrollable(column(elements).spacing(4).padding(Padding::from([12, 16])))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Helper: if `buf` is non-empty, push a paragraph element and clear the buffer.
fn flush_paragraph<'a, M: Clone + 'a>(
    buf: &mut String,
    elements: &mut Vec<Element<'a, M>>,
) {
    if !buf.is_empty() {
        elements.push(
            text(std::mem::take(buf))
                .size(14)
                .color(TEXT_PRIMARY())
                .into(),
        );
    }
}

/// Renders an image file centered in the pane.
fn render_image_pane<'a, M: Clone + 'a>(file_path: &str) -> Element<'a, M> {
    use iced::widget::image::{Handle, Image};

    container(
        Image::new(Handle::from_path(file_path)).content_fit(iced::ContentFit::Contain),
    )
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
