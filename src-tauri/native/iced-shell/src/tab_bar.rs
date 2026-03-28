use std::collections::HashMap;

use iced::widget::{button, column, container, mouse_area, row, rule, text, Space};
use iced::{Border, Color, Element, Font, Length, Padding};

use crate::horizontal_wheel::horizontal_wheel;
use crate::terminal_state::TerminalInfo;
use crate::theme::{
    ACCENT, BG_SECONDARY, BORDER_VARIANT, DANGER, GHOST_ACTIVE, GHOST_HOVER, TAB_ACTIVE_BG,
    TAB_INACTIVE_BG, TEXT_PRIMARY, TEXT_SECONDARY,
};

/// Height of the tab bar in logical pixels.
pub const TAB_BAR_HEIGHT: f32 = 36.0;
/// Duration of tab entry animation in milliseconds.
pub const TAB_ENTRY_DURATION_MS: u64 = 200;
/// Estimated max width for a tab during entry animation.
const TAB_ENTRY_MAX_WIDTH: f32 = 200.0;

const TAB_BUTTON_HEIGHT: f32 = 30.0;
const CLOSE_BUTTON_SIZE: f32 = 18.0;
const SEPARATOR_HEIGHT: f32 = 16.0;
const ACCENT_INDICATOR_HEIGHT: f32 = 3.0;

/// Truncate a label to at most `max_chars` characters, appending "..." if truncated.
fn truncate_label(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}\u{2026}")
    }
}

#[cfg(test)]
fn contains_ascii_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }

    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
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
    } else if contains_ascii_insensitive(trimmed, "node") {
        "ND"
    } else if contains_ascii_insensitive(trimmed, "python")
        || contains_ascii_insensitive(trimmed, "python3")
    {
        "PY"
    } else if contains_ascii_insensitive(trimmed, "vim")
        || contains_ascii_insensitive(trimmed, "nvim")
    {
        "VI"
    } else if contains_ascii_insensitive(trimmed, "ssh") {
        "SS"
    } else if trimmed.eq_ignore_ascii_case("git") || contains_ascii_insensitive(trimmed, "git.exe")
    {
        "GI"
    } else if contains_ascii_insensitive(trimmed, "ruby")
        || contains_ascii_insensitive(trimmed, "irb")
    {
        "RB"
    } else {
        "TM"
    };

    Some(label)
}

/// Returns a small icon glyph for the process, delegating to `TerminalInfo::tab_icon()`.
fn process_icon_glyph(terminal: &TerminalInfo) -> Option<&'static str> {
    if terminal.process_name.trim().is_empty() {
        return None;
    }
    Some(terminal.tab_icon())
}

/// Compute entry progress (0.0..=1.0) for each animating tab.
pub fn tab_entry_progress(
    entering_tabs: &HashMap<String, u64>,
    now_ms: u64,
) -> HashMap<String, f32> {
    entering_tabs
        .iter()
        .filter_map(|(id, &started_at)| {
            let elapsed = now_ms.saturating_sub(started_at);
            if elapsed >= TAB_ENTRY_DURATION_MS {
                None // animation complete
            } else {
                let t = elapsed as f32 / TAB_ENTRY_DURATION_MS as f32;
                // ease-out cubic
                let eased = 1.0 - (1.0 - t).powi(3);
                Some((id.clone(), eased))
            }
        })
        .collect()
}

