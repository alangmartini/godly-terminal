//! Left sidebar: session list with names, active indicator, and new session button.

use super::anim::{self, Anim, AnimVec, lerp_color, lerp};
use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const HEADER_HEIGHT: f32 = 30.0;
const ITEM_HEIGHT: f32 = 46.0;       // web: padding 7+7 + name 16 + gap 2 + branch 13 = ~45px
const ITEM_HEIGHT_COMPACT: f32 = 32.0; // web: padding 7+7 + name 16 = ~30px
const ITEM_PADDING_H: f32 = 14.0;
const ACTIVE_INDICATOR_W: f32 = 3.5;
const BOTTOM_PANEL_HEIGHT: f32 = 160.0;

/// Accent colors for each session position (same cycle as tab bar).
const SESSION_ACCENTS: &[[f32; 4]] = &[
    colors::ACCENT_BLUE,
    colors::ACCENT_GREEN,
    colors::ACCENT_PEACH,
    colors::ACCENT_MAUVE,
    colors::ACCENT_RED,
];

pub struct SidebarItem {
    pub id: String,
    pub label: String,
    pub number: usize,
    pub branch: String,
    pub description: String,
    pub active: bool,
    /// Shell type displayed as a small pill badge (e.g. "pwsh", "bash", "zsh").
    pub shell_type: String,
    /// Working directory shown as secondary text when description is empty.
    pub cwd: String,
    /// Relative timestamp label (e.g. "5m", "2h", "3d"). Displayed right-aligned
    /// on the second line in very muted text.
    pub timestamp: String,
}

pub struct AgentItem {
    pub icon: &'static str,
    pub name: String,
    pub task: String,
    pub status: AgentStatus,
}

pub enum AgentStatus {
    Running,
    Waiting,
    Stopped,
}

