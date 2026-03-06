use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme;

/// Shell type selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellChoice {
    PowerShell,
    Wsl { distribution: Option<String> },
    Custom { program: String, args: Vec<String> },
}

impl ShellChoice {
    pub fn label(&self) -> String {
        match self {
            Self::PowerShell => "PowerShell".to_string(),
            Self::Wsl { distribution: None } => "WSL (default)".to_string(),
            Self::Wsl {
                distribution: Some(d),
            } => format!("WSL: {}", d),
            Self::Custom { program, .. } => format!("Custom: {}", program),
        }
    }
}

/// Active tab in the shell picker dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPickerTab {
    PowerShell,
    Wsl,
    Custom,
}

impl ShellPickerTab {
    pub fn all() -> [Self; 3] {
        [Self::PowerShell, Self::Wsl, Self::Custom]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Wsl => "WSL",
            Self::Custom => "Custom",
        }
    }
}

/// Shell picker dialog state.
#[derive(Debug, Clone)]
pub struct ShellPickerState {
    pub visible: bool,
    pub tab: ShellPickerTab,
    pub custom_program: String,
    pub custom_args: String,
    pub wsl_distros: Vec<String>,
    pub selected_distro: Option<String>,
}

impl Default for ShellPickerState {
    fn default() -> Self {
        Self {
            visible: false,
            tab: ShellPickerTab::PowerShell,
            custom_program: String::new(),
            custom_args: String::new(),
            wsl_distros: Vec::new(),
            selected_distro: None,
        }
    }
}

impl ShellPickerState {
    pub fn open(&mut self) {
        self.visible = true;
        self.tab = ShellPickerTab::PowerShell;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.custom_program.clear();
        self.custom_args.clear();
        self.selected_distro = None;
    }

    pub fn current_choice(&self) -> ShellChoice {
        match self.tab {
            ShellPickerTab::PowerShell => ShellChoice::PowerShell,
            ShellPickerTab::Wsl => ShellChoice::Wsl {
                distribution: self.selected_distro.clone(),
            },
            ShellPickerTab::Custom => ShellChoice::Custom {
                program: self.custom_program.clone(),
                args: self
                    .custom_args
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
            },
        }
    }
}

/// AI tool mode for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiToolMode {
    #[default]
    None,
    Claude,
    Codex,
    Both,
}

impl AiToolMode {
    pub fn all() -> [Self; 4] {
        [Self::None, Self::Claude, Self::Codex, Self::Both]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Both => "Both",
        }
    }

    /// Icon text for sidebar display, if any.
    pub fn icon(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Claude => Some("\u{2728}"),  // sparkles
            Self::Codex => Some("\u{26A1}"),   // lightning
            Self::Both => Some("\u{1F680}"),   // rocket
        }
    }
}

