use iced::window;

mod app;
mod claude_md_editor;
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
mod perf_overlay;
mod url_detector;
mod terminal_context_menu;
mod whisper_ui;

use app::{GodlyApp, Message};

fn main() -> iced::Result {
    env_logger::init();
    log::info!(
        "Starting Godly Terminal (Native) — contract v{}",
        godly_protocol::FRONTEND_CONTRACT_VERSION,
    );

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
    let mut app = GodlyApp::default();
    let task = app::initialize(&mut app);
    (app, task)
}
