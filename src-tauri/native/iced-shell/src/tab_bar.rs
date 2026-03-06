use iced::widget::{button, container, mouse_area, row, rule, text};
use iced::{Border, Color, Element, Length, Padding};

use crate::terminal_state::TerminalInfo;

/// Height of the tab bar in logical pixels.
pub const TAB_BAR_HEIGHT: f32 = 32.0;

// Colors
const TAB_BAR_BG: Color = Color::from_rgb(0.08, 0.08, 0.10);
const ACTIVE_TAB_BG: Color = Color::from_rgb(0.2, 0.2, 0.25);
const INACTIVE_TAB_BG: Color = Color::from_rgb(0.12, 0.12, 0.15);
const TAB_TEXT_COLOR: Color = Color::from_rgb(0.85, 0.85, 0.85);
const TAB_SEPARATOR_COLOR: Color = Color::from_rgba(0.55, 0.55, 0.62, 0.30);
const PROCESS_BADGE_BG: Color = Color::from_rgb(0.16, 0.16, 0.20);
const PROCESS_BADGE_ACTIVE_BG: Color = Color::from_rgb(0.25, 0.21, 0.14);
const PROCESS_BADGE_BORDER: Color = Color::from_rgba(0.72, 0.72, 0.82, 0.30);
const PROCESS_BADGE_TEXT: Color = Color::from_rgb(0.90, 0.90, 0.94);
const CLOSE_HOVER_BG: Color = Color::from_rgb(0.35, 0.15, 0.15);
const TAB_BUTTON_HEIGHT: f32 = 26.0;
const CLOSE_BUTTON_SIZE: f32 = 20.0;
const SEPARATOR_HEIGHT: f32 = 14.0;
const NEW_TAB_HOVER_BG: Color = Color::from_rgb(0.16, 0.16, 0.20);
const NEW_TAB_PRESSED_BG: Color = Color::from_rgb(0.19, 0.19, 0.24);
const NEW_TAB_BORDER_COLOR: Color = Color::from_rgba(0.60, 0.60, 0.72, 0.45);
const NEW_TAB_BG: Color = Color::from_rgb(0.13, 0.13, 0.17);
const NEW_TAB_IDLE_BORDER_COLOR: Color = Color::from_rgba(0.48, 0.48, 0.58, 0.32);
const NEW_TAB_TEXT_COLOR: Color = Color::from_rgb(0.92, 0.92, 0.96);

fn contains_ascii_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn process_badge_label(process_name: &str) -> Option<&'static str> {
    let trimmed = process_name.trim();
    if trimmed.is_empty() {
        return None;
    }

    let label = if contains_ascii_insensitive(trimmed, "claude") {
        "CC"
    } else if contains_ascii_insensitive(trimmed, "codex") {
        "CX"
    } else if contains_ascii_insensitive(trimmed, "pwsh")
        || contains_ascii_insensitive(trimmed, "powershell")
    {
        "PS"
    } else if trimmed.eq_ignore_ascii_case("cmd") || contains_ascii_insensitive(trimmed, "cmd.exe")
    {
        "CM"
    } else if contains_ascii_insensitive(trimmed, "wsl") {
        "WS"
    } else if contains_ascii_insensitive(trimmed, "bash")
        || contains_ascii_insensitive(trimmed, "zsh")
        || contains_ascii_insensitive(trimmed, "fish")
        || trimmed.eq_ignore_ascii_case("sh")
        || trimmed
            .get(trimmed.len().saturating_sub(3)..)
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case("/sh"))
    {
        "SH"
    } else {
        "TM"
    };

    Some(label)
}

fn separator_after_tab(index: usize, tab_count: usize, active_index: Option<usize>) -> bool {
    if tab_count <= 1 || index + 1 >= tab_count {
        return false;
    }

    match active_index {
        Some(active_index) => index != active_index && index + 1 != active_index,
        None => true,
    }
}

