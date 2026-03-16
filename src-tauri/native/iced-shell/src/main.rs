use iced::window;

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
mod url_detector;
mod terminal_context_menu;
mod whisper_ui;
mod quick_claude;
mod session_persistence;
mod keybinding_persistence;

use app::{GodlyApp, Message};

fn main() -> iced::Result {
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
