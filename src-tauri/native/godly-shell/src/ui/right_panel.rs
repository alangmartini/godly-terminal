//! Right panel: contextual detail/content panel.
//!
//! Follows the web reference layout: header with title + close button,
//! scrollable content area with poem, and a small status bar at the bottom.

use super::anim::{self, lerp_color, Anim};
use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::widget::{MouseEvent, Rect};

pub struct RightPanel {
    /// Whether the right panel is currently shown.
    pub visible: bool,
    /// Header title text (web: "one more").
    pub title: String,
    /// Poem title (web: "The Gardener of Broken Things").
    pub poem_title: String,
    /// Poem stanzas — each entry is a multi-line stanza.
    pub stanzas: Vec<String>,
    /// Footer text below the poem.
    pub footer: String,
    // Close button hover animation
    close_hover_anim: Anim,
    close_hovered: bool,
    /// Current scroll offset (pixels scrolled from top).
    scroll_offset: f32,
    /// Total height of all content (for computing scroll max).
    total_content_height: f32,
    /// Scrollbar thumb hover animation (0=idle, 1=hovered).
    scrollbar_hover_anim: Anim,
    scrollbar_hovered: bool,
}

impl RightPanel {
    pub fn new() -> Self {
        Self {
            visible: true,
            title: "one more".into(),
            poem_title: "The Gardener of Broken Things".into(),
            stanzas: vec![
                "I keep a workshop in my chest\nwhere bent and rusted hours collect,\nwhere Mondays that did not go well\nsit next to plans I didn\u{2019}t protect.".into(),
                "There\u{2019}s a drawer of almost-good-enough,\na shelf of words I should have said,\na box of mornings lost to doubt,\na jar of thoughts I overfed.".into(),
                "But I have learned \u{2014} not all at once,\nnot in a flash of brilliant light,\nbut slowly, like a vine that climbs\na wall it cannot see at night \u{2014}".into(),
                "that broken things still hold their shape.\nA cracked cup knows what it can pour.\nA fraying rope still understands\nthe weight it used to carry before.".into(),
                "So I sit down with careful hands\nand turn each piece against the light,\nnot asking it to be brand new,\nbut asking it to feel less tight.".into(),
                "I oil the hinge of an old regret.\nI sand the edge of a clumsy year.\nI don\u{2019}t rebuild \u{2014} I just make room\nfor what was always living here.".into(),
                "Some people throw their damage out,\nreplace it all with polished chrome.\nBut I prefer the dents and scuffs \u{2014}\nthey\u{2019}re how I recognize my home.".into(),
                "The workshop hums. The lantern sways.\nI mend what I can mend, and then\nI set the broken clock to now\nand let the whole thing start again.".into(),
            ],
            footer: "Hope you enjoyed that one too.".into(),
            close_hover_anim: Anim::default(),
            close_hovered: false,
            scroll_offset: 0.0,
            total_content_height: 0.0,
            scrollbar_hover_anim: Anim::default(),
            scrollbar_hovered: false,
        }
    }

