use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme::{
    ACCENT, BACKDROP, BG_SECONDARY, BG_TERTIARY, BORDER, DANGER, RADIUS_LG, RADIUS_MD, RADIUS_SM,
    TEXT_ACTIVE, TEXT_PRIMARY, TEXT_SECONDARY,
};

/// Render a centered modal confirmation dialog.
///
/// - Semi-transparent backdrop
/// - Title, body text (or element), confirm/cancel buttons
pub fn view_confirm_dialog<'a, M: Clone + 'a>(
    title: &'a str,
    body: Element<'a, M>,
    confirm_label: &'a str,
    cancel_label: &'a str,
    on_confirm: M,
    on_cancel: M,
    danger: bool,
) -> Element<'a, M> {
    let title_text = text(title).size(16).color(TEXT_ACTIVE());

    let confirm_color = if danger { DANGER() } else { ACCENT() };

    let cancel_btn = button(text(cancel_label).size(13).color(TEXT_PRIMARY()))
        .on_press(on_cancel)
        .padding(Padding::from([7, 18]))
        .style(move |_theme, status| {
            let bg = match status {
                button::Status::Hovered => BG_TERTIARY(),
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: TEXT_PRIMARY(),
                border: Border {
                    color: BORDER(),
                    width: 1.0,
                    radius: RADIUS_MD.into(),
                },
                ..button::Style::default()
            }
        });

    let confirm_btn = button(text(confirm_label).size(13).color(Color::WHITE))
        .on_press(on_confirm)
        .padding(Padding::from([7, 18]))
        .style(move |_theme, status| {
            let bg = match status {
                button::Status::Hovered => Color::from_rgba(
                    (confirm_color.r * 1.1).min(1.0),
                    (confirm_color.g * 1.1).min(1.0),
                    (confirm_color.b * 1.1).min(1.0),
                    1.0,
                ),
                _ => confirm_color,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: Color::WHITE,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: RADIUS_MD.into(),
                },
                ..button::Style::default()
            }
        });

    let footer = row![Space::new().width(Length::Fill), cancel_btn, confirm_btn].spacing(10);

    let dialog_content = column![
        title_text,
        Space::new().height(12.0),
        body,
        Space::new().height(16.0),
        footer,
    ];

    let dialog = container(dialog_content)
        .padding(Padding::from([20, 24]))
        .width(Length::Fixed(440.0))
        .style(|_theme| container::Style {
            background: Some(Background::Color(BG_SECONDARY())),
            border: Border {
                color: BORDER(),
                width: 1.0,
                radius: RADIUS_LG.into(),
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
        .style(|_theme| container::Style {
            background: Some(Background::Color(BACKDROP())),
            ..container::Style::default()
        })
        .into()
}

/// Render a quit confirmation dialog showing active terminal count.
pub fn view_quit_confirm<'a, M: Clone + 'a>(
    terminal_count: usize,
    on_confirm: M,
    on_cancel: M,
) -> Element<'a, M> {
    let body_text = format!(
        "You have {} active terminal session{}. \
         Quitting will detach all sessions (they continue running in the daemon).\n\n\
         Are you sure you want to quit?",
        terminal_count,
        if terminal_count == 1 { "" } else { "s" }
    );
    let body = text(body_text).size(13).color(TEXT_PRIMARY()).into();

    view_confirm_dialog(
        "Quit Godly Terminal?",
        body,
        "Quit",
        "Cancel",
        on_confirm,
        on_cancel,
        true,
    )
}

/// Render a copy preview dialog for large selections.
pub fn view_copy_preview<'a, M: Clone + 'a>(
    preview_text: &str,
    total_chars: usize,
    on_confirm: M,
    on_cancel: M,
) -> Element<'a, M> {
    let truncated = if preview_text.len() > 500 {
        format!("{}...", &preview_text[..500])
    } else {
        preview_text.to_string()
    };

    let body = column![
        text(format!("Selection contains {} characters.", total_chars))
            .size(13)
            .color(TEXT_SECONDARY()),
        Space::new().height(8.0),
        container(
            scrollable(text(truncated).size(12).color(TEXT_PRIMARY()))
                .height(Length::Fixed(200.0))
        )
        .padding(Padding::from([8, 10]))
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(BG_TERTIARY())),
            border: Border {
                color: BORDER(),
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..container::Style::default()
        }),
    ]
    .into();

    view_confirm_dialog(
        "Copy Large Selection?",
        body,
        "Copy",
        "Cancel",
        on_confirm,
        on_cancel,
        false,
    )
}

/// Threshold in characters above which a copy preview is shown.
pub const COPY_PREVIEW_THRESHOLD: usize = 500;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    enum TestMsg {
        Confirm,
        Cancel,
    }

    #[test]
    fn quit_confirm_renders() {
        let _el: Element<'_, TestMsg> = view_quit_confirm(3, TestMsg::Confirm, TestMsg::Cancel);
    }

    #[test]
    fn quit_confirm_singular() {
        let _el: Element<'_, TestMsg> = view_quit_confirm(1, TestMsg::Confirm, TestMsg::Cancel);
    }

    #[test]
    fn copy_preview_renders() {
        let _el: Element<'_, TestMsg> = view_copy_preview(
            "hello world this is a test",
            26,
            TestMsg::Confirm,
            TestMsg::Cancel,
        );
    }

    #[test]
    fn copy_preview_long_text() {
        let long = "x".repeat(1000);
        let _el: Element<'_, TestMsg> =
            view_copy_preview(&long, 1000, TestMsg::Confirm, TestMsg::Cancel);
    }

    #[test]
    fn copy_preview_threshold() {
        assert_eq!(COPY_PREVIEW_THRESHOLD, 500);
    }

    #[test]
    fn confirm_dialog_renders() {
        let body: Element<'_, TestMsg> = text("Are you sure?").into();
        let _el: Element<'_, TestMsg> = view_confirm_dialog(
            "Confirm",
            body,
            "Yes",
            "No",
            TestMsg::Confirm,
            TestMsg::Cancel,
            false,
        );
    }

    #[test]
    fn confirm_dialog_danger_mode() {
        let body: Element<'_, TestMsg> = text("This is dangerous!").into();
        let _el: Element<'_, TestMsg> = view_confirm_dialog(
            "Warning",
            body,
            "Delete",
            "Cancel",
            TestMsg::Confirm,
            TestMsg::Cancel,
            true,
        );
    }
}
