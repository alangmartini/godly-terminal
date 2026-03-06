use iced::widget::{button, column, container, row, text, Space};
use iced::{Border, Color, Element, Length, Padding};

use crate::theme::{ACCENT, BACKDROP, BG_PRIMARY, RADIUS_LG, TEXT_PRIMARY, TEXT_SECONDARY};

/// State for the whisper recording overlay.
#[derive(Debug, Clone)]
pub struct WhisperState {
    pub recording: bool,
    pub level: f32,
    pub elapsed_ms: u64,
    pub transcribing: bool,
}

impl WhisperState {
    pub fn new() -> Self {
        Self {
            recording: false,
            level: 0.0,
            elapsed_ms: 0,
            transcribing: false,
        }
    }
}

const OVERLAY_WIDTH: f32 = 320.0;
const LEVEL_BAR_HEIGHT: f32 = 8.0;
const LEVEL_BAR_WIDTH: f32 = 240.0;
const RECORDING_DOT_COLOR: Color = Color::from_rgb(0.90, 0.20, 0.20);
const LEVEL_BAR_BG: Color = Color::from_rgb(0.15, 0.15, 0.20);
const LEVEL_BAR_FILL: Color = Color::from_rgb(0.30, 0.75, 0.45);
const STOP_BTN_BG: Color = Color::from_rgb(0.85, 0.22, 0.22);
const STOP_BTN_HOVER_BG: Color = Color::from_rgb(0.95, 0.30, 0.30);
const CANCEL_BTN_BG: Color = Color::from_rgb(0.25, 0.25, 0.30);
const CANCEL_BTN_HOVER_BG: Color = Color::from_rgb(0.35, 0.35, 0.40);

fn format_elapsed(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins:02}:{secs:02}")
}

/// Render the recording overlay as a centered modal card.
pub fn view_whisper_overlay<'a, M: Clone + 'a>(
    state: &WhisperState,
    on_stop: M,
    on_cancel: M,
) -> Element<'a, M> {
    let title_text = if state.transcribing {
        "Transcribing..."
    } else {
        "Recording..."
    };

    let dot = container(Space::new().width(10.0).height(10.0)).style(move |_theme| container::Style {
        background: Some(iced::Background::Color(RECORDING_DOT_COLOR)),
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });

    let title_row = row![
        dot,
        Space::new().width(8.0),
        text(title_text).size(16).color(TEXT_PRIMARY()),
    ]
    .align_y(iced::Alignment::Center);

    // Level meter bar
    let fill_width = (LEVEL_BAR_WIDTH * state.level).max(0.0);
    let level_fill = container(Space::new().width(fill_width).height(LEVEL_BAR_HEIGHT)).style(
        move |_theme| container::Style {
            background: Some(iced::Background::Color(LEVEL_BAR_FILL)),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        },
    );
    let level_bar = container(level_fill)
        .width(Length::Fixed(LEVEL_BAR_WIDTH))
        .height(Length::Fixed(LEVEL_BAR_HEIGHT))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(LEVEL_BAR_BG)),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    let timer = text(format_elapsed(state.elapsed_ms))
        .size(28)
        .color(TEXT_PRIMARY());

    let stop_label = if state.transcribing { "..." } else { "Stop" };
    let stop_btn = button(
        text(stop_label)
            .size(13)
            .color(Color::WHITE)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Length::Fixed(90.0))
    .padding(Padding::from([6, 16]))
    .style(move |_theme, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => STOP_BTN_HOVER_BG,
            _ => STOP_BTN_BG,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    });

    let stop_btn = if state.transcribing {
        stop_btn
    } else {
        stop_btn.on_press(on_stop)
    };

    let cancel_btn = button(
        text("Cancel")
            .size(13)
            .color(TEXT_SECONDARY())
            .align_x(iced::alignment::Horizontal::Center),
    )
    .on_press(on_cancel)
    .width(Length::Fixed(90.0))
    .padding(Padding::from([6, 16]))
    .style(move |_theme, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => CANCEL_BTN_HOVER_BG,
            _ => CANCEL_BTN_BG,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: TEXT_SECONDARY(),
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    });

    let btn_row = row![stop_btn, Space::new().width(12.0), cancel_btn]
        .align_y(iced::Alignment::Center);

    let card_content = column![
        title_row,
        Space::new().height(12.0),
        level_bar,
        Space::new().height(16.0),
        container(timer).align_x(iced::alignment::Horizontal::Center),
        Space::new().height(16.0),
        container(btn_row).align_x(iced::alignment::Horizontal::Center),
    ]
    .align_x(iced::Alignment::Center)
    .width(Length::Fixed(OVERLAY_WIDTH));

    let card = container(card_content)
        .padding(Padding::from([24, 28]))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(BG_PRIMARY())),
            border: Border {
                color: ACCENT(),
                width: 1.0,
                radius: RADIUS_LG.into(),
            },
            ..container::Style::default()
        });

    let backdrop = container(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(BACKDROP())),
        ..container::Style::default()
    });

    backdrop.into()
}

/// Render a small mic button for the tab bar.
pub fn view_mic_button<'a, M: Clone + 'a>(on_press: M) -> Element<'a, M> {
    let mic_icon_color = Color::from_rgb(0.92, 0.92, 0.96);
    let mic_bg = Color::from_rgb(0.13, 0.13, 0.17);
    let mic_hover_bg = Color::from_rgb(0.16, 0.16, 0.20);
    let mic_border_idle = Color::from_rgba(0.48, 0.48, 0.58, 0.32);
    let mic_border_hover = Color::from_rgba(0.60, 0.60, 0.72, 0.45);

    button(
        text("\u{1F3A4}")
            .size(12)
            .color(mic_icon_color)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .on_press(on_press)
    .padding(Padding::from([2, 6]))
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(24.0))
    .style(move |_theme, status| {
        let (bg, border_color) = match status {
            button::Status::Hovered | button::Status::Pressed => (mic_hover_bg, mic_border_hover),
            _ => (mic_bg, mic_border_idle),
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: mic_icon_color,
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..button::Style::default()
        }
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_elapsed_formats_correctly() {
        assert_eq!(format_elapsed(0), "00:00");
        assert_eq!(format_elapsed(5_000), "00:05");
        assert_eq!(format_elapsed(65_000), "01:05");
        assert_eq!(format_elapsed(3_661_000), "61:01");
    }

    #[derive(Clone)]
    enum TestMsg {
        Stop,
        Cancel,
    }

    #[test]
    fn view_whisper_overlay_renders_without_panic() {
        let state = WhisperState::new();
        let _ = view_whisper_overlay(&state, TestMsg::Stop, TestMsg::Cancel);
    }

    #[test]
    fn view_whisper_overlay_renders_transcribing_state() {
        let state = WhisperState {
            recording: false,
            level: 0.5,
            elapsed_ms: 3000,
            transcribing: true,
        };
        let _ = view_whisper_overlay(&state, TestMsg::Stop, TestMsg::Cancel);
    }

    #[test]
    fn view_mic_button_renders_without_panic() {
        let _ = view_mic_button(TestMsg::Stop);
    }
}
