use iced::widget::{button, center, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme::{
    ACCENT, ACCENT_HOVER, BACKDROP, BG_PRIMARY, BG_SECONDARY, BG_TERTIARY, BORDER, TEXT_ACTIVE,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

/// A tab in the settings dialog.
pub struct SettingsTab {
    pub id: &'static str,
    pub label: &'static str,
}

const DIALOG_RADIUS: f32 = 13.0;
const DIALOG_OUTER_RADIUS: f32 = 14.0;
const TAB_RADIUS: f32 = 7.0;
const TAB_STRIP_RADIUS: f32 = 9.0;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

/// Renders a centered modal settings dialog overlay.
///
/// - Semi-transparent backdrop
/// - Header with "Settings" title and close button (X)
/// - Horizontal tab bar
/// - Content area (passed in as `tab_content`)
/// - Footer with version info
pub fn view_settings_dialog<'a, M: Clone + 'a>(
    tabs: &[SettingsTab],
    active_tab: &str,
    tab_content: Element<'a, M>,
    on_tab_click: impl Fn(String) -> M + 'a,
    on_close: M,
) -> Element<'a, M> {
    // Build tab buttons
    let mut tab_row = row![].spacing(6);
    for tab in tabs {
        let is_active = tab.id == active_tab;
        let tab_id = tab.id.to_string();
        let tab_btn = button(text(tab.label).size(13))
            .on_press(on_tab_click(tab_id))
            .padding(Padding::from([7, 14]))
            .style(move |_theme, status| {
                let (bg, border_color, text_color, shadow) = if is_active {
                    (
                        tint(ACCENT, 0.22),
                        ACCENT_HOVER,
                        TEXT_ACTIVE,
                        Shadow {
                            color: tint(ACCENT, 0.30),
                            offset: Vector::new(0.0, 1.0),
                            blur_radius: 8.0,
                        },
                    )
                } else {
                    match status {
                        button::Status::Hovered => (
                            tint(BG_TERTIARY, 0.95),
                            tint(ACCENT, 0.45),
                            TEXT_PRIMARY,
                            Shadow::default(),
                        ),
                        button::Status::Pressed => (
                            tint(ACCENT, 0.16),
                            tint(ACCENT, 0.60),
                            TEXT_ACTIVE,
                            Shadow::default(),
                        ),
                        _ => (
                            tint(BG_PRIMARY, 0.18),
                            tint(BORDER, 0.85),
                            TEXT_SECONDARY,
                            Shadow::default(),
                        ),
                    }
                };

                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color,
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: TAB_RADIUS.into(),
                    },
                    shadow,
                    ..button::Style::default()
                }
            });
        tab_row = tab_row.push(tab_btn);
    }

    // Header: "Settings" title + close button
    let close_btn = button(text("\u{2715}").size(16))
        .on_press(on_close)
        .padding(Padding::from([4, 8]))
        .style(|_theme, status| {
            let (bg, border_color, text_color) = match status {
                button::Status::Hovered => {
                    (tint(BG_TERTIARY, 0.95), tint(BORDER, 0.9), TEXT_ACTIVE)
                }
                button::Status::Pressed => (tint(ACCENT, 0.18), tint(ACCENT, 0.6), TEXT_ACTIVE),
                _ => (tint(BG_PRIMARY, 0.35), tint(BORDER, 0.7), TEXT_PRIMARY),
            };

            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: TAB_RADIUS.into(),
                },
                ..button::Style::default()
            }
        });

    let header = container(
        row![
            text("Settings").size(18).color(TEXT_ACTIVE),
            Space::new().width(Length::Fill),
            close_btn,
        ]
        .align_y(iced::Alignment::Center)
        .padding(Padding::from([10, 16])),
    )
    .style(|_theme| container::Style {
        background: Some(Background::Color(tint(BG_PRIMARY, 0.97))),
        ..container::Style::default()
    })
    .width(Length::Fill);

    // Version footer
    let version = format!(
        "Godly Terminal (Native) \u{2014} v{}",
        godly_protocol::FRONTEND_CONTRACT_VERSION
    );
    let footer = container(text(version).size(11).color(TEXT_SECONDARY))
        .padding(Padding::from([8, 16]))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY, 0.68))),
            ..container::Style::default()
        })
        .width(Length::Fill);

    let tab_strip = container(
        scrollable(tab_row)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::hidden(),
            ))
            .width(Length::Fill),
    )
    .padding(Padding::from([8, 10]))
    .width(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(Background::Color(tint(BG_PRIMARY, 0.72))),
        border: Border {
            color: tint(BORDER, 0.9),
            width: 1.0,
            radius: TAB_STRIP_RADIUS.into(),
        },
        shadow: Shadow {
            color: tint(BACKDROP, 0.28),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 6.0,
        },
        ..container::Style::default()
    });

    // Dialog content
    let dialog_surface = container(column![
        header,
        container(tab_strip).padding(Padding::from([8, 16])),
        container(tab_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::from([10, 16])),
        footer,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(Background::Color(BG_SECONDARY)),
        border: Border {
            color: tint(BG_PRIMARY, 0.88),
            width: 1.0,
            radius: DIALOG_RADIUS.into(),
        },
        ..container::Style::default()
    });

    let dialog = container(dialog_surface)
        .padding(1)
        .width(Length::FillPortion(80))
        .height(Length::FillPortion(70))
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY, 0.84))),
            border: Border {
                color: tint(ACCENT, 0.26),
                width: 1.0,
                radius: DIALOG_OUTER_RADIUS.into(),
            },
            shadow: Shadow {
                color: tint(BACKDROP, 0.65),
                offset: Vector::new(0.0, 14.0),
                blur_radius: 34.0,
            },
            ..container::Style::default()
        });

    // Backdrop + centered dialog
    container(center(dialog))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BACKDROP, 0.84))),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_tab_struct() {
        let tab = SettingsTab {
            id: "shortcuts",
            label: "Shortcuts",
        };
        assert_eq!(tab.id, "shortcuts");
        assert_eq!(tab.label, "Shortcuts");
    }
}
