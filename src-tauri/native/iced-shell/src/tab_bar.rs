use iced::widget::{button, container, row, text};
use iced::{Border, Color, Element, Length, Padding};

use crate::terminal_state::TerminalInfo;

/// Height of the tab bar in logical pixels.
pub const TAB_BAR_HEIGHT: f32 = 32.0;

// Colors
const TAB_BAR_BG: Color = Color::from_rgb(0.08, 0.08, 0.10);
const ACTIVE_TAB_BG: Color = Color::from_rgb(0.2, 0.2, 0.25);
const INACTIVE_TAB_BG: Color = Color::from_rgb(0.12, 0.12, 0.15);
const TAB_TEXT_COLOR: Color = Color::from_rgb(0.85, 0.85, 0.85);
const CLOSE_HOVER_BG: Color = Color::from_rgb(0.35, 0.15, 0.15);

/// Renders the tab bar as a horizontal row of tab buttons.
///
/// This function is generic over the message type so it can be used
/// independently of any specific app `Message` enum.
pub fn view_tab_bar<'a, M: Clone + 'a>(
    terminals: &[&'a TerminalInfo],
    active_id: Option<&str>,
    on_tab_click: impl Fn(String) -> M + 'a,
    on_close: impl Fn(String) -> M + 'a,
    on_new: M,
) -> Element<'a, M> {
    let mut tabs = row![].spacing(1);

    for &terminal in terminals {
        let is_active = active_id == Some(terminal.id.as_str());
        let bg = if is_active {
            ACTIVE_TAB_BG
        } else {
            INACTIVE_TAB_BG
        };

        let label = text(terminal.tab_label())
            .size(13)
            .color(TAB_TEXT_COLOR);

        let close_id = terminal.id.clone();
        let close_btn = button(text("\u{00D7}").size(14).color(TAB_TEXT_COLOR))
            .on_press(on_close(close_id))
            .padding(Padding::from([0, 4]))
            .style(move |_theme, status| {
                let bg_color = match status {
                    button::Status::Hovered | button::Status::Pressed => CLOSE_HOVER_BG,
                    _ => Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    text_color: TAB_TEXT_COLOR,
                    border: Border::default(),
                    ..button::Style::default()
                }
            });

        let tab_content = row![label, close_btn]
            .spacing(4)
            .align_y(iced::Alignment::Center);

        let tab_id = terminal.id.clone();
        let tab_btn = button(tab_content)
            .on_press(on_tab_click(tab_id))
            .padding(Padding::from([4, 10]))
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: TAB_TEXT_COLOR,
                border: Border::default(),
                ..button::Style::default()
            });

        tabs = tabs.push(tab_btn);
    }

    // "+" button to add new terminals.
    let new_btn = button(text("+").size(14).color(TAB_TEXT_COLOR))
        .on_press(on_new)
        .padding(Padding::from([4, 10]))
        .style(|_theme, status| {
            let bg_color = match status {
                button::Status::Hovered | button::Status::Pressed => ACTIVE_TAB_BG,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: TAB_TEXT_COLOR,
                border: Border::default(),
                ..button::Style::default()
            }
        });

    tabs = tabs.push(new_btn);

    container(tabs)
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(TAB_BAR_BG)),
            ..container::Style::default()
        })
        .into()
}