    /// Advance hover animations. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.close_hover_anim
            .set(if self.close_hovered { 1.0 } else { 0.0 });
        let a = self.close_hover_anim.tick(hl, dt);
        self.scrollbar_hover_anim
            .set(if self.scrollbar_hovered { 1.0 } else { 0.0 });
        let b = self.scrollbar_hover_anim.tick(hl, dt);
        a || b
    }

    pub fn build(&mut self, ui: &mut UiBuilder, panel: Rect, status: Rect, text: &UiTextRenderer) {
        if !self.visible || panel.width < 1.0 {
            return;
        }

        let s = |v: f32| text.s(v);
        let ch = text.cell_height;

        // Panel background — web: backgroundColor "#0b0d12"
        ui.fill(panel, colors::BG_DARK);

        // Left border — web: borderLeft "1px solid #1a1d25"
        ui.vline(panel.x, panel.y, panel.height, 1.0, colors::BORDER);

        // --- Header ---
        // Web: padding "10px 14px", borderBottom "1px solid #1a1d25",
        //      display flex, gap 8, fontSize 12
        let header_h = s(36.0);
        let header = Rect {
            x: panel.x,
            y: panel.y,
            width: panel.width,
            height: header_h,
        };

        // Header bottom border — web: borderBottom "1px solid #1a1d25"
        ui.hline(
            header.x,
            header.bottom() - 1.0,
            header.width,
            1.0,
            colors::BORDER,
        );

        // Title text — web: color "#484f58" (STATUS_DEFAULT), fontSize 12
        let title_y = header.y + (header_h - ch * font_scale::PX12) / 2.0;
        let title = if self.title.is_empty() {
            "Panel"
        } else {
            &self.title
        };
        ui.text_ui_scaled(
            text,
            title,
            panel.x + s(14.0),
            title_y,
            colors::STATUS_DEFAULT, // #484f58
            colors::BG_DARK,
            font_scale::PX12,
        );

        // Close button (×) — web: color "#3b4048", fontSize 14
        let close_sz = ch;
        let close_x = panel.right() - close_sz - s(10.0);
        let close_y = header.y + (header_h - close_sz) / 2.0;
        let close_rect = Rect {
            x: close_x,
            y: close_y,
            width: close_sz,
            height: close_sz,
        };
        let close_t = self.close_hover_anim.value();
        let close_fg = lerp_color(
            colors::STATUS_PATH, // #3b4048
            colors::FG_SECONDARY,
            close_t,
        );
        let icon_t = (0.8 * text.scale).max(1.0);
        ui.icon_x(close_rect, s(7.0), icon_t, close_fg);

        // --- Content area ---
        // Web: flex 1, overflowY auto, padding "16px 20px"
        let content_y = header.bottom();
        let content_h = if status.height > 0.0 {
            (status.y - content_y).max(0.0)
        } else {
            (panel.bottom() - content_y).max(0.0)
        };
        let content_pad_x = s(20.0);
        let content_pad_y = s(16.0);
        let content_rect = Rect {
            x: panel.x + content_pad_x,
            y: content_y + content_pad_y,
            width: panel.width - content_pad_x * 2.0,
            height: content_h - content_pad_y,
        };

        // Compute total content height (all stanzas + title + footer)
        // so scroll_max can be derived as (total - visible).max(0.0).
        {
            let mut h = 0.0_f32;
            // Poem title height + marginBottom
            h += ch * font_scale::PX15 + s(16.0);
            // Stanzas
            let stanza_ch = ch * font_scale::PX13;
            let stanza_line_h = stanza_ch * 1.7;
            let stanza_gap = s(18.0);
            for stanza in &self.stanzas {
                let line_count = stanza.split('\n').count();
                h += stanza_line_h * line_count as f32;
                h += stanza_gap - stanza_line_h; // net gap between stanzas
            }
            // Footer: marginTop 8 + divider line + paddingTop 12 + text height
            h += s(8.0) + s(12.0) + ch * font_scale::PX12;
            self.total_content_height = h;
        }

        // Set clip to content area so scrolled content doesn't overflow.
        // Web: overflowY "auto" — clip at the content div boundary.
        let clip_rect = Rect {
            x: panel.x,
            y: content_y,
            width: panel.width,
            height: content_h,
        };
        ui.set_clip(clip_rect);

        // Apply scroll offset: shift content upward by scroll_offset pixels.
        let mut y = content_rect.y - self.scroll_offset;

        // Visibility check — text commands bypass the quad clip rect,
        // so we manually skip text draws that are fully outside the area.
        let vis_top = content_y;
        let vis_bot = content_y + content_h;
        let visible = |ly: f32, lh: f32| -> bool { ly + lh > vis_top && ly < vis_bot };

        // Poem title — web: display flex, gap 8, marginBottom 16
        // White dot (8px) + bold 15px text, color "#e6edf3", letterSpacing 0.3
        let dot_sz = s(8.0);
        let title_h = ch * font_scale::PX15;
        if visible(y, title_h) {
            let dot_y = y + (ch - dot_sz) / 2.0;
            ui.fill_rounded(
                Rect {
                    x: content_rect.x,
                    y: dot_y,
                    width: dot_sz,
                    height: dot_sz,
                },
                colors::FG_BRIGHT, // #e6edf3
                dot_sz / 2.0,
            );
            ui.text_ui_bold_scaled(
                text,
                &self.poem_title,
                content_rect.x + dot_sz + s(8.0),
                y,
                colors::FG_BRIGHT, // #e6edf3
                colors::BG_DARK,
                font_scale::PX15,
            ); // web: fontSize 15
            ui.set_last_letter_spacing(0.3); // web: letterSpacing 0.3
        }
        y += title_h + s(16.0); // marginBottom 16

        // Stanzas — web: marginBottom 18, lineHeight 1.7, fontSize 13,
        //                 color "#9198a1", fontFamily Georgia/serif italic,
        //                 letterSpacing 0.2, whiteSpace pre-wrap
        let stanza_ch = ch * font_scale::PX13; // fontSize 13
        let stanza_line_h = stanza_ch * 1.7; // lineHeight 1.7
        let stanza_gap = s(18.0); // marginBottom 18
        let stanza_fg: [f32; 4] = colors::FG_INACTIVE; // #9198a1

        for stanza in &self.stanzas {
            for line in stanza.split('\n') {
                if visible(y, stanza_line_h) {
                    ui.text_serif_italic_scaled(
                        text,
                        line,
                        content_rect.x,
                        y,
                        stanza_fg,
                        colors::BG_DARK,
                        font_scale::PX13,
                    );
                    ui.set_last_letter_spacing(0.2); // web: letterSpacing 0.2
                }
                y += stanza_line_h;
            }
            y += stanza_gap - stanza_line_h; // net gap between stanzas
        }

        // Footer divider + text — web: borderTop "1px solid #1a1d25",
        //     paddingTop 12, marginTop 8, color "#6e7681", fontSize 12
        y += s(8.0);
        if visible(y, 1.0) {
            ui.hline(content_rect.x, y, content_rect.width, 1.0, colors::BORDER);
        }
        y += s(12.0);
        let footer_h = ch * font_scale::PX12;
        if visible(y, footer_h) {
            ui.text_ui_scaled(
                text,
                &self.footer,
                content_rect.x,
                y,
                colors::FG_MUTED, // #6e7681
                colors::BG_DARK,
                font_scale::PX12,
            ); // web: fontSize 12
        }

        // Clear clip before rendering scrollbar and status bar.
        ui.clear_clip();

        // --- Scrollbar thumb ---
        // Web: ::-webkit-scrollbar { width: 6px }, thumb #2d333b, radius 3px
        // Only shown when content overflows the visible area.
        let scroll_max = (self.total_content_height - content_rect.height).max(0.0);
        if scroll_max > 0.0 {
            let track_w = s(6.0);
            let track_x = panel.right() - track_w - s(2.0);
            let track_y = content_y;
            let track_h = content_h;
            let radius = s(3.0);

            let visible_ratio = (content_rect.height / self.total_content_height).min(1.0);
            let thumb_h = (track_h * visible_ratio).max(s(20.0)).min(track_h);
            let scroll_progress = if scroll_max > 0.0 {
                self.scroll_offset / scroll_max
            } else {
                0.0
            };
            let thumb_y = track_y + scroll_progress * (track_h - thumb_h);
            let thumb_rect = Rect {
                x: track_x,
                y: thumb_y,
                width: track_w,
                height: thumb_h,
            };
            // Web: thumb #2d333b, thumbHover #3b4048
            let hover_t = self.scrollbar_hover_anim.value();
            let thumb_color = lerp_color(
                colors::BG_HOVER,    // #2d333b — normal
                colors::STATUS_PATH, // #3b4048 — hover
                hover_t,
            );
            ui.fill_rounded(thumb_rect, thumb_color, radius);
        }

        // --- Bottom status bar ---
        // Web: height 26, backgroundColor "#0c0e14", borderTop "1px solid #1a1d25",
        //      fontSize 11, color "#3b4048"
        if status.height > 0.0 {
            ui.fill(status, colors::BG_STATUS);
            ui.hline(status.x, status.y, status.width, 1.0, colors::BORDER);

            let status_ch = ch * font_scale::PX11;
            let status_y = status.y + (status.height - status_ch) / 2.0;
            let status_fg = colors::STATUS_PATH; // #3b4048
                                                 // Left: "}" brace
            ui.text_ui_scaled(
                text,
                "}",
                status.x + s(14.0), // web: padding "0 14px"
                status_y,
                status_fg,
                colors::BG_STATUS,
                font_scale::PX11,
            );
            // Right: "? for shortcuts"
            let hint = "? for shortcuts";
            let hint_w = text.text_width_ui_scaled(hint, font_scale::PX11);
            ui.text_ui_scaled(
                text,
                hint,
                status.right() - hint_w - s(14.0), // web: padding "0 14px"
                status_y,
                status_fg,
                colors::BG_STATUS,
                font_scale::PX11,
            );
        }
    }

    pub fn on_mouse(
        &mut self,
        event: MouseEvent,
        panel: Rect,
        text: &UiTextRenderer,
    ) -> Option<RightPanelAction> {
        if !self.visible || panel.width < 1.0 {
            return None;
        }

        let s = |v: f32| text.s(v);
        let ch = text.cell_height;
        let header_h = s(36.0);

        // Close button hit rect
        let close_sz = ch;
        let close_x = panel.right() - close_sz - s(10.0);
        let close_y = panel.y + (header_h - close_sz) / 2.0;
        let close_rect = Rect {
            x: close_x,
            y: close_y,
            width: close_sz,
            height: close_sz,
        };

        // Scrollbar hover zone: right edge of panel, within content area
        let content_y = panel.y + header_h;
        let content_h = panel.height - header_h;
        let sb_zone_w = s(16.0); // generous hit zone around 6px bar

        match event {
            MouseEvent::Move { x, y } => {
                self.close_hovered = close_rect.contains(x, y);
                // Check if mouse is near scrollbar (right edge of panel, in content area)
                let scroll_max = (self.total_content_height - content_h).max(0.0);
                self.scrollbar_hovered = scroll_max > 0.0
                    && x >= panel.right() - sb_zone_w
                    && x <= panel.right()
                    && y >= content_y
                    && y <= content_y + content_h;
                None
            }
            MouseEvent::Press { x, y, .. } => {
                if close_rect.contains(x, y) {
                    Some(RightPanelAction::Close)
                } else {
                    None
                }
            }
            MouseEvent::Scroll { delta } => {
                let visible_h = panel.height - s(36.0); // header height
                let scroll_max = (self.total_content_height - visible_h).max(0.0);
                self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, scroll_max);
                None
            }
            _ => None,
        }
    }

    /// Returns `true` when the mouse is over any interactive (clickable) element.
    pub fn wants_pointer_cursor(&self) -> bool {
        self.visible && self.close_hovered
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RightPanelAction {
    Close,
}