/// Renders the tab bar as a horizontal row of tab buttons.
///
/// This function is generic over the message type so it can be used
/// independently of any specific app `Message` enum.
pub fn view_tab_bar<'a, M: Clone + 'a>(
    terminals: &[&'a TerminalInfo],
    active_id: Option<&str>,
    on_tab_click: impl Fn(String) -> M + 'a,
    on_close: impl Fn(String) -> M + 'a,
    on_drag_start: impl Fn(String) -> M + 'a,
    on_drag_hover: impl Fn(String) -> M + 'a,
    on_context_toggle: impl Fn(String) -> M + 'a,
    on_drag_end: M,
    on_new: M,
) -> Element<'a, M> {
    let active_index = active_id.and_then(|id| terminals.iter().position(|term| term.id == id));
    let mut tabs = row![].spacing(0);

    for (index, &terminal) in terminals.iter().enumerate() {
        let is_active = active_id == Some(terminal.id.as_str());
        let bg = if is_active {
            ACTIVE_TAB_BG
        } else {
            INACTIVE_TAB_BG
        };

        let label = text(terminal.tab_label()).size(13).color(TAB_TEXT_COLOR);
        let process_badge = process_badge_label(&terminal.process_name).map(|badge| {
            let badge_bg = if is_active {
                PROCESS_BADGE_ACTIVE_BG
            } else {
                PROCESS_BADGE_BG
            };

            container(text(badge).size(10).color(PROCESS_BADGE_TEXT))
                .padding(Padding::from([2, 5]))
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(badge_bg)),
                    border: Border {
                        color: PROCESS_BADGE_BORDER,
                        width: 1.0,
                        radius: 999.0.into(),
                    },
                    ..container::Style::default()
                })
        });

        let close_id = terminal.id.clone();
        let close_btn = button(text("\u{00D7}").size(13).color(TAB_TEXT_COLOR))
            .on_press(on_close(close_id))
            .padding(0)
            .width(Length::Fixed(CLOSE_BUTTON_SIZE))
            .height(Length::Fixed(CLOSE_BUTTON_SIZE))
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

        let mut tab_content = row![].spacing(8).align_y(iced::Alignment::Center);
        if let Some(process_badge) = process_badge {
            tab_content = tab_content.push(process_badge);
        }
        let tab_content = tab_content
            .push(container(label).padding(Padding::from([0, 1])))
            .push(close_btn);

        let tab_btn = button(tab_content)
            .padding(Padding::from([4, 10]))
            .height(Length::Fixed(TAB_BUTTON_HEIGHT))
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: TAB_TEXT_COLOR,
                border: Border::default(),
                ..button::Style::default()
            });

        let click_id = terminal.id.clone();
        let hover_id = terminal.id.clone();
        let drag_start_id = terminal.id.clone();
        let context_toggle_id = terminal.id.clone();

        let tab_with_click = mouse_area(tab_btn)
            .on_enter(on_drag_hover(hover_id))
            .on_release(on_tab_click(click_id))
            .on_right_press(on_context_toggle(context_toggle_id));

        let tab_with_drag = mouse_area(tab_with_click)
            .on_press(on_drag_start(drag_start_id))
            .on_release(on_drag_end.clone());

        tabs = tabs.push(tab_with_drag);

        if separator_after_tab(index, terminals.len(), active_index) {
            let separator = container(rule::vertical(1).style(|_theme| rule::Style {
                color: TAB_SEPARATOR_COLOR,
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }))
            .height(Length::Fixed(SEPARATOR_HEIGHT))
            .padding(Padding::from([9, 4]));
            tabs = tabs.push(separator);
        }
    }

    let tabs_scroll = iced::widget::scrollable(tabs)
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::hidden(),
        ))
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT));

    // "+" button to add new terminals.
    let new_btn = button(text("+").size(14).color(NEW_TAB_TEXT_COLOR))
        .on_press(on_new)
        .padding(Padding::from([2, 10]))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(24.0))
        .style(|_theme, status| {
            let (bg_color, border_color, border_width) = match status {
                button::Status::Hovered => (NEW_TAB_HOVER_BG, NEW_TAB_BORDER_COLOR, 1.0),
                button::Status::Pressed => (NEW_TAB_PRESSED_BG, NEW_TAB_BORDER_COLOR, 1.0),
                _ => (NEW_TAB_BG, NEW_TAB_IDLE_BORDER_COLOR, 1.0),
            };
            button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: NEW_TAB_TEXT_COLOR,
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: 8.0.into(),
                },
                ..button::Style::default()
            }
        });

    let content = row![
        container(tabs_scroll).width(Length::Fill),
        container(new_btn)
            .padding(Padding::from([0, 6]))
            .height(Length::Fixed(TAB_BAR_HEIGHT))
    ]
    .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(TAB_BAR_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(TAB_BAR_BG)),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::{
        contains_ascii_insensitive, process_badge_label, separator_after_tab, view_tab_bar,
    };
    use crate::terminal_state::TerminalInfo;

    #[derive(Clone)]
    enum TestMessage {
        TabClicked,
        TabClosed,
        TabDragStart,
        TabDragHover,
        TabContextToggle,
        TabDragEnd,
        NewTabRequested,
    }

    fn sample_terminal(id: &str) -> TerminalInfo {
        TerminalInfo {
            id: id.to_string(),
            title: String::new(),
            process_name: "pwsh".to_string(),
            order: 0,
            grid: None,
            dirty: false,
            fetching: false,
            rows: 24,
            cols: 80,
            exited: false,
            exit_code: None,
            scrollback_offset: 0,
            total_scrollback: 0,
            workspace_id: None,
            custom_name: None,
        }
    }

    #[test]
    fn process_badge_label_maps_known_processes() {
        assert_eq!(process_badge_label("pwsh"), Some("PS"));
        assert_eq!(process_badge_label("PowerShell"), Some("PS"));
        assert_eq!(
            process_badge_label("C:/Program Files/PowerShell/pwsh.exe"),
            Some("PS")
        );
        assert_eq!(process_badge_label("cmd.exe"), Some("CM"));
        assert_eq!(process_badge_label("zsh"), Some("SH"));
        assert_eq!(process_badge_label("claude"), Some("CC"));
        assert_eq!(process_badge_label("codex"), Some("CX"));
        assert_eq!(process_badge_label("some-custom-tool"), Some("TM"));
    }

    #[test]
    fn process_badge_label_skips_empty_values() {
        assert_eq!(process_badge_label(""), None);
        assert_eq!(process_badge_label("   "), None);
    }

    #[test]
    fn contains_ascii_insensitive_matches_without_allocating() {
        assert!(contains_ascii_insensitive("PowerShell", "powershell"));
        assert!(contains_ascii_insensitive("C:/bin/CLAUDE.exe", "claude"));
        assert!(!contains_ascii_insensitive("terminal", "codex"));
    }

    #[test]
    fn view_tab_bar_accepts_context_toggle_callback() {
        let terminal = sample_terminal("t-1");
        let terminals = vec![&terminal];

        let _ = view_tab_bar(
            &terminals,
            Some("t-1"),
            |_| TestMessage::TabClicked,
            |_| TestMessage::TabClosed,
            |_| TestMessage::TabDragStart,
            |_| TestMessage::TabDragHover,
            |_| TestMessage::TabContextToggle,
            TestMessage::TabDragEnd,
            TestMessage::NewTabRequested,
        );
    }

    #[test]
    fn view_tab_bar_handles_many_tabs() {
        let owned: Vec<TerminalInfo> = (0..40)
            .map(|index| sample_terminal(&format!("t-{index}")))
            .collect();
        let terminals: Vec<&TerminalInfo> = owned.iter().collect();

        let _ = view_tab_bar(
            &terminals,
            Some("t-0"),
            |_| TestMessage::TabClicked,
            |_| TestMessage::TabClosed,
            |_| TestMessage::TabDragStart,
            |_| TestMessage::TabDragHover,
            |_| TestMessage::TabContextToggle,
            TestMessage::TabDragEnd,
            TestMessage::NewTabRequested,
        );
    }

    #[test]
    fn view_tab_bar_handles_missing_process_badges() {
        let mut terminal = sample_terminal("t-1");
        terminal.process_name.clear();
        terminal.title = "Named tab".into();
        let terminals = vec![&terminal];

        let _ = view_tab_bar(
            &terminals,
            Some("t-1"),
            |_| TestMessage::TabClicked,
            |_| TestMessage::TabClosed,
            |_| TestMessage::TabDragStart,
            |_| TestMessage::TabDragHover,
            |_| TestMessage::TabContextToggle,
            TestMessage::TabDragEnd,
            TestMessage::NewTabRequested,
        );
    }

    #[test]
    fn separator_hidden_for_last_tab() {
        assert!(!separator_after_tab(2, 3, Some(1)));
        assert!(!separator_after_tab(0, 1, Some(0)));
    }

    #[test]
    fn separator_hidden_adjacent_to_active_tab() {
        assert!(!separator_after_tab(0, 4, Some(1)));
        assert!(!separator_after_tab(1, 4, Some(1)));
        assert!(separator_after_tab(2, 4, Some(1)));
    }

    #[test]
    fn separator_shown_between_inactive_tabs_when_no_active_tab() {
        assert!(separator_after_tab(0, 3, None));
        assert!(separator_after_tab(1, 3, None));
        assert!(!separator_after_tab(2, 3, None));
    }
}
