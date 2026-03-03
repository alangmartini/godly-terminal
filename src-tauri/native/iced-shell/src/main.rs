use iced::window;

mod app;
use app::GodlyApp;

fn main() -> iced::Result {
    env_logger::init();
    log::info!(
        "Starting Godly Terminal (Native) — contract v{}",
        godly_protocol::FRONTEND_CONTRACT_VERSION,
    );

    iced::application(GodlyApp::title, GodlyApp::update, GodlyApp::view)
        .window(window::Settings {
            size: iced::Size::new(1200.0, 800.0),
            min_size: Some(iced::Size::new(400.0, 300.0)),
            ..Default::default()
        })
        .run_with(|| (GodlyApp::default(), iced::Task::none()))
}
