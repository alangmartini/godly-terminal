use iced::widget::{button, center, column, container, row, text, Space};
use iced::{Border, Color, Element, Length, Padding};

/// A tab in the settings dialog.
pub struct SettingsTab {
    pub id: &'static str,
    pub label: &'static str,
}

// Colors
const BACKDROP_COLOR: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.5);
const DIALOG_BG: Color = Color::from_rgb(0.12, 0.12, 0.15);
const HEADER_BG: Color = Color::from_rgb(0.08, 0.08, 0.10);
const TAB_ACTIVE_BG: Color = Color::from_rgb(0.2, 0.2, 0.25);
const TAB_INACTIVE_BG: Color = Color::TRANSPARENT;
const TEXT_COLOR: Color = Color::from_rgb(0.85, 0.85, 0.85);
const DIM_TEXT_COLOR: Color = Color::from_rgb(0.5, 0.5, 0.55);

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
    let mut tab_row = row![].spacing(2);
    for tab in tabs {
        let is_active = tab.id == active_tab;
        let bg = if is_active {
            TAB_ACTIVE_BG
        } else {
            TAB_INACTIVE_BG
        };
        let tab_id = tab.id.to_string();
        let tab_btn = button(text(tab.label).size(13).color(TEXT_COLOR))
            .on_press(on_tab_click(tab_id))
            .padding(Padding::from([6, 14]))
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: TEXT_COLOR,
                border: Border::default().rounded(4),
                ..button::Style::default()
            });
        tab_row = tab_row.push(tab_btn);
    }

    // Header: "Settings" title + close button
    let close_btn = button(text("\u{2715}").size(16).color(TEXT_COLOR))
        .on_press(on_close)
        .padding(Padding::from([4, 8]))
        .style(|_theme, _status| button::Style {
            background: None,
            text_color: TEXT_COLOR,
            border: Border::default(),
            ..button::Style::default()
        });

    let header = container(
        row![
            text("Settings").size(18).color(TEXT_COLOR),
            Space::new().width(Length::Fill),
            close_btn,
        ]
        .align_y(iced::Alignment::Center)
        .padding(Padding::from([10, 16])),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(HEADER_BG)),
        ..container::Style::default()
    })
    .width(Length::Fill);

    // Version footer
    let version = format!(
        "Godly Terminal (Native) \u{2014} v{}",
        godly_protocol::FRONTEND_CONTRACT_VERSION
    );
    let footer = container(text(version).size(11).color(DIM_TEXT_COLOR))
        .padding(Padding::from([8, 16]))
        .width(Length::Fill);

    // Dialog content
    let dialog = container(
        column![
            header,
            container(tab_row).padding(Padding::from([8, 16])),
            container(tab_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(Padding::from([8, 16])),
            footer,
        ],
    )
    .width(Length::FillPortion(80))
    .height(Length::FillPortion(70))
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(DIALOG_BG)),
        border: Border::default().rounded(8),
        ..container::Style::default()
    });

    // Backdrop + centered dialog
    container(center(dialog))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BACKDROP_COLOR)),
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
