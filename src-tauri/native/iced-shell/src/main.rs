use iced::window;

/// Geist Mono font family — bundled under SIL Open Font License.
pub mod fonts {
    use iced::Font;

    pub const REGULAR: &[u8] = include_bytes!("../fonts/GeistMono-Regular.ttf");
    pub const BOLD: &[u8] = include_bytes!("../fonts/GeistMono-Bold.ttf");
    pub const ITALIC: &[u8] = include_bytes!("../fonts/GeistMono-Italic.ttf");
    pub const BOLD_ITALIC: &[u8] = include_bytes!("../fonts/GeistMono-BoldItalic.ttf");

    /// The Geist Mono font for normal-weight text.
    pub const GEIST_MONO: Font = Font {
        family: iced::font::Family::Name("Geist Mono"),
        weight: iced::font::Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };
}

mod app;
mod claude_md_editor;
mod mcp_handler;
mod notification_state;
mod notifications;
mod scrollback_restore;
mod selection;
mod settings_dialog;
mod shortcuts_tab;
mod sidebar;
mod split_pane;
mod subscription;
mod tab_bar;
mod terminal_state;
mod theme;
mod title_bar;
mod workspace_state;
mod shell_picker;

mod confirm_dialog;
mod search;
mod scrollbar;
mod status_bar;
mod perf_overlay;
mod perf_stats;
mod url_detector;
mod terminal_context_menu;
mod whisper_ui;
mod quick_claude;
mod quick_claude_dialog;
mod session_persistence;
mod font_enumerator;
mod git_worktree;
mod keybinding_persistence;
mod phone_remote;
mod crash_handler;

use app::{GodlyApp, Message};

fn main() -> iced::Result {
    crash_handler::init();

    env_logger::init();
    log::info!(
        "Starting Godly Terminal (Native) — v{}",
        env!("GODLY_APP_VERSION"),
    );

    // Install a Win32 timer on the main thread that fires every second.
    // This generates WM_TIMER messages in the thread's message queue, which
    // keeps winit's event loop alive even when the window is minimized.
    // Without this, Iced stops polling all subscriptions and streams,
    // causing the terminal to freeze on restore.
    #[cfg(windows)]
    unsafe {
        extern "system" {
            fn SetTimer(
                hwnd: *mut std::ffi::c_void,
                id: usize,
                elapse: u32,
                callback: *const std::ffi::c_void,
            ) -> usize;
        }
        SetTimer(std::ptr::null_mut(), 0, 1000, std::ptr::null());
    }

    iced::application(boot, GodlyApp::update, GodlyApp::view)
        .title(GodlyApp::title)
        .subscription(GodlyApp::subscription)
        .antialiasing(true)
        .font(fonts::REGULAR)
        .font(fonts::BOLD)
        .font(fonts::ITALIC)
        .font(fonts::BOLD_ITALIC)
        .default_font(fonts::GEIST_MONO)
        .window(window::Settings {
            size: iced::Size::new(1200.0, 800.0),
            min_size: Some(iced::Size::new(400.0, 300.0)),
            decorations: false,
            ..Default::default()
        })
        .run()
}

fn boot() -> (GodlyApp, iced::Task<Message>) {
    app::diag::init();
    app::diag::log("BOOT: app starting");
    let mut app = GodlyApp::default();
    let task = app::initialize(&mut app);
    (app, task)
}
