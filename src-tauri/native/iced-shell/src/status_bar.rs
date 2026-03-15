use iced::widget::{container, row, text, Space};
use iced::{Border, Color, Element, Length, Padding};

use crate::theme::{BG_SECONDARY, BORDER, TEXT_PRIMARY, TEXT_SECONDARY};

/// Height of the status bar in logical pixels.
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Information needed to render the status bar.
pub struct StatusBarInfo<'a> {
    pub shell_label: &'a str,
    pub cwd: &'a str,
    pub cols: u16,
    pub rows: u16,
}

/// Derive a short shell type label from a process name.
pub fn shell_label(process_name: &str) -> &'static str {
    let lower = process_name.to_ascii_lowercase();
    if lower.contains("pwsh") || lower.contains("powershell") {
        "PS"
    } else if lower.contains("bash") {
        "bash"
    } else if lower.contains("zsh") {
        "zsh"
    } else if lower.contains("fish") {
        "fish"
    } else if lower == "cmd" || lower.contains("cmd.exe") {
        "cmd"
    } else if lower.contains("wsl") {
        "WSL"
    } else if lower.contains("sh") {
        "sh"
    } else {
        "term"
    }
}

/// Renders the bottom status bar.
pub fn view_status_bar<'a, M: Clone + 'a>(info: Option<StatusBarInfo<'_>>) -> Element<'a, M> {
    let (shell, cwd, dims) = match info {
        Some(info) => (
            info.shell_label.to_string(),
            if info.cwd.is_empty() {
                String::new()
            } else {
                info.cwd.to_string()
            },
            format!("{}x{}", info.cols, info.rows),
        ),
        None => (String::new(), String::new(), String::new()),
    };

    let shell_text = text(shell)
        .size(11)
        .color(TEXT_PRIMARY())
        .font(iced::Font::MONOSPACE);

    let cwd_text = text(cwd)
        .size(11)
        .color(TEXT_SECONDARY());

    let dims_text = text(dims)
        .size(11)
        .color(TEXT_SECONDARY())
        .font(iced::Font::MONOSPACE);

    let content = row![
        container(shell_text).padding(Padding::from([0, 8])),
        container(cwd_text).padding(Padding::from([0, 4])),
        Space::new().width(Length::Fill),
        container(dims_text).padding(Padding::from([0, 8])),
    ]
    .align_y(iced::Alignment::Center)
    .height(Length::Fixed(STATUS_BAR_HEIGHT));

    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(STATUS_BAR_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY())),
            border: Border {
                color: Color::from_rgba(BORDER().r, BORDER().g, BORDER().b, 0.5),
                width: 0.5,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_label_maps_known_shells() {
        assert_eq!(shell_label("pwsh"), "PS");
        assert_eq!(shell_label("PowerShell"), "PS");
        assert_eq!(shell_label("bash"), "bash");
        assert_eq!(shell_label("zsh"), "zsh");
        assert_eq!(shell_label("cmd.exe"), "cmd");
        assert_eq!(shell_label("fish"), "fish");
        assert_eq!(shell_label("wsl"), "WSL");
        assert_eq!(shell_label("unknown"), "term");
    }

    #[derive(Clone)]
    enum Msg {
        Noop,
    }

    #[test]
    fn status_bar_renders_with_info() {
        let info = StatusBarInfo {
            shell_label: "PS",
            cwd: "C:\\Users\\test",
            cols: 80,
            rows: 24,
        };
        let _el: Element<'_, Msg> = view_status_bar(Some(info));
    }

    #[test]
    fn status_bar_renders_without_info() {
        let _el: Element<'_, Msg> = view_status_bar(None);
    }
}
