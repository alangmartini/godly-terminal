use iced::widget::{center, column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::theme::{
    BACKDROP, BG_ACTIVE, BG_SECONDARY, BORDER, TEXT_ACTIVE, TEXT_PRIMARY, TEXT_SECONDARY,
};

const MAX_VISIBLE_ENTRIES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MruSwitcherEntry {
    pub terminal_id: String,
    pub label: String,
    pub detail: Option<String>,
}

pub fn view_overlay<'a, Message: 'a>(
    entries: Vec<MruSwitcherEntry>,
    selected_terminal_id: Option<&'a str>,
) -> Element<'a, Message> {
    if entries.is_empty() {
        return Space::new().into();
    }

    let mut list = column![
        text("Recent Tabs").size(13).color(TEXT_SECONDARY),
        text("Release Ctrl to switch. Press Esc to cancel.")
            .size(11)
            .color(TEXT_SECONDARY),
    ]
    .spacing(6)
    .width(Length::Fixed(420.0));

    for entry in entries.into_iter().take(MAX_VISIBLE_ENTRIES) {
        let is_selected = Some(entry.terminal_id.as_str()) == selected_terminal_id;
        let row_bg = if is_selected { BG_ACTIVE } else { BG_SECONDARY };
        let label_color = if is_selected {
            TEXT_ACTIVE
        } else {
            TEXT_PRIMARY
        };
        let detail_color = if is_selected {
            TEXT_PRIMARY
        } else {
            TEXT_SECONDARY
        };
        let badge = if is_selected { "Selected" } else { "" };

        let mut labels = column![text(entry.label).size(14).color(label_color)].spacing(2);
        if let Some(detail) = entry.detail {
            labels = labels.push(text(detail).size(11).color(detail_color));
        }

        let entry_row = row![
            labels,
            Space::new().width(Length::Fill),
            text(badge).size(11).color(detail_color),
        ]
        .align_y(Alignment::Center)
        .spacing(8);

        list = list.push(
            container(entry_row)
                .width(Length::Fill)
                .padding(Padding::from([8, 10]))
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(row_bg)),
                    border: iced::Border {
                        color: BORDER,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    let card = container(list)
        .padding(Padding::from([12, 12]))
        .width(Length::Fixed(440.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY)),
            border: iced::Border {
                color: BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        });

    container(center(card))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BACKDROP)),
            ..container::Style::default()
        })
        .into()
}