/// Parse WSL distro list from `wsl -l -q` output.
pub fn parse_wsl_distros(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim().trim_matches('\0').trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Render the shell picker dialog as a modal overlay.
pub fn view_shell_picker<'a, M: Clone + 'a>(
    state: &'a ShellPickerState,
    on_tab_click: impl Fn(ShellPickerTab) -> M + 'a,
    on_distro_select: impl Fn(Option<String>) -> M + 'a,
    on_custom_program_changed: impl Fn(String) -> M + 'a,
    on_custom_args_changed: impl Fn(String) -> M + 'a,
    on_confirm: M,
    on_cancel: M,
) -> Element<'a, M> {
    let accent = theme::ACCENT();
    let border_color = theme::BORDER();
    let bg_secondary = theme::BG_SECONDARY();
    let text_active = theme::TEXT_ACTIVE();
    let text_primary = theme::TEXT_PRIMARY();
    let text_secondary = theme::TEXT_SECONDARY();
    let backdrop = theme::BACKDROP();

    // Build tab buttons (PowerShell / WSL / Custom)
    let mut tab_row = row![].spacing(6);
    for tab in ShellPickerTab::all() {
        let is_active = tab == state.tab;
        let tab_btn = button(text(tab.label()).size(13))
            .on_press(on_tab_click(tab))
            .padding(Padding::from([6, 12]))
            .style(move |_theme, _status| {
                let bg = if is_active {
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.22)
                } else {
                    Color::TRANSPARENT
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: if is_active { text_active } else { text_secondary },
                    border: Border {
                        color: if is_active { accent } else { border_color },
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..button::Style::default()
                }
            });
        tab_row = tab_row.push(tab_btn);
    }

    // Tab content
    let tab_content: Element<'a, M> = match state.tab {
        ShellPickerTab::PowerShell => column![
            text("Launch a new PowerShell terminal.")
                .size(13)
                .color(text_secondary),
            Space::new().height(8.0),
            text("Uses pwsh.exe if available, falls back to powershell.exe.")
                .size(12)
                .color(text_secondary),
        ]
        .spacing(4)
        .into(),
        ShellPickerTab::Wsl => {
            let mut items = column![text("Select a WSL distribution:")
                .size(13)
                .color(text_secondary),]
            .spacing(4);

            // Default distro option
            let is_selected = state.selected_distro.is_none();
            let default_btn = button(
                text("Default")
                    .size(13)
                    .color(if is_selected { text_active } else { text_primary }),
            )
            .on_press(on_distro_select(None))
            .padding(Padding::from([5, 10]))
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(if is_selected {
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.12)
                } else {
                    Color::TRANSPARENT
                })),
                text_color: text_primary,
                border: Border::default(),
                ..button::Style::default()
            });
            items = items.push(default_btn);

            for distro in &state.wsl_distros {
                let d2 = distro.clone();
                let is_selected = state.selected_distro.as_deref() == Some(distro.as_str());
                let btn = button(
                    text(distro.as_str())
                        .size(13)
                        .color(if is_selected { text_active } else { text_primary }),
                )
                .on_press(on_distro_select(Some(d2)))
                .padding(Padding::from([5, 10]))
                .width(Length::Fill)
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(if is_selected {
                        Color::from_rgba(accent.r, accent.g, accent.b, 0.12)
                    } else {
                        Color::TRANSPARENT
                    })),
                    text_color: text_primary,
                    border: Border::default(),
                    ..button::Style::default()
                });
                items = items.push(btn);
            }

            scrollable(items).height(Length::Fixed(200.0)).into()
        }
        ShellPickerTab::Custom => column![
            text("Program:").size(12).color(text_secondary),
            text_input("e.g. bash, zsh, cmd.exe", &state.custom_program)
                .on_input(on_custom_program_changed)
                .size(13)
                .padding(Padding::from([4, 8])),
            Space::new().height(8.0),
            text("Arguments (space-separated):")
                .size(12)
                .color(text_secondary),
            text_input("e.g. --login -i", &state.custom_args)
                .on_input(on_custom_args_changed)
                .size(13)
                .padding(Padding::from([4, 8])),
        ]
        .spacing(4)
        .into(),
    };

    // Footer buttons
    let cancel_btn = button(text("Cancel").size(13))
        .on_press(on_cancel)
        .padding(Padding::from([6, 16]));
    let confirm_btn = button(text("Create").size(13).color(text_active))
        .on_press(on_confirm)
        .padding(Padding::from([6, 16]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(accent)),
            text_color: Color::WHITE,
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    let footer = row![Space::new().width(Length::Fill), cancel_btn, confirm_btn].spacing(8);

    // Compose dialog
    let dialog_content = column![
        text("New Terminal").size(16).color(text_active),
        Space::new().height(8.0),
        tab_row,
        Space::new().height(12.0),
        container(tab_content)
            .width(Length::Fill)
            .height(Length::Fill),
        Space::new().height(12.0),
        footer,
    ]
    .spacing(4);

    let dialog = container(dialog_content)
        .padding(Padding::from([16, 20]))
        .width(Length::Fixed(420.0))
        .height(Length::Fixed(380.0))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_secondary)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            },
            ..container::Style::default()
        });

    // Backdrop + centered dialog
    container(iced::widget::center(dialog))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(backdrop)),
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wsl_distros_basic() {
        let output = "Ubuntu\nDebian\nkali-linux\n";
        let distros = parse_wsl_distros(output);
        assert_eq!(distros, vec!["Ubuntu", "Debian", "kali-linux"]);
    }

    #[test]
    fn parse_wsl_distros_with_null_bytes() {
        // wsl -l -q on some systems outputs UTF-16 with null bytes
        let output = "U\0b\0u\0n\0t\0u\0\n\0";
        let distros = parse_wsl_distros(output);
        assert!(!distros.is_empty());
    }

    #[test]
    fn parse_wsl_distros_empty() {
        let distros = parse_wsl_distros("");
        assert!(distros.is_empty());
    }

    #[test]
    fn shell_choice_labels() {
        assert_eq!(ShellChoice::PowerShell.label(), "PowerShell");
        assert!(ShellChoice::Wsl {
            distribution: None
        }
        .label()
        .contains("default"));
        assert!(ShellChoice::Wsl {
            distribution: Some("Ubuntu".into())
        }
        .label()
        .contains("Ubuntu"));
        assert!(ShellChoice::Custom {
            program: "bash".into(),
            args: vec![]
        }
        .label()
        .contains("bash"));
    }

    #[test]
    fn current_choice_powershell() {
        let state = ShellPickerState::default();
        assert_eq!(state.current_choice(), ShellChoice::PowerShell);
    }

    #[test]
    fn current_choice_wsl() {
        let mut state = ShellPickerState::default();
        state.tab = ShellPickerTab::Wsl;
        state.selected_distro = Some("Ubuntu".into());
        match state.current_choice() {
            ShellChoice::Wsl { distribution } => {
                assert_eq!(distribution, Some("Ubuntu".into()));
            }
            _ => panic!("expected WSL"),
        }
    }

    #[test]
    fn current_choice_custom() {
        let mut state = ShellPickerState::default();
        state.tab = ShellPickerTab::Custom;
        state.custom_program = "bash".into();
        state.custom_args = "--login -i".into();
        match state.current_choice() {
            ShellChoice::Custom { program, args } => {
                assert_eq!(program, "bash");
                assert_eq!(args, vec!["--login", "-i"]);
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn ai_tool_mode_labels() {
        for mode in AiToolMode::all() {
            assert!(!mode.label().is_empty());
        }
    }

    #[test]
    fn ai_tool_mode_icons() {
        assert!(AiToolMode::None.icon().is_none());
        assert!(AiToolMode::Claude.icon().is_some());
        assert!(AiToolMode::Codex.icon().is_some());
        assert!(AiToolMode::Both.icon().is_some());
    }

    #[test]
    fn open_close_state() {
        let mut state = ShellPickerState::default();
        assert!(!state.visible);
        state.open();
        assert!(state.visible);
        state.close();
        assert!(!state.visible);
    }
}
