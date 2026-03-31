use iced::widget::{column, container, row, rule, text, Space};
use iced::{Border, Color, Element, Font, Length, Padding};

use crate::theme::{BORDER, BORDER_VARIANT, GHOST_HOVER, STATUS_BAR_BG, TEXT_SECONDARY};

/// Height of the status bar in logical pixels.
/// Includes 1px top separator line.
pub const STATUS_BAR_HEIGHT: f32 = 21.0;

/// Information needed to render the status bar.
pub struct StatusBarInfo<'a> {
    pub shell_label: &'a str,
    pub cwd: &'a str,
    pub cols: u16,
    pub rows: u16,
    /// Font to use for status bar monospace text.
    pub font: Font,
    /// Git branch name, if available.
    pub git_branch: Option<&'a str>,
}

/// Derive a friendly shell type label from a process name.
pub fn shell_label(process_name: &str) -> &'static str {
    let lower = process_name.to_ascii_lowercase();
    if lower.contains("pwsh") || lower.contains("powershell") {
        "PowerShell"
    } else if lower.contains("bash") {
        "Bash"
    } else if lower.contains("zsh") {
        "Zsh"
    } else if lower.contains("fish") {
        "Fish"
    } else if lower == "cmd" || lower.contains("cmd.exe") {
        "Command Prompt"
    } else if lower.contains("wsl") {
        "WSL"
    } else if lower.contains("sh") {
        "Shell"
    } else {
        "Terminal"
    }
}

/// Renders the bottom status bar.
pub fn view_status_bar<'a, M: Clone + 'a>(info: Option<StatusBarInfo<'_>>) -> Element<'a, M> {
    let (shell, cwd, dims, status_font, git_branch) = match info {
        Some(info) => (
            info.shell_label.to_string(),
            if info.cwd.is_empty() {
                String::new()
            } else {
                info.cwd.to_string()
            },
            format!("{}\u{00D7}{}", info.cols, info.rows),
            info.font,
            info.git_branch.map(|s| s.to_string()),
        ),
        None => (String::new(), String::new(), String::new(), Font::default(), None),
    };

    let shell_text = text(shell)
        .size(10)
        .color(TEXT_SECONDARY())
        .font(status_font);

    // Shell label styled as a small pill badge with subtle border and depth.
    let badge_bg = GHOST_HOVER();
    let badge_border = BORDER_VARIANT();
    let shell_badge = container(shell_text)
        .padding(Padding::from([1, 6]))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(badge_bg)),
            border: Border {
                color: badge_border,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
                offset: iced::Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            },
            ..container::Style::default()
        });

    let cwd_text = text(cwd).size(11).color(TEXT_SECONDARY()).font(status_font);

    // Dims text slightly brighter for quick readability.
    let dims_color = {
        let s = TEXT_SECONDARY();
        Color::from_rgba(
            (s.r * 1.2).min(1.0),
            (s.g * 1.2).min(1.0),
            (s.b * 1.2).min(1.0),
            s.a,
        )
    };
    let dims_text = text(dims).size(11).color(dims_color).font(status_font);

    let section_sep = |_theme: &iced::Theme| rule::Style {
        color: BORDER_VARIANT(),
        radius: 0.5.into(),
        fill_mode: rule::FillMode::Padded(2),
        snap: true,
    };

    // Git branch (if available)
    let branch_element: Option<Element<'a, M>> = git_branch.map(|branch| {
        let muted = TEXT_SECONDARY();
        let branch_color = Color::from_rgba(
            (muted.r * 0.8 + 0.2 * 0.9).min(1.0),
            (muted.g * 0.8 + 0.2 * 0.76).min(1.0),
            (muted.b * 0.8 + 0.2 * 0.48).min(1.0),
            muted.a,
        );
        row![
            text("\u{EA68}").font(Font {
                family: iced::font::Family::Name("codicon"),
                weight: iced::font::Weight::Normal,
                stretch: iced::font::Stretch::Normal,
                style: iced::font::Style::Normal,
            }).size(10).color(branch_color),
            text(branch).size(10).color(branch_color).font(status_font),
        ]
        .spacing(3)
        .align_y(iced::Alignment::Center)
        .into()
    });

    // Encoding and line-ending labels
    let label_color = {
        let s = TEXT_SECONDARY();
        Color::from_rgba(s.r * 0.85, s.g * 0.85, s.b * 0.85, s.a)
    };
    let utf8_text = text("UTF-8").size(10).color(label_color).font(status_font);
    let lf_text = text("LF").size(10).color(label_color).font(status_font);

    let mut content = row![
        container(shell_badge).padding(Padding::from([0, 8])),
        container(rule::vertical(1).style(section_sep))
            .height(Length::Fixed(12.0))
            .padding(Padding::from([0, 2])),
        container(cwd_text).padding(Padding::from([0, 4])),
    ]
    .align_y(iced::Alignment::Center);

    if let Some(branch_el) = branch_element {
        content = content
            .push(container(rule::vertical(1).style(section_sep))
                .height(Length::Fixed(12.0))
                .padding(Padding::from([0, 2])))
            .push(container(branch_el).padding(Padding::from([0, 4])));
    }

    content = content
        .push(Space::new().width(Length::Fill))
        .push(container(utf8_text).padding(Padding::from([0, 4])))
        .push(container(rule::vertical(1).style(section_sep))
            .height(Length::Fixed(12.0))
            .padding(Padding::from([0, 2])))
        .push(container(lf_text).padding(Padding::from([0, 4])))
        .push(container(rule::vertical(1).style(section_sep))
            .height(Length::Fixed(12.0))
            .padding(Padding::from([0, 2])))
        .push(container(dims_text).padding(Padding::from([0, 8])));

    let content = content.height(Length::Fixed(STATUS_BAR_HEIGHT));

    // Soft top separator — less harsh than full-opacity border.
    let status_sep_color = {
        let b = BORDER();
        Color::from_rgba(b.r, b.g, b.b, 0.45)
    };
    let separator = rule::horizontal(1).style(move |_theme| rule::Style {
        color: status_sep_color,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    });

    let bar = container(content)
        .width(Length::Fill)
        .height(Length::Fixed(STATUS_BAR_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(STATUS_BAR_BG())),
            ..container::Style::default()
        });

    column![separator, bar].width(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_label_maps_known_shells() {
        assert_eq!(shell_label("pwsh"), "PowerShell");
        assert_eq!(shell_label("PowerShell"), "PowerShell");
        assert_eq!(shell_label("bash"), "Bash");
        assert_eq!(shell_label("zsh"), "Zsh");
        assert_eq!(shell_label("cmd.exe"), "Command Prompt");
        assert_eq!(shell_label("fish"), "Fish");
        assert_eq!(shell_label("wsl"), "WSL");
        assert_eq!(shell_label("unknown"), "Terminal");
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
            font: Font::default(),
            git_branch: Some("main"),
        };
        let _el: Element<'_, Msg> = view_status_bar(Some(info));
    }

    #[test]
    fn status_bar_renders_without_info() {
        let _el: Element<'_, Msg> = view_status_bar(None);
    }
}
