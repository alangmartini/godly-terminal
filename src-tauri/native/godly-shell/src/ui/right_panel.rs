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
        }
    }

    /// Advance hover animations. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.close_hover_anim
            .set(if self.close_hovered { 1.0 } else { 0.0 });
        self.close_hover_anim.tick(hl, dt)
    }

    pub fn build(&self, ui: &mut UiBuilder, panel: Rect, status: Rect, text: &UiTextRenderer) {
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

        let mut y = content_rect.y;

        // Poem title — web: display flex, gap 8, marginBottom 16
        // White dot (8px) + bold 15px text, color "#e6edf3", letterSpacing 0.3
        let dot_sz = s(8.0);
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
        y += ch * font_scale::PX15 + s(16.0); // marginBottom 16

        // Stanzas — web: marginBottom 18, lineHeight 1.7, fontSize 13,
        //                 color "#9198a1", fontFamily Georgia/serif italic,
        //                 letterSpacing 0.2, whiteSpace pre-wrap
        let stanza_ch = ch * font_scale::PX13; // fontSize 13
        let stanza_line_h = stanza_ch * 1.7; // lineHeight 1.7
        let stanza_gap = s(18.0); // marginBottom 18
        let stanza_fg: [f32; 4] = colors::FG_INACTIVE; // #9198a1

        for stanza in &self.stanzas {
            for line in stanza.split('\n') {
                if y + stanza_ch > content_rect.y + content_rect.height {
                    break;
                }
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
                y += stanza_line_h;
            }
            y += stanza_gap - stanza_line_h; // net gap between stanzas
        }

        // Footer divider + text — web: borderTop "1px solid #1a1d25",
        //     paddingTop 12, marginTop 8, color "#6e7681", fontSize 12
        if y + s(24.0) < content_rect.y + content_rect.height {
            y += s(8.0);
            ui.hline(content_rect.x, y, content_rect.width, 1.0, colors::BORDER);
            y += s(12.0);
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

        match event {
            MouseEvent::Move { x, y } => {
                self.close_hovered = close_rect.contains(x, y);
                None
            }
            MouseEvent::Press { x, y, .. } => {
                if close_rect.contains(x, y) {
                    Some(RightPanelAction::Close)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RightPanelAction {
    Close,
}
