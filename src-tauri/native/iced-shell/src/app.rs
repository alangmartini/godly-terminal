use iced::widget::{center, text};
use iced::Element;

#[derive(Default)]
pub struct GodlyApp;

#[derive(Debug, Clone)]
pub enum Message {}

impl GodlyApp {
    pub fn title(&self) -> String {
        format!(
            "Godly Terminal (Native) — contract v{}",
            godly_protocol::FRONTEND_CONTRACT_VERSION
        )
    }

    pub fn update(&mut self, _message: Message) {}

    pub fn view(&self) -> Element<'_, Message> {
        center(
            text("Godly Terminal — Native Frontend (Phase 0 placeholder)")
                .size(24),
        )
        .into()
    }
}
