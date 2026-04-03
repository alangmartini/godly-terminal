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

    /// Grow the rect by `amount` on each side.
    pub fn expand(&self, amount: f32) -> Rect {
        Rect {
            x: self.x - amount,
            y: self.y - amount,
            width: self.width + amount * 2.0,
            height: self.height + amount * 2.0,
        }
    }

    /// Shrink the rect by `amount` on each side.
    pub fn shrink(&self, amount: f32) -> Rect {
        self.expand(-amount)
    }

    /// Inset by `h` horizontally and `v` vertically.
    pub fn inset(&self, h: f32, v: f32) -> Rect {
        Rect {
            x: self.x + h,
            y: self.y + v,
            width: (self.width - h * 2.0).max(0.0),
            height: (self.height - v * 2.0).max(0.0),
        }
    }

    /// Center point (cx, cy).
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Split horizontally at `offset` pixels from the left.
    pub fn split_h(&self, offset: f32) -> (Rect, Rect) {
        let left = Rect { x: self.x, y: self.y, width: offset, height: self.height };
        let right = Rect { x: self.x + offset, y: self.y, width: (self.width - offset).max(0.0), height: self.height };
        (left, right)
    }

    /// Split vertically at `offset` pixels from the top.
    pub fn split_v(&self, offset: f32) -> (Rect, Rect) {
        let top = Rect { x: self.x, y: self.y, width: self.width, height: offset };
        let bottom = Rect { x: self.x, y: self.y + offset, width: self.width, height: (self.height - offset).max(0.0) };
        (top, bottom)
    }

    /// Sub-rectangle at a pixel offset within this rect.
    pub fn sub(&self, x_off: f32, y_off: f32, w: f32, h: f32) -> Rect {
        Rect { x: self.x + x_off, y: self.y + y_off, width: w, height: h }
    }

    /// Right edge (x + width).
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (y + height).
    pub fn bottom(&self) -> f32 {
        self.y + self.height
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