pub struct Sidebar {
    pub items: Vec<SidebarItem>,
    pub agents: Vec<AgentItem>,
    pub hovered_index: Option<usize>,
    pub hovered_agent: Option<usize>,
    pub hovered_new: bool,
    pub hovered_settings: bool,
    // Smooth animation state
    item_hover_anim: AnimVec,
    active_anim: AnimVec,
    agent_hover_anim: AnimVec,
    new_btn_anim: Anim,
    settings_anim: Anim,
    /// Continuous phase for ambient breathing glow on active elements.
    /// Incremented each frame; `sin(glow_phase)` modulates glow intensity.
    glow_phase: f32,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            items: vec![
                SidebarItem {
                    id: "default".into(),
                    label: "plane".into(),
                    number: 1,
                    branch: "fix/pd".into(),
                    description: "fix/pdf-export...".into(),
                    active: true,
                    shell_type: "pwsh".into(),
                    cwd: "~/dev/plane".into(),
                    timestamp: "5m".into(),
                },
                SidebarItem {
                    id: "session-2".into(),
                    label: "opensessions".into(),
                    number: 2,
                    branch: "main".into(),
                    description: "".into(),
                    active: false,
                    shell_type: "pwsh".into(),
                    cwd: "~/work/opensessions".into(),
                    timestamp: "2h".into(),
                },
                SidebarItem {
                    id: "session-3".into(),
                    label: "quiver".into(),
                    number: 3,
                    branch: "main".into(),
                    description: "".into(),
                    active: false,
                    shell_type: "bash".into(),
                    cwd: "~/dev/quiver".into(),
                    timestamp: "1d".into(),
                },
                SidebarItem {
                    id: "session-4".into(),
                    label: "godly-terminal".into(),
                    number: 4,
                    branch: "feat/sh".into(),
                    description: "".into(),
                    active: false,
                    shell_type: "pwsh".into(),
                    cwd: "~/dev/godly-terminal".into(),
                    timestamp: "3d".into(),
                },
            ],
            item_hover_anim: AnimVec::default(),
            active_anim: AnimVec::default(),
            agent_hover_anim: AnimVec::default(),
            new_btn_anim: Anim::default(),
            settings_anim: Anim::default(),
            agents: vec![
                AgentItem {
                    icon: "\u{2191}",
                    name: "amp".into(),
                    task: "Verify and clean README".into(),
                    status: AgentStatus::Running,
                },
                AgentItem {
                    icon: "\u{2193}",
                    name: "anu".into(),
                    task: "Verify README against...".into(),
                    status: AgentStatus::Stopped,
                },
                AgentItem {
                    icon: "\u{25CB}",
                    name: "claude-code".into(),
                    task: "cycle # gol d remov...".into(),
                    status: AgentStatus::Waiting,
                },
            ],
            hovered_index: None,
            hovered_agent: None,
            hovered_new: false,
            hovered_settings: false,
            glow_phase: 0.0,
        }
    }

    /// Advance all hover animations. `dt` = seconds since last frame. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.item_hover_anim.ensure_len(self.items.len());
        for i in 0..self.items.len() {
            self.item_hover_anim.set(i, if self.hovered_index == Some(i) { 1.0 } else { 0.0 });
        }
        self.agent_hover_anim.ensure_len(self.agents.len());
        for i in 0..self.agents.len() {
            self.agent_hover_anim.set(i, if self.hovered_agent == Some(i) { 1.0 } else { 0.0 });
        }
        self.new_btn_anim.set(if self.hovered_new { 1.0 } else { 0.0 });
        self.settings_anim.set(if self.hovered_settings { 1.0 } else { 0.0 });
        self.active_anim.ensure_len(self.items.len());
        for i in 0..self.items.len() {
            self.active_anim.set(i, if self.items[i].active { 1.0 } else { 0.0 });
        }
        let mut animating = false;
        animating |= self.active_anim.tick(hl, dt);
        animating |= self.item_hover_anim.tick(hl, dt);
        animating |= self.agent_hover_anim.tick(hl, dt);
        animating |= self.new_btn_anim.tick(hl, dt);
        animating |= self.settings_anim.tick(hl, dt);

        // Ambient breathing glow: ~3.5s period, frame-rate independent.
        let has_active = self.items.iter().any(|i| i.active)
            || self.agents.iter().any(|a| matches!(a.status, AgentStatus::Running));
        if has_active {
            self.glow_phase += dt * std::f32::consts::TAU / 3.5;
            if self.glow_phase > std::f32::consts::TAU { self.glow_phase -= std::f32::consts::TAU; }
            animating = true;
        }

        animating
    }

    fn item_height_for(&self, index: usize, scale: f32) -> f32 {
        let item = &self.items[index];
        // Web reference shows branch on second line for all items.
        // Two-line if there's a branch or description.
        if item.branch.is_empty() && item.description.is_empty() {
            (ITEM_HEIGHT_COMPACT * scale).round()
        } else {
            (ITEM_HEIGHT * scale).round()
        }
    }

    fn items_y_offset(&self, up_to: usize, scale: f32) -> f32 {
        let mut y = 0.0;
        for i in 0..up_to.min(self.items.len()) {
            y += self.item_height_for(i, scale);
        }
        y
    }

    fn item_rect(&self, index: usize, sidebar: Rect, scale: f32) -> Rect {
        let header_h = (HEADER_HEIGHT * scale).round();
        let h = self.item_height_for(index, scale);
        Rect {
            x: sidebar.x,
            y: sidebar.y + header_h + self.items_y_offset(index, scale),
            width: sidebar.width,
            height: h,
        }
    }

    fn settings_rect(&self, sidebar: Rect, scale: f32) -> Rect {
        let pad_h = (ITEM_PADDING_H * scale).round();
        let settings_h = (28.0 * scale).round();
        Rect {
            x: sidebar.x + pad_h,
            y: sidebar.bottom() - settings_h,
            width: sidebar.width - pad_h * 2.0,
            height: settings_h,
        }
    }

    fn new_button_rect(&self, sidebar: Rect, scale: f32) -> Rect {
        let header_h = (HEADER_HEIGHT * scale).round();
        let pad_h = (ITEM_PADDING_H * scale).round();
        let compact_h = (ITEM_HEIGHT_COMPACT * scale).round();
        let y = sidebar.y + header_h + self.items_y_offset(self.items.len(), scale) + (4.0 * scale).round();
        Rect {
            x: sidebar.x + pad_h,
            y,
            width: sidebar.width - pad_h * 2.0,
            height: compact_h,
        }
    }

    pub fn build(&self, ui: &mut UiBuilder, sidebar: Rect, text: &UiTextRenderer) {
        if sidebar.width < 1.0 { return; }

        let s = |v: f32| text.s(v);
        let cw = text.cell_width;
        let ch = text.cell_height;
        let header_h = s(HEADER_HEIGHT);
        let item_h = s(ITEM_HEIGHT);
        let pad_h = s(ITEM_PADDING_H);
        let indicator_w = s(ACTIVE_INDICATOR_W);
        let _bottom_panel_h = s(BOTTOM_PANEL_HEIGHT);
        let text_y_off = |area_h: f32| (area_h - ch) / 2.0;

        // Sidebar background — flat fill matching web reference
        // (backgroundColor: "#0b0d12").
        ui.fill(sidebar, colors::BG_DARK);

        // (Convexity gradient removed — flat surface matches Zed/VS Code restraint)

        // Right border separator — solid 1px hairline matching web reference
        // (borderRight: "1px solid #1a1d25").
        ui.vline(sidebar.right() - 1.0, sidebar.y, sidebar.height, 1.0, colors::BORDER);
        // (Web reference uses clean flat sidebar with no shadow effects)

        // "Sessions {count}" header — inline mixed-case matching web reference
        let header_rect = Rect {
            x: sidebar.x,
            y: sidebar.y,
            width: sidebar.width,
            height: header_h,
        };
        // Web reference: "Sessions 3" inline, no disclosure triangle, no pill badge
        let header_label = format!("Sessions {}", self.items.len());
        let header_text_x = header_rect.x + pad_h;
        ui.text_ui(
            text,
            &header_label,
            header_text_x,
            header_rect.y + text_y_off(header_h),
            [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.65],
            colors::BG_DARK,
        );

        // Layout: [pad][num][gap][name...][pad]
        // Matches web reference: plain text IDs + name, branch on second line
        let num_x = sidebar.x + pad_h;
        let name_x = num_x + cw * 2.0 + s(4.0);
        let ui_cw = if text.ui_avg_advance > 0.0 { text.ui_avg_advance } else { cw * 0.75 };

        // Truncate a string to fit within max_w pixels using exact per-glyph widths.
        // Returns the truncated string with ellipsis if needed.
        let truncate_to_width = |s: &str, max_w: f32, text: &UiTextRenderer| -> String {
            let full_w = text.text_width_ui(s);
            if full_w <= max_w { return s.to_string(); }
            let ellipsis_w = text.text_width_ui("\u{2026}");
            let target_w = max_w - ellipsis_w;
            let mut w = 0.0f32;
            let mut end = 0;
            for (i, ch) in s.char_indices() {
                let ch_w = text.text_width_ui(&s[i..i + ch.len_utf8()]);
                if w + ch_w > target_w { break; }
                w += ch_w;
                end = i + ch.len_utf8();
            }
            format!("{}\u{2026}", &s[..end])
        };
        let line1_y_off = s(7.0); // web: padding-top 7px
        let line2_y_off = line1_y_off + ch + s(2.0); // web: marginTop 2px below name

        // Session items (dynamic height per item)
        let compact_h = s(ITEM_HEIGHT_COMPACT);
        for (i, item) in self.items.iter().enumerate() {
            let this_item_h = if item.description.is_empty() { compact_h } else { item_h };
            let rect = Rect {
                x: sidebar.x,
                y: sidebar.y + header_h + self.items_y_offset(i, text.scale),
                width: sidebar.width,
                height: this_item_h,
            };

            // Hover background (rounded, animated)
            let item_radius = s(6.0);
            let inset_rect = Rect {
                x: rect.x + s(6.0),
                y: rect.y + s(2.0),
                width: rect.width - s(12.0),
                height: rect.height - s(4.0),
            };
            let hover_t = self.item_hover_anim.get(i);
            let active_t = self.active_anim.get(i);
            let session_accent = SESSION_ACCENTS[i % SESSION_ACCENTS.len()];

            // Inactive hover state (fades out as active_t increases)
            if active_t < 0.995 {
                let inv_active = 1.0 - active_t;
                if hover_t > 0.005 {
                    // Soft glow shadow behind hovered item for "lift" effect
                    let glow_rect = Rect {
                        x: inset_rect.x - s(2.0),
                        y: inset_rect.y - s(1.0),
                        width: inset_rect.width + s(4.0),
                        height: inset_rect.height + s(2.0),
                    };
                    ui.fill_shadow(glow_rect,
                        [session_accent[0], session_accent[1], session_accent[2], 0.06 * hover_t * inv_active],
                        item_radius + s(2.0), s(8.0));
                    let hover_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], lerp(0.12, 0.4, hover_t) * inv_active];
                    let hover_top = [
                        colors::BG_HOVER[0] * lerp(1.0, 1.08, hover_t),
                        colors::BG_HOVER[1] * lerp(1.0, 1.08, hover_t),
                        colors::BG_HOVER[2] * lerp(1.0, 1.08, hover_t),
                        colors::BG_HOVER[3] * hover_t * inv_active,
                    ];
                    let hover_bot = [colors::BG_HOVER[0], colors::BG_HOVER[1], colors::BG_HOVER[2], colors::BG_HOVER[3] * hover_t * inv_active];
                    ui.fill_rounded_gradient(inset_rect, hover_top, hover_bot, item_radius);
                    ui.stroke_rounded(inset_rect, item_radius, 0.5, hover_border);
                }
            }

            // Active state — flat background + 3px left accent bar
            // Web reference always uses #6366f1 (indigo/ACCENT_BLUE) for active border
            if active_t > 0.005 {
                // Flat active background (#171b24)
                let active_bg = lerp_color(colors::BG_DARK, colors::BG_ACTIVE, active_t);
                ui.fill_rounded(inset_rect, active_bg, item_radius);

                // 3px flat left accent bar — always indigo, matching web
                let indicator_rect = Rect {
                    x: rect.x + s(3.0),
                    y: rect.y + s(7.0),
                    width: s(3.0),
                    height: rect.height - s(14.0),
                };
                ui.fill_rounded(indicator_rect,
                    [colors::ACCENT_BLUE[0], colors::ACCENT_BLUE[1], colors::ACCENT_BLUE[2], active_t],
                    s(1.5));
            }

            // Text y position: centered for compact, top-aligned for two-line
            let is_two_line = !item.branch.is_empty() || !item.description.is_empty();
            let text_y = if is_two_line {
                rect.y + line1_y_off
            } else {
                rect.y + text_y_off(this_item_h)
            };

            // Session number — text bg smoothly blends with hover and active
            let item_bg = lerp_color(
                lerp_color(colors::BG_DARK, colors::BG_HOVER, hover_t),
                colors::BG_ACTIVE,
                active_t,
            );

            // Session number — plain dim text matching web (#555d6b = FG_DIM)
            let num_str = format!("{}", item.number);
            let num_fg = lerp_color(colors::FG_DIM, colors::FG_SECONDARY, hover_t * 0.5);
            ui.text(text, &num_str, num_x, text_y, num_fg, item_bg);

            // Session name (truncated to fit) — web: #e6edf3 active, #9198a1 inactive
            let inactive_name = lerp_color(colors::FG_SECONDARY, colors::FG_BRIGHT, hover_t * 0.6);
            let name_fg = lerp_color(inactive_name, colors::WHITE, active_t);
            let name_max_w = (rect.right() - name_x - pad_h).max(s(30.0));
            let name = truncate_to_width(&item.label, name_max_w, text);
            // Web reference: fontWeight 600 for all session names (both active and inactive)
            ui.text_ui_bold(text, &name, name_x, text_y, name_fg, item_bg);
            if item.active {
                // Web reference: "::" indicator right-aligned on active session
                let indicator_fg = [colors::FG_MUTED[0] * 0.7, colors::FG_MUTED[1] * 0.7, colors::FG_MUTED[2] * 0.7, 0.65];
                let indicator_x = inset_rect.right() - text.text_width_ui("::") - s(4.0);
                ui.text_ui(text, "::", indicator_x, text_y, indicator_fg, item_bg);
            }

            // Second line: branch (web: paddingLeft ~20px, color #484f58)
            // Web always shows branch below the session name.
            if !item.branch.is_empty() {
                let branch_x = name_x; // indented to align with name
                // Web uses #484f58 which is between FG_MUTED and a darker shade
                let branch_fg = lerp_color(
                    [colors::FG_MUTED[0] * 0.7, colors::FG_MUTED[1] * 0.7, colors::FG_MUTED[2] * 0.7, 1.0],
                    colors::FG_MUTED,
                    hover_t * 0.4 + active_t * 0.3,
                );
                ui.text_ui(text, &item.branch, branch_x, rect.y + line2_y_off, branch_fg, item_bg);
            } else if !item.description.is_empty() {
                let desc_avail = sidebar.width - pad_h * 2.0 - ui_cw * 2.0;
                let desc = truncate_to_width(&item.description, desc_avail, text);
                let desc_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, 0.40 + hover_t * 0.3);
                ui.text_ui(text, &desc, name_x, rect.y + line2_y_off, desc_fg, item_bg);
            }

            // Web reference: no separator lines between sessions, just marginBottom spacing
        }

        // Scrollbar — matches web CSS: ::-webkit-scrollbar { width: 6px }
        // Track: transparent, Thumb: #2d333b (BG_HOVER), hover: #3b4048, radius: 3px
        {
            let items_total_h = self.items_y_offset(self.items.len(), text.scale);
            let track_w = s(6.0);
            let track_x = sidebar.right() - track_w - s(2.0);
            let track_y = sidebar.y + header_h + s(4.0);
            let track_h = items_total_h - s(4.0);
            let radius = s(3.0);
            if track_h > s(10.0) {
                // Track: transparent (web: background: transparent) — no fill
                // Thumb — only shown when content overflows (matches web CSS behavior)
                let any_hover = self.hovered_index.is_some();
                let visible_ratio = 1.0_f32; // all items visible for now
                if visible_ratio < 1.0 {
                    let thumb_h = (track_h * visible_ratio).max(s(16.0)).min(track_h);
                    let thumb_y = track_y; // scroll_offset * (track_h - thumb_h) for real scrolling
                    let thumb_rect = Rect { x: track_x, y: thumb_y, width: track_w, height: thumb_h };
                    // Web: thumb #2d333b, hover #3b4048
                    let thumb_color = if any_hover {
                        [0.231, 0.251, 0.282, 1.0] // #3b4048
                    } else {
                        [colors::BG_HOVER[0], colors::BG_HOVER[1], colors::BG_HOVER[2], 1.0] // #2d333b
                    };
                    ui.fill_rounded(thumb_rect, thumb_color, radius);
                }
            }
        }

        // Session list bottom fade — gradient overlay for smooth visual clipping
        // before the New Session button. Professional sidebars (Zed, VS Code) use
        // this to indicate scrollable content.
        {
            let items_total_h = self.items_y_offset(self.items.len(), text.scale);
            let fade_h = s(16.0);
            let fade_y = sidebar.y + header_h + items_total_h - fade_h;
            if fade_y > sidebar.y + header_h {
                let fade_rect = Rect {
                    x: sidebar.x,
                    y: fade_y,
                    width: sidebar.width - s(6.0), // leave room for scrollbar
                    height: fade_h,
                };
                ui.fill_gradient(
                    fade_rect,
                    [colors::BG_DARK[0], colors::BG_DARK[1], colors::BG_DARK[2], 0.0],
                    [colors::BG_DARK[0], colors::BG_DARK[1], colors::BG_DARK[2], 0.5],
                );
            }
        }

        // Section divider between session list and new-session button — thin line
        {
            let items_total_h = self.items_y_offset(self.items.len(), text.scale);
            let div_y = sidebar.y + header_h + items_total_h + s(1.0);
            ui.hline_fade(sidebar.x + pad_h * 1.5, div_y,
                     sidebar.width - pad_h * 3.0, 1.0,
                     [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12], s(16.0));
        }

        // "+ New session" button
        let items_total_h = self.items_y_offset(self.items.len(), text.scale);
        let new_y = sidebar.y + header_h + items_total_h + s(4.0);
        let new_rect = Rect {
            x: sidebar.x + pad_h,
            y: new_y,
            width: sidebar.width - pad_h * 2.0,
            height: compact_h,
        };
        let new_t = self.new_btn_anim.value();
        let btn_r = s(6.0);
        // Filled CTA button: subtle green-tinted fill at rest, brighter on hover.
        // Rest state has a green wash so it reads as an action button.
        let rest_fill = [
            colors::ACCENT_GREEN[0] * 0.15 + colors::BG_DARK[0] * 0.85,
            colors::ACCENT_GREEN[1] * 0.15 + colors::BG_DARK[1] * 0.85,
            colors::ACCENT_GREEN[2] * 0.15 + colors::BG_DARK[2] * 0.85,
            1.0,
        ];
        let rest_border = [
            colors::ACCENT_GREEN[0] * 0.30 + colors::BORDER[0] * 0.70,
            colors::ACCENT_GREEN[1] * 0.30 + colors::BORDER[1] * 0.70,
            colors::ACCENT_GREEN[2] * 0.30 + colors::BORDER[2] * 0.70,
            0.40,
        ];
        let hover_fill = [
            colors::ACCENT_GREEN[0] * 0.22 + colors::BG_SURFACE[0] * 0.78,
            colors::ACCENT_GREEN[1] * 0.22 + colors::BG_SURFACE[1] * 0.78,
            colors::ACCENT_GREEN[2] * 0.22 + colors::BG_SURFACE[2] * 0.78,
            1.0,
        ];
        let hover_border = [
            colors::ACCENT_GREEN[0] * 0.45 + colors::BORDER[0] * 0.55,
            colors::ACCENT_GREEN[1] * 0.45 + colors::BORDER[1] * 0.55,
            colors::ACCENT_GREEN[2] * 0.45 + colors::BORDER[2] * 0.55,
            0.60,
        ];
        let btn_fill = lerp_color(rest_fill, hover_fill, new_t);
        let btn_fill_top = [btn_fill[0] * 1.06, btn_fill[1] * 1.06, btn_fill[2] * 1.06, btn_fill[3]];
        let btn_border = lerp_color(rest_border, hover_border, new_t);
        ui.fill_rounded_gradient(new_rect, btn_fill_top, btn_fill, btn_r);
        ui.stroke_rounded(new_rect, btn_r, 0.5, btn_border);
        // Green glow on hover
        if new_t > 0.005 {
            let glow_rect = Rect {
                x: new_rect.x - s(2.0), y: new_rect.y - s(1.0),
                width: new_rect.width + s(4.0), height: new_rect.height + s(2.0),
            };
            ui.fill_shadow(glow_rect,
                [colors::ACCENT_GREEN[0], colors::ACCENT_GREEN[1], colors::ACCENT_GREEN[2], 0.06 * new_t],
                btn_r, s(8.0));
        }
        // Plus icon + label — icon uses accent green for visual pop
        let icon_t = (1.2 * text.scale).max(1.0);
        let icon_rect = Rect {
            x: new_rect.x, y: new_rect.y,
            width: s(24.0), height: new_rect.height,
        };
        let icon_fg = lerp_color(
            [colors::ACCENT_GREEN[0], colors::ACCENT_GREEN[1], colors::ACCENT_GREEN[2], 0.65],
            colors::ACCENT_GREEN,
            new_t,
        );
        ui.icon_plus(icon_rect, icon_t, s(4.0), icon_fg);
        let new_fg = lerp_color(
            [colors::FG_MUTED[0] * 0.6 + colors::ACCENT_GREEN[0] * 0.4,
             colors::FG_MUTED[1] * 0.6 + colors::ACCENT_GREEN[1] * 0.4,
             colors::FG_MUTED[2] * 0.6 + colors::ACCENT_GREEN[2] * 0.4,
             colors::FG_MUTED[3]],
            colors::FG_PRIMARY,
            new_t,
        );
        let new_bg = btn_fill;
        ui.text_ui(text, "New Session",
                new_rect.x + s(22.0),
                new_rect.y + text_y_off(compact_h),
                new_fg, new_bg);

        // Section divider above processes panel — thin line
        if !self.agents.is_empty() {
            let settings_row_h = s(28.0);
            let header_section_h = s(28.0);
            let agent_item_h = s(36.0);
            let agent_panel_h_est = header_section_h + self.agents.len() as f32 * agent_item_h + s(4.0);
            let panel_y_est = sidebar.bottom() - settings_row_h - agent_panel_h_est;
            let div_y = panel_y_est - s(4.0);
            if div_y > new_y + compact_h + s(4.0) {
                ui.hline_fade(sidebar.x + pad_h * 1.5, div_y,
                         sidebar.width - pad_h * 3.0, 1.0,
                         [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12], s(16.0));
            }
        }

        // Bottom panel: running agents/processes — flat inline layout matching
        // SESSIONS section style (no card container, just header + items).
        if !self.agents.is_empty() {
            let agent_item_h = s(36.0);
            let header_section_h = s(28.0);
            let agent_panel_h = header_section_h + self.agents.len() as f32 * agent_item_h + s(4.0);
            let settings_row_h = s(28.0);
            let panel_y = sidebar.bottom() - settings_row_h - agent_panel_h;

            // "Processes" header — inline mixed-case matching web reference
            // (no disclosure triangle, no pill badge)
            ui.text_ui(text, &format!("Processes {}", self.agents.len()),
                    sidebar.x + pad_h,
                    panel_y + (header_section_h - ch) / 2.0,
                    [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.65],
                    colors::BG_DARK);
            // Thin separator below header
            ui.hline_fade(sidebar.x + pad_h, panel_y + header_section_h - 1.0,
                     sidebar.width - pad_h * 2.0, 1.0,
                     [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.15], s(12.0));

            let mut ay = panel_y + header_section_h;
            for (ai, agent) in self.agents.iter().enumerate() {
                let status_color = match agent.status {
                    AgentStatus::Running => colors::ACCENT_GREEN,
                    AgentStatus::Waiting => colors::ACCENT_PEACH,
                    AgentStatus::Stopped => colors::ACCENT_RED,
                };
                let status_text = match agent.status {
                    AgentStatus::Running => "running",
                    AgentStatus::Waiting => "waiting",
                    AgentStatus::Stopped => "stopped",
                };

                // Single-line agent item: dot + name + status badge
                let agent_hover_t = self.agent_hover_anim.get(ai);
                let agent_inset = Rect {
                    x: sidebar.x + s(6.0),
                    y: ay + s(1.0),
                    width: sidebar.width - s(12.0),
                    height: agent_item_h - s(2.0),
                };
                if agent_hover_t > 0.005 {
                    let ahover_bg = [
                        colors::BG_HOVER[0], colors::BG_HOVER[1], colors::BG_HOVER[2],
                        colors::BG_HOVER[3] * agent_hover_t,
                    ];
                    ui.fill_rounded(agent_inset, ahover_bg, s(3.0));
                }

                let text_y = ay + (agent_item_h - ch) / 2.0;
                let panel_bg = colors::BG_DARK;

                // Status indicator dot — clean, no orbit animation
                let dot_r = s(2.5);
                let dot_size = dot_r * 2.0;
                let dot_rect = Rect {
                    x: sidebar.x + pad_h + (cw - dot_size) / 2.0,
                    y: text_y + (ch - dot_size) / 2.0,
                    width: dot_size,
                    height: dot_size,
                };
                // Running dots get subtle breathing glow only
                if matches!(agent.status, AgentStatus::Running) {
                    let breath = 0.92 + 0.08 * self.glow_phase.sin();
                    let glow_rect = Rect {
                        x: dot_rect.x - s(2.0), y: dot_rect.y - s(2.0),
                        width: dot_size + s(4.0), height: dot_size + s(4.0),
                    };
                    ui.fill_shadow(glow_rect, [status_color[0], status_color[1], status_color[2], 0.15 * breath], dot_r + s(2.0), s(4.0));
                }
                ui.fill_rounded(dot_rect, status_color, dot_r);

                // Agent name (brightens on hover)
                let agent_name_fg = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, agent_hover_t * 0.4);
                ui.text_ui(text, &agent.name,
                        sidebar.x + pad_h + cw * 2.0,
                        text_y,
                        agent_name_fg, panel_bg);

                // Status label (right-aligned, pill-shaped badge)
                let sw = text.text_width_ui(status_text);
                let status_badge_pad_h = s(4.0);
                let status_badge_h = ch * 0.75;
                let status_badge_w = sw + status_badge_pad_h * 2.0;
                let status_badge_x = sidebar.right() - status_badge_w - pad_h;
                let status_badge_y = text_y + (ch - status_badge_h) / 2.0;
                let status_badge_rect = Rect {
                    x: status_badge_x, y: status_badge_y,
                    width: status_badge_w, height: status_badge_h,
                };
                let status_badge_r = status_badge_h / 2.0;
                let status_bg = [status_color[0], status_color[1], status_color[2], 0.12];
                ui.fill_rounded(status_badge_rect, status_bg, status_badge_r);
                ui.stroke_rounded(status_badge_rect, status_badge_r, 0.5,
                    [status_color[0], status_color[1], status_color[2], 0.25]);
                let status_text_x = status_badge_x + status_badge_pad_h;
                let status_text_y = status_badge_y + (status_badge_h - ch) / 2.0;
                ui.text_ui(text, status_text,
                        status_text_x, status_text_y,
                        status_color, panel_bg);

                ay += agent_item_h;

                // Subtle separator between agent items
                if !std::ptr::eq(agent, self.agents.last().unwrap()) {
                    ui.hline_fade(
                        sidebar.x + pad_h + cw * 2.0,
                        ay - 1.0,
                        sidebar.width - pad_h * 2.0 - cw * 2.0,
                        1.0,
                        [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12],
                        s(8.0),
                    );
                }
            }
        }

        // Bottom settings row — gear icon + "Settings" label (anchored to very bottom, animated hover)
        {
            let settings_h = s(28.0);
            let settings_y = sidebar.bottom() - settings_h;
            let settings_t = self.settings_anim.value();
            // Top separator — single thin hairline (modern, matching session dividers)
            ui.hline_fade(sidebar.x + pad_h, settings_y, sidebar.width - pad_h * 2.0, 1.0,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12], s(12.0));

            // Hover background (animated rounded rect)
            let settings_inset = Rect {
                x: sidebar.x + s(6.0),
                y: settings_y + s(2.0),
                width: sidebar.width - s(12.0),
                height: settings_h - s(4.0),
            };
            if settings_t > 0.005 {
                let hover_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], lerp(0.0, 0.4, settings_t)];
                let hover_top = [
                    colors::BG_HOVER[0] * lerp(1.0, 1.08, settings_t),
                    colors::BG_HOVER[1] * lerp(1.0, 1.08, settings_t),
                    colors::BG_HOVER[2] * lerp(1.0, 1.08, settings_t),
                    colors::BG_HOVER[3] * settings_t,
                ];
                let hover_bot = [colors::BG_HOVER[0], colors::BG_HOVER[1], colors::BG_HOVER[2], colors::BG_HOVER[3] * settings_t];
                ui.fill_rounded_gradient(settings_inset, hover_top, hover_bot, s(4.0));
                ui.stroke_rounded(settings_inset, s(4.0), 0.5, hover_border);
            }

            let settings_bg = lerp_color(colors::BG_DARK, colors::BG_HOVER, settings_t);

            // Gear icon (circle ring) — brightens on hover
            let gear_sz = s(14.0);
            let gear_rect = Rect {
                x: sidebar.x + pad_h,
                y: settings_y + (settings_h - gear_sz) / 2.0,
                width: gear_sz,
                height: gear_sz,
            };
            let gear_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, settings_t);
            ui.icon_gear(gear_rect, gear_sz, gear_sz * 0.5, gear_fg, settings_bg);

            // "Settings" label — brightens on hover
            let settings_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, settings_t);
            ui.text_ui(text, "Settings",
                    sidebar.x + pad_h + gear_sz + s(8.0),
                    settings_y + (settings_h - ch) / 2.0,
                    settings_fg, settings_bg);

            // Keyboard shortcut hint (right-aligned, very muted)
            let hint = "Ctrl+,";
            let hint_w = text.text_width_ui(hint);
            let hint_fg = lerp_color(
                [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.4],
                [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.7],
                settings_t,
            );
            ui.text_ui(text, hint,
                    sidebar.right() - hint_w - pad_h,
                    settings_y + (settings_h - ch) / 2.0,
                    hint_fg, settings_bg);

            // Version indicator — very muted, right-aligned below Settings row
            let version_str = concat!("v", env!("CARGO_PKG_VERSION"));
            let version_w = text.text_width_ui(version_str);
            let version_y = settings_y + settings_h + s(2.0);
            if version_y + ch < sidebar.bottom() {
                let version_fg = [
                    colors::FG_MUTED[0], colors::FG_MUTED[1],
                    colors::FG_MUTED[2], 0.25,
                ];
                ui.text_ui(text, version_str,
                    sidebar.right() - version_w - pad_h,
                    version_y,
                    version_fg, colors::BG_DARK);
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, sidebar: Rect, scale: f32) -> Option<UiAction> {
        if sidebar.width < 1.0 { return None; }
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_index = None;
                self.hovered_agent = None;
                self.hovered_new = false;
                self.hovered_settings = false;
                for (i, _) in self.items.iter().enumerate() {
                    if self.item_rect(i, sidebar, scale).contains(x, y) { self.hovered_index = Some(i); }
                }
                // Agent item hover detection
                if !self.agents.is_empty() {
                    let agent_item_h = (44.0 * scale).round();
                    let header_section_h = (28.0 * scale).round();
                    let settings_row_h = (28.0 * scale).round();
                    let agent_panel_h = header_section_h + self.agents.len() as f32 * agent_item_h + (8.0 * scale).round();
                    let panel_y = sidebar.bottom() - settings_row_h - agent_panel_h;
                    let mut ay = panel_y + header_section_h;
                    for i in 0..self.agents.len() {
                        let agent_rect = Rect {
                            x: sidebar.x,
                            y: ay,
                            width: sidebar.width,
                            height: agent_item_h,
                        };
                        if agent_rect.contains(x, y) {
                            self.hovered_agent = Some(i);
                        }
                        ay += agent_item_h;
                    }
                }
                if self.new_button_rect(sidebar, scale).contains(x, y) { self.hovered_new = true; }
                if self.settings_rect(sidebar, scale).contains(x, y) { self.hovered_settings = true; }
                None
            }
            MouseEvent::Press { x, y } => {
                for (i, item) in self.items.iter().enumerate() {
                    if self.item_rect(i, sidebar, scale).contains(x, y) {
                        return Some(UiAction::SwitchTab(item.id.clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }
}