/// Returns true if all entry animations have finished.
pub fn all_entries_finished(entering_tabs: &HashMap<String, u64>, now_ms: u64) -> bool {
    entering_tabs
        .values()
        .all(|&started_at| now_ms.saturating_sub(started_at) >= TAB_ENTRY_DURATION_MS)
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
    entry_progress: &HashMap<String, f32>,
    font: Font,
    on_tab_click: impl Fn(String) -> M + 'a,
    on_close: impl Fn(String) -> M + 'a,
    on_drag_start: impl Fn(String) -> M + 'a,
    on_drag_hover: impl Fn(String) -> M + 'a,
    on_context_toggle: impl Fn(String) -> M + 'a,
    on_drag_end: M,
    on_new: M,
    on_scroll: impl Fn(iced::widget::scrollable::Viewport) -> M + 'a,
) -> Element<'a, M> {
    let active_index = active_id.and_then(|id| terminals.iter().position(|term| term.id == id));
    let mut tabs = row![].spacing(0);

    for (index, &terminal) in terminals.iter().enumerate() {
        let is_active = active_id == Some(terminal.id.as_str());
        let bg = if is_active {
            TAB_ACTIVE_BG()
        } else {
            TAB_INACTIVE_BG()
        };
        let text_color = if is_active {
            TEXT_PRIMARY()
        } else {
            TEXT_SECONDARY()
        };

        let truncated = truncate_label(&terminal.tab_label(), 30);
        let label = text(truncated).size(13).font(font).color(text_color);
        let icon_glyph = process_icon_glyph(terminal).map(|glyph| {
            let icon_color = if is_active {
                ACCENT()
            } else {
                TEXT_SECONDARY()
            };
            text(glyph).size(14).color(icon_color)
        });

        let close_id = terminal.id.clone();
        // Inactive tabs: close button hidden until directly hovered.
        // Active tab: close button always visible.
        let close_btn = button(text("\u{00D7}").size(12).color(text_color))
            .on_press(on_close(close_id))
            .padding(0)
            .width(Length::Fixed(CLOSE_BUTTON_SIZE))
            .height(Length::Fixed(CLOSE_BUTTON_SIZE))
            .style(move |_theme, status| {
                let (bg_color, btn_text_color) = match status {
                    button::Status::Hovered => {
                        let d = DANGER();
                        (Color::from_rgba(d.r, d.g, d.b, 0.15), d)
                    }
                    button::Status::Pressed => {
                        let d = DANGER();
                        (Color::from_rgba(d.r, d.g, d.b, 0.25), d)
                    }
                    _ if is_active => (Color::TRANSPARENT, text_color),
                    _ => (Color::TRANSPARENT, Color::TRANSPARENT),
                };
                button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    text_color: btn_text_color,
                    border: Border {
                        radius: 999.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            });

        let mut tab_content = row![].spacing(6).align_y(iced::Alignment::Center);
        if let Some(icon) = icon_glyph {
            tab_content = tab_content.push(icon);
        }
        let tab_content = tab_content
            .push(container(label).padding(Padding::from([0, 1])))
            .push(close_btn);

        let ghost_hover_bg = GHOST_HOVER();
        let ghost_active_bg = GHOST_ACTIVE();
        // Active tab: blend TAB_ACTIVE_BG with a subtle accent tint.
        let active_bg = if is_active {
            let base = bg;
            let accent = ACCENT();
            Color::from_rgb(
                base.r * 0.85 + accent.r * 0.15,
                base.g * 0.85 + accent.g * 0.15,
                base.b * 0.85 + accent.b * 0.15,
            )
        } else {
            bg
        };
        let tab_btn = button(tab_content)
            .padding(Padding::from([4, 12]))
            .height(Length::Fixed(TAB_BUTTON_HEIGHT))
            .style(move |_theme, status| {
                let bg_color = if is_active {
                    active_bg
                } else {
                    match status {
                        button::Status::Hovered => ghost_hover_bg,
                        button::Status::Pressed => ghost_active_bg,
                        _ => bg,
                    }
                };
                // Active tab: rounded top corners only so it visually
                // connects to the terminal pane below.
                let radius: iced::border::Radius = if is_active {
                    iced::border::Radius::new(6.0)
                        .bottom_right(0.0)
                        .bottom_left(0.0)
                } else {
                    6.0.into()
                };
                button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    text_color,
                    border: Border {
                        radius,
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
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

        // Active tab accent indicator (2px colored line at bottom edge).
        let accent_color = if is_active {
            ACCENT()
        } else {
            Color::TRANSPARENT
        };
        let indicator = container(Space::new().width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fixed(ACCENT_INDICATOR_HEIGHT))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(accent_color)),
                ..container::Style::default()
            });

        let tab_column = column![indicator, container(tab_with_drag).height(Length::Fill),]
            .height(Length::Fixed(TAB_BAR_HEIGHT))
            .spacing(0);

        // Animate entry: clip max_width during the entry animation.
        if let Some(&progress) = entry_progress.get(&terminal.id) {
            let max_w = (TAB_ENTRY_MAX_WIDTH * progress).max(1.0);
            let clip_container = container(tab_column).max_width(max_w).clip(true);
            tabs = tabs.push(clip_container);
        } else {
            tabs = tabs.push(tab_column);
        }

        if separator_after_tab(index, terminals.len(), active_index) {
            let separator = container(rule::vertical(1).style(|_theme| rule::Style {
                color: BORDER_VARIANT(),
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: true,
            }))
            .height(Length::Fixed(SEPARATOR_HEIGHT))
            .padding(Padding::from([10, 4]));
            tabs = tabs.push(separator);
        }
    }

    let tabs_scroll = horizontal_wheel(
        iced::widget::scrollable(tabs)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::new()
                    .width(4)
                    .scroller_width(4),
            ))
            .on_scroll(on_scroll)
            .width(Length::Fill)
            .height(Length::Fixed(TAB_BAR_HEIGHT)),
    );

    // "+" button to add new terminals (ghost style).
    let new_btn = button(text("+").size(15).color(TEXT_SECONDARY()))
        .on_press(on_new)
        .padding(Padding::from([3, 10]))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(26.0))
        .style(|_theme, status| {
            let bg_color = match status {
                button::Status::Hovered => GHOST_HOVER(),
                button::Status::Pressed => GHOST_ACTIVE(),
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: TEXT_SECONDARY(),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
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
            background: Some(iced::Background::Color(BG_SECONDARY())),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use iced::Font;

    use super::{
        contains_ascii_insensitive, process_badge_label, process_icon_glyph, separator_after_tab,
        truncate_label, view_tab_bar,
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
        TabBarScrolled,
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
            needs_refetch: false,
            rows: 24,
            cols: 80,
            exited: false,
            exit_code: None,
            scrollback_offset: 0,
            total_scrollback: 0,
            workspace_id: None,
            custom_name: None,
            worktree_path: None,
            is_clone: false,
            cached_image_handle: None,
            last_grid_fingerprint: None,
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
        assert_eq!(process_badge_label("node"), Some("ND"));
        assert_eq!(process_badge_label("node.exe"), Some("ND"));
        assert_eq!(process_badge_label("python"), Some("PY"));
        assert_eq!(process_badge_label("python3"), Some("PY"));
        assert_eq!(process_badge_label("vim"), Some("VI"));
        assert_eq!(process_badge_label("nvim"), Some("VI"));
        assert_eq!(process_badge_label("ssh"), Some("SS"));
        assert_eq!(process_badge_label("git"), Some("GI"));
        assert_eq!(process_badge_label("git.exe"), Some("GI"));
        assert_eq!(process_badge_label("ruby"), Some("RB"));
        assert_eq!(process_badge_label("irb"), Some("RB"));
        assert_eq!(process_badge_label("some-custom-tool"), Some("TM"));
    }

    #[test]
    fn truncate_label_short_strings_unchanged() {
        assert_eq!(truncate_label("hello", 30), "hello");
        assert_eq!(truncate_label("", 30), "");
        assert_eq!(
            truncate_label("exactly 30 chars long padded!!", 30),
            "exactly 30 chars long padded!!"
        );
    }

    #[test]
    fn truncate_label_long_strings_truncated() {
        let long = "a]".repeat(20); // 40 chars
        let result = truncate_label(&long, 30);
        assert!(result.chars().count() <= 30);
        assert!(result.ends_with('\u{2026}'));
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
        let no_anim: HashMap<String, f32> = HashMap::new();

        let _ = view_tab_bar(
            &terminals,
            Some("t-1"),
            &no_anim,
            Font::default(),
            |_| TestMessage::TabClicked,
            |_| TestMessage::TabClosed,
            |_| TestMessage::TabDragStart,
            |_| TestMessage::TabDragHover,
            |_| TestMessage::TabContextToggle,
            TestMessage::TabDragEnd,
            TestMessage::NewTabRequested,
            |_| TestMessage::TabBarScrolled,
        );
    }

    #[test]
    fn view_tab_bar_handles_many_tabs() {
        let owned: Vec<TerminalInfo> = (0..40)
            .map(|index| sample_terminal(&format!("t-{index}")))
            .collect();
        let terminals: Vec<&TerminalInfo> = owned.iter().collect();
        let no_anim: HashMap<String, f32> = HashMap::new();

        let _ = view_tab_bar(
            &terminals,
            Some("t-0"),
            &no_anim,
            Font::default(),
            |_| TestMessage::TabClicked,
            |_| TestMessage::TabClosed,
            |_| TestMessage::TabDragStart,
            |_| TestMessage::TabDragHover,
            |_| TestMessage::TabContextToggle,
            TestMessage::TabDragEnd,
            TestMessage::NewTabRequested,
            |_| TestMessage::TabBarScrolled,
        );
    }

    #[test]
    fn view_tab_bar_handles_missing_process_badges() {
        let mut terminal = sample_terminal("t-1");
        terminal.process_name.clear();
        terminal.title = "Named tab".into();
        let terminals = vec![&terminal];
        let no_anim: HashMap<String, f32> = HashMap::new();

        let _ = view_tab_bar(
            &terminals,
            Some("t-1"),
            &no_anim,
            Font::default(),
            |_| TestMessage::TabClicked,
            |_| TestMessage::TabClosed,
            |_| TestMessage::TabDragStart,
            |_| TestMessage::TabDragHover,
            |_| TestMessage::TabContextToggle,
            TestMessage::TabDragEnd,
            TestMessage::NewTabRequested,
            |_| TestMessage::TabBarScrolled,
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

    #[test]
    fn tab_entry_progress_returns_eased_values() {
        use super::{all_entries_finished, tab_entry_progress, TAB_ENTRY_DURATION_MS};

        let mut entering = HashMap::new();
        entering.insert("t-1".to_string(), 1000u64);

        // Midpoint
        let progress = tab_entry_progress(&entering, 1000 + TAB_ENTRY_DURATION_MS / 2);
        let p = *progress.get("t-1").unwrap();
        assert!(p > 0.3 && p < 0.95, "midpoint progress={p}");

        // Not finished at midpoint
        assert!(!all_entries_finished(
            &entering,
            1000 + TAB_ENTRY_DURATION_MS / 2
        ));

        // Finished after full duration
        assert!(all_entries_finished(
            &entering,
            1000 + TAB_ENTRY_DURATION_MS
        ));

        // No entries returned when finished
        let progress = tab_entry_progress(&entering, 1000 + TAB_ENTRY_DURATION_MS);
        assert!(progress.is_empty());
    }

    #[test]
    fn process_icon_glyph_delegates_to_tab_icon() {
        let mut term = sample_terminal("t-1");

        term.process_name = "pwsh".into();
        assert_eq!(process_icon_glyph(&term), Some("\u{276F}")); // ❯

        term.process_name = "cmd.exe".into();
        assert_eq!(process_icon_glyph(&term), Some("\u{25BA}")); // ►

        term.process_name = "bash".into();
        assert_eq!(process_icon_glyph(&term), Some("\u{25B8}")); // ▸

        term.process_name = "ssh".into();
        assert_eq!(process_icon_glyph(&term), Some("\u{2192}")); // →

        term.process_name = "ruby".into();
        assert_eq!(process_icon_glyph(&term), Some("\u{25C8}")); // ◈

        term.process_name = "claude".into();
        assert_eq!(process_icon_glyph(&term), Some("\u{25C6}")); // ◆
    }

    #[test]
    fn process_icon_glyph_returns_none_for_empty() {
        let mut term = sample_terminal("t-1");
        term.process_name = String::new();
        assert_eq!(process_icon_glyph(&term), None);

        term.process_name = "   ".into();
        assert_eq!(process_icon_glyph(&term), None);
    }
}
