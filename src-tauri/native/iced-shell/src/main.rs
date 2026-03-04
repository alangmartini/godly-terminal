use iced::window;

mod app;
mod notification_state;
mod selection;
mod settings_dialog;
mod shortcuts_tab;
mod sidebar;
mod split_pane;
mod subscription;
mod tab_bar;
mod terminal_state;
mod theme;
mod workspace_state;

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
            ..Default::default()
        })
        .run()
}

fn boot() -> (GodlyApp, iced::Task<Message>) {
    let mut app = GodlyApp::default();
    let task = app::initialize(&mut app);
    (app, task)
}
