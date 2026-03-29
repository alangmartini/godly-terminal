/// Axis-aligned rectangle in pixel coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Actions produced by UI widget interactions.
#[derive(Debug, Clone)]
pub enum UiAction {
    NewTab,
    CloseTab(String),
    SwitchTab(String),
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    DragWindow,
}

/// Mouse event delivered to widgets.
#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Press { x: f32, y: f32 },
    Release { x: f32, y: f32 },
    Move { x: f32, y: f32 },
}
