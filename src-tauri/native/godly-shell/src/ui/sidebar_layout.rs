use super::widget::Rect;

pub const HEADER_PAD_X: f32 = 14.0;
pub const HEADER_PAD_TOP: f32 = 12.0;
pub const HEADER_PAD_BOTTOM: f32 = 4.0;
pub const LIST_PAD_X: f32 = 6.0;
pub const LIST_PAD_Y: f32 = 4.0;
pub const ITEM_PAD_X: f32 = 8.0;
pub const ITEM_PAD_Y: f32 = 7.0;
pub const ITEM_MARGIN_BOTTOM: f32 = 2.0;
pub const ROW_GAP_X: f32 = 8.0;
pub const SECONDARY_PAD_LEFT: f32 = 20.0;
pub const SECONDARY_MARGIN_TOP: f32 = 2.0;
pub const SESSION_NUMBER_MIN_WIDTH: f32 = 10.0;
pub const ACTIVE_BORDER_W: f32 = 3.0;
pub const HEADER_LABEL_FONT_PX: f32 = 12.0;
pub const HEADER_LIGHTNING_FONT_PX: f32 = 10.0;
pub const SESSION_NUMBER_FONT_PX: f32 = 12.0;
pub const SESSION_NAME_FONT_PX: f32 = 13.0;
pub const SESSION_SECONDARY_FONT_PX: f32 = 11.0;

#[derive(Debug, Clone, Copy)]
pub struct SessionStackItemSpec {
    pub has_secondary: bool,
}

#[derive(Debug, Clone)]
pub struct SidebarSessionItemLayout {
    pub outer: Rect,
    pub first_row: Rect,
    pub secondary_row: Option<Rect>,
}

#[derive(Debug, Clone)]
pub struct SidebarSessionLayout {
    pub header: Rect,
    pub header_content: Rect,
    pub list: Rect,
    pub items: Vec<SidebarSessionItemLayout>,
}

impl SidebarSessionLayout {
    pub fn item_rect(&self, index: usize) -> Option<Rect> {
        self.items.get(index).map(|item| item.outer)
    }

    pub fn items_bottom(&self) -> f32 {
        self.items
            .last()
            .map(|item| item.outer.bottom())
            .unwrap_or(self.list.y)
    }
}

pub fn compute_sidebar_session_layout(
    sidebar: Rect,
    scale: f32,
    items: &[SessionStackItemSpec],
) -> SidebarSessionLayout {
    let s = |value: f32| (value * scale).round();

    let header_content_h = s(HEADER_LABEL_FONT_PX.max(HEADER_LIGHTNING_FONT_PX));
    let header_h = s(HEADER_PAD_TOP) + header_content_h + s(HEADER_PAD_BOTTOM);
    let header = Rect {
        x: sidebar.x,
        y: sidebar.y,
        width: sidebar.width,
        height: header_h,
    };
    let header_content = Rect {
        x: sidebar.x + s(HEADER_PAD_X),
        y: sidebar.y + s(HEADER_PAD_TOP),
        width: (sidebar.width - s(HEADER_PAD_X * 2.0)).max(0.0),
        height: header_content_h,
    };
    let list = Rect {
        x: sidebar.x + s(LIST_PAD_X),
        y: header.bottom() + s(LIST_PAD_Y),
        width: (sidebar.width - s(LIST_PAD_X * 2.0)).max(0.0),
        height: (sidebar.height - header_h - s(LIST_PAD_Y * 2.0)).max(0.0),
    };

    let first_row_h = s(SESSION_NAME_FONT_PX.max(SESSION_NUMBER_FONT_PX.max(SESSION_SECONDARY_FONT_PX)));
    let secondary_row_h = s(SESSION_SECONDARY_FONT_PX);
    let mut item_y = list.y;
    let item_w = list.width;
    let mut item_layouts = Vec::with_capacity(items.len());

    for item in items {
        let item_h = s(ITEM_PAD_Y * 2.0)
            + first_row_h
            + if item.has_secondary {
                s(SECONDARY_MARGIN_TOP) + secondary_row_h
            } else {
                0.0
            };
        let outer = Rect {
            x: list.x,
            y: item_y,
            width: item_w,
            height: item_h,
        };
        let first_row = Rect {
            x: outer.x + s(ITEM_PAD_X),
            y: outer.y + s(ITEM_PAD_Y),
            width: (outer.width - s(ITEM_PAD_X * 2.0)).max(0.0),
            height: first_row_h,
        };
        let secondary_row = item.has_secondary.then(|| Rect {
            x: outer.x + s(ITEM_PAD_X + SECONDARY_PAD_LEFT),
            y: first_row.bottom() + s(SECONDARY_MARGIN_TOP),
            width: (outer.width - s(ITEM_PAD_X * 2.0 + SECONDARY_PAD_LEFT)).max(0.0),
            height: secondary_row_h,
        });
        item_layouts.push(SidebarSessionItemLayout {
            outer,
            first_row,
            secondary_row,
        });
        item_y = outer.bottom() + s(ITEM_MARGIN_BOTTOM);
    }

    SidebarSessionLayout {
        header,
        header_content,
        list,
        items: item_layouts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_stack_matches_web_spacing() {
        let layout = compute_sidebar_session_layout(
            Rect {
                x: 0.0,
                y: 36.0,
                width: 200.0,
                height: 400.0,
            },
            1.0,
            &[
                SessionStackItemSpec {
                    has_secondary: true,
                },
                SessionStackItemSpec {
                    has_secondary: true,
                },
                SessionStackItemSpec {
                    has_secondary: true,
                },
            ],
        );

        assert_eq!(layout.header.height, 28.0);
        assert_eq!(layout.header_content.x, 14.0);
        assert_eq!(layout.header_content.y, 48.0);
        assert_eq!(layout.items[0].outer.x, 6.0);
        assert_eq!(layout.items[0].outer.y, 68.0);
        assert_eq!(layout.items[0].outer.width, 188.0);
        assert_eq!(layout.items[0].outer.height, 40.0);
        assert_eq!(layout.items[1].outer.y, 110.0);
        assert_eq!(layout.items[0].first_row.y, 75.0);
        assert_eq!(
            layout.items[0].secondary_row.expect("secondary row").x,
            34.0
        );
        assert_eq!(
            layout.items[0].secondary_row.expect("secondary row").y,
            90.0
        );
    }

    #[test]
    fn compact_item_hides_secondary_row() {
        let layout = compute_sidebar_session_layout(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            },
            1.0,
            &[SessionStackItemSpec {
                has_secondary: false,
            }],
        );

        assert_eq!(layout.items[0].outer.height, 27.0);
        assert!(layout.items[0].secondary_row.is_none());
    }
}
