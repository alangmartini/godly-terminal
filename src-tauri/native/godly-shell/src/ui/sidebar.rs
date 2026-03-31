//! Left sidebar: session list with names, active indicator, and new session button.

use super::anim::{self, Anim, AnimVec, lerp_color, lerp};
use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const HEADER_HEIGHT: f32 = 30.0;
const ITEM_HEIGHT: f32 = 52.0;
const ITEM_HEIGHT_COMPACT: f32 = 38.0;
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
                },
                SidebarItem {
                    id: "session-2".into(),
                    label: "opensessions".into(),
                    number: 2,
                    branch: "main".into(),
                    description: "".into(),
                    active: false,
                },
                SidebarItem {
                    id: "session-3".into(),
                    label: "quiver".into(),
                    number: 3,
                    branch: "main".into(),
                    description: "".into(),
                    active: false,
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
        if self.items[index].description.is_empty() {
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
        let bottom_panel_h = s(BOTTOM_PANEL_HEIGHT);
        let text_y_off = |area_h: f32| (area_h - ch) / 2.0;

        // Sidebar background — subtle vertical gradient (slightly darker at bottom)
        let sidebar_bottom_color = [
            colors::BG_DARK[0] * 0.9,
            colors::BG_DARK[1] * 0.9,
            colors::BG_DARK[2] * 0.9,
            colors::BG_DARK[3],
        ];
        ui.fill_gradient(sidebar, colors::BG_DARK, sidebar_bottom_color);

        // Convexity gradient: very subtle left-brighter overlay that suggests
        // the sidebar surface has slight curvature catching light from the left.
        // At 0.02 alpha this shifts brightness by ~5/255 — barely perceptible
        // but contributes to the "real material" feel.
        ui.fill_gradient_h(
            sidebar,
            [1.0, 1.0, 1.0, 0.02],
            [1.0, 1.0, 1.0, 0.0],
        );

        // Right border separator — embossed groove for professional panel junction.
        // Dark edge + light highlight creates an inset channel effect.
        let groove_dark = [0.0, 0.0, 0.0, 0.15];
        let groove_light = [1.0, 1.0, 1.0, 0.04];
        ui.vgroove_fade(sidebar.right() - 2.0, sidebar.y, sidebar.height, groove_dark, groove_light, s(12.0));
        // SDF inner shadow — smooth Gaussian falloff from all edges for recessed depth.
        // Replaces separate gradient overlays with a single, more natural shadow.
        ui.fill_inner_shadow(sidebar, [0.0, 0.0, 0.0, 0.06], 0.0, s(5.0));
        // Inner bevel highlight (faded at edges for softer integration)
        // Slightly brighter than typical to be perceptible on dark themes.
        ui.hline_fade(sidebar.x + s(4.0), sidebar.y, sidebar.width - s(8.0) - 1.0, 1.0, [1.0, 1.0, 1.0, 0.04], s(12.0));

        // "Sessions" header with count badge
        let header_rect = Rect {
            x: sidebar.x,
            y: sidebar.y,
            width: sidebar.width,
            height: header_h,
        };
        // All-caps section header (Zed-style: small, muted, uppercase)
        ui.text_ui(
            text,
            "SESSIONS",
            header_rect.x + pad_h,
            header_rect.y + text_y_off(header_h),
            [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.65],
            colors::BG_DARK,
        );
        // Session count badge (right-aligned, pill-shaped)
        let count_str = format!("{}", self.items.len());
        let count_w = text.text_width(&count_str);
        let badge_pad_h = s(4.0);
        let badge_h = ch * 0.85;
        let badge_w = (count_w + badge_pad_h * 2.0).max(badge_h);
        let badge_x = header_rect.right() - badge_w - pad_h;
        let badge_y = header_rect.y + (header_h - badge_h) / 2.0;
        let badge_rect = Rect { x: badge_x, y: badge_y, width: badge_w, height: badge_h };
        let badge_radius = badge_h / 2.0;
        ui.fill_rounded(badge_rect, [
            colors::BG_SURFACE[0], colors::BG_SURFACE[1],
            colors::BG_SURFACE[2], 0.5,
        ], badge_radius);
        ui.stroke_rounded(badge_rect, badge_radius, 0.5,
            [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.2]);
        let count_text_x = badge_x + (badge_w - count_w) / 2.0;
        let count_text_y = badge_y + (badge_h - ch) / 2.0;
        ui.text(
            text,
            &count_str,
            count_text_x,
            count_text_y,
            colors::FG_MUTED,
            colors::BG_SURFACE,
        );
        // Header bottom separator — groove for embossed look
        ui.hgroove_fade(sidebar.x + pad_h, header_rect.bottom() - 1.0,
                 sidebar.width - pad_h * 2.0, groove_dark, groove_light, s(8.0));

        // Layout: [pad][dot][gap][num][gap][name...][gap][branch][pad]
        // Two-line items: line 1 = dot + number + name + branch, line 2 = description
        let num_x = sidebar.x + pad_h;
        let dot_space = s(5.0) + s(4.0); // dot width + gap
        let name_x = num_x + dot_space + cw * 2.0;
        let branch_max_chars: usize = 6;
        let branch_reserve = cw * (branch_max_chars as f32) + pad_h + cw;
        let name_max_w = sidebar.width - (name_x - sidebar.x) - branch_reserve;
        let name_max_chars = (name_max_w / cw).floor().max(1.0) as usize;
        let line1_y_off = s(8.0); // top padding for first line
        let line2_y_off = line1_y_off + ch + s(2.0); // second line below first

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
            let item_radius = s(4.0);
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
                } else if active_t < 0.005 {
                    let rest_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12];
                    ui.stroke_rounded(inset_rect, item_radius, 0.5, rest_border);
                }
            }

            // Active state (fades in with active_t)
            // Uses the session's own accent color from the rotating palette
            // for visual continuity with the tab bar's colored badges.
            if active_t > 0.005 {
                let ac = session_accent;
                let breath = 0.85 + 0.15 * self.glow_phase.sin();
                let glow_rect = Rect {
                    x: inset_rect.x - s(3.0),
                    y: inset_rect.y - s(3.0),
                    width: inset_rect.width + s(6.0),
                    height: inset_rect.height + s(6.0),
                };
                ui.fill_shadow(glow_rect, [ac[0], ac[1], ac[2], 0.08 * breath * active_t], item_radius + s(3.0), s(10.0));
                ui.fill_shadow(inset_rect, [0.0, 0.0, 0.0, 0.12 * active_t], item_radius, s(5.0));
                let active_border = [
                    ac[0] * 0.35,
                    ac[1] * 0.35,
                    ac[2] * 0.35,
                    0.6 * active_t,
                ];
                let active_bg = lerp_color(colors::BG_DARK, colors::BG_ACTIVE, active_t);
                ui.fill_rounded_bordered(
                    inset_rect, active_bg, item_radius,
                    0.5, active_border,
                );
                let ambient = Rect {
                    x: inset_rect.x + 1.0, y: inset_rect.y + 1.0,
                    width: inset_rect.width - 2.0, height: inset_rect.height - 1.0,
                };
                let inner_r = (item_radius - 1.0).max(0.0);
                ui.fill_rounded_gradient(ambient,
                    [ac[0], ac[1], ac[2], 0.06 * active_t],
                    [ac[0], ac[1], ac[2], 0.0],
                    inner_r,
                );
            }

            // Active indicator (left colored bar, pill shape via SDF + breathing glow)
            // Uses session's own accent color for sidebar-tab color continuity.
            if active_t > 0.005 {
                let ac = session_accent;
                let indicator_rect = Rect {
                    x: rect.x + s(3.0),
                    y: rect.y + s(7.0),
                    width: indicator_w,
                    height: rect.height - s(14.0),
                };
                let breath = 0.85 + 0.15 * self.glow_phase.sin();
                let glow_alpha = 0.25 * breath * active_t;
                ui.fill_shadow(indicator_rect, [ac[0], ac[1], ac[2], glow_alpha], indicator_w, s(7.0));
                ui.fill_rounded(indicator_rect, [ac[0], ac[1], ac[2], active_t], indicator_w / 2.0);

                let trail_rect = Rect {
                    x: indicator_rect.right(),
                    y: indicator_rect.y + indicator_rect.height * 0.15,
                    width: s(18.0),
                    height: indicator_rect.height * 0.7,
                };
                ui.fill_shadow(trail_rect,
                    [ac[0], ac[1], ac[2], 0.07 * breath * active_t],
                    0.0, s(12.0));
            }

            // Text y position: centered for compact, top-aligned for two-line
            let text_y = if item.description.is_empty() {
                rect.y + text_y_off(this_item_h)
            } else {
                rect.y + line1_y_off
            };

            // Session number — text bg smoothly blends with hover and active
            let item_bg = lerp_color(
                lerp_color(colors::BG_DARK, colors::BG_HOVER, hover_t),
                colors::BG_ACTIVE,
                active_t,
            );

            // Session accent dot — small colored circle matching the tab accent
            // color cycle. Creates visual continuity between tab bar badges and
            // the sidebar session list.
            let dot_sz = s(5.0);
            let dot_x = num_x;
            let dot_y = text_y + (ch - dot_sz) / 2.0;
            let dot_rect = Rect {
                x: dot_x, y: dot_y, width: dot_sz, height: dot_sz,
            };
            let dot_alpha = lerp(0.5, 1.0, active_t.max(hover_t * 0.6));
            let dot_color = [session_accent[0], session_accent[1], session_accent[2], dot_alpha];
            ui.fill_rounded(dot_rect, dot_color, dot_sz / 2.0);
            // Subtle glow on active session dot
            if active_t > 0.005 {
                let breath = 0.85 + 0.15 * self.glow_phase.sin();
                let glow_rect = Rect {
                    x: dot_x - s(2.0), y: dot_y - s(2.0),
                    width: dot_sz + s(4.0), height: dot_sz + s(4.0),
                };
                ui.fill_shadow(glow_rect,
                    [session_accent[0], session_accent[1], session_accent[2], 0.15 * breath * active_t],
                    dot_sz / 2.0 + s(2.0), s(4.0));
            }

            // Session number (shifted right to make room for accent dot)
            let num_x_shifted = num_x + dot_sz + s(4.0);
            let num_str = format!("{}", item.number);
            let inactive_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, hover_t);
            let fg = lerp_color(inactive_fg, session_accent, active_t);
            ui.text(text, &num_str, num_x_shifted, text_y, fg, item_bg);

            // Session name (truncated to fit) — text brightens on hover and active
            // Active session name gets full brightness for clear visual hierarchy
            let inactive_name = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, hover_t * 0.4);
            let name_fg = lerp_color(inactive_name, colors::WHITE, active_t * 0.85);
            let name = if item.label.len() > name_max_chars {
                format!("{}\u{2026}", &item.label[..name_max_chars.saturating_sub(1)])
            } else {
                item.label.clone()
            };
            if item.active {
                ui.text_ui_bold(text, &name, name_x, text_y, name_fg, item_bg);
            } else {
                ui.text_ui(text, &name, name_x, text_y, name_fg, item_bg);
            }

            // Branch info (right-aligned, truncated)
            if !item.branch.is_empty() && sidebar.width > s(150.0) {
                let branch = if item.branch.len() > branch_max_chars {
                    format!("{}\u{2026}", &item.branch[..branch_max_chars - 1])
                } else {
                    item.branch.clone()
                };
                let branch_w = text.text_width_ui(&branch);
                // Boost readability: start at 40% toward FG_SECONDARY (up from 25%)
                let branch_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, 0.40 + hover_t * 0.3);
                ui.text_ui(text, &branch,
                        rect.right() - branch_w - pad_h,
                        text_y,
                        branch_fg, item_bg);
            }

            // Description line (second row, brightens on hover for readability)
            if !item.description.is_empty() {
                let desc_max_chars = ((sidebar.width - pad_h * 2.0 - cw * 2.0) / cw).floor().max(1.0) as usize;
                let desc = if item.description.len() > desc_max_chars {
                    format!("{}\u{2026}", &item.description[..desc_max_chars.saturating_sub(1)])
                } else {
                    item.description.clone()
                };
                // Start from a blend between FG_MUTED and FG_SECONDARY for better baseline readability
                let base_desc = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, 0.40);
                let inactive_desc = lerp_color(base_desc, colors::FG_SECONDARY, hover_t * 0.4);
                let desc_fg = lerp_color(inactive_desc, colors::FG_SECONDARY, active_t * 0.5);
                ui.text_ui(text, &desc,
                        name_x,
                        rect.y + line2_y_off,
                        desc_fg, item_bg);
            }

            // Subtle separator between items (faded, skip for last item)
            if i + 1 < self.items.len() {
                let next_active = self.items[i + 1].active;
                if !item.active && !next_active {
                    let sep_fade = 1.0 - hover_t.max(self.item_hover_anim.get(i + 1));
                    let sep_color = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.2 * sep_fade];
                    ui.hline_fade(
                        sidebar.x + pad_h + cw * 2.0,
                        rect.bottom() - 1.0,
                        sidebar.width - pad_h * 2.0 - cw * 2.0,
                        1.0,
                        sep_color,
                        s(8.0),
                    );
                }
            }
        }

        // Section divider between session list and new-session button
        {
            let items_total_h = self.items_y_offset(self.items.len(), text.scale);
            let div_y = sidebar.y + header_h + items_total_h + s(1.0);
            ui.hgroove_fade(sidebar.x + pad_h, div_y,
                     sidebar.width - pad_h * 2.0, groove_dark, groove_light, s(12.0));
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
        // Rest state: dashed-looking border with green accent tint to hint at CTA
        let rest_border = [
            colors::ACCENT_GREEN[0] * 0.3 + colors::BORDER[0] * 0.7,
            colors::ACCENT_GREEN[1] * 0.3 + colors::BORDER[1] * 0.7,
            colors::ACCENT_GREEN[2] * 0.3 + colors::BORDER[2] * 0.7,
            0.25,
        ];
        if new_t > 0.005 {
            let bg = lerp_color(colors::BG_DARK, colors::BG_SURFACE, new_t);
            let new_top = [bg[0] * lerp(1.0, 1.08, new_t), bg[1] * lerp(1.0, 1.08, new_t), bg[2] * lerp(1.0, 1.08, new_t), bg[3]];
            let border_alpha = lerp(0.25, 0.6, new_t);
            let border = [
                lerp(rest_border[0], colors::ACCENT_GREEN[0] * 0.5, new_t),
                lerp(rest_border[1], colors::ACCENT_GREEN[1] * 0.5, new_t),
                lerp(rest_border[2], colors::ACCENT_GREEN[2] * 0.5, new_t),
                border_alpha,
            ];
            ui.fill_rounded_gradient(new_rect, new_top, bg, s(5.0));
            ui.stroke_rounded(new_rect, s(5.0), 0.5, border);
            // Subtle green glow on hover
            let glow_rect = Rect {
                x: new_rect.x - s(2.0), y: new_rect.y - s(1.0),
                width: new_rect.width + s(4.0), height: new_rect.height + s(2.0),
            };
            ui.fill_shadow(glow_rect,
                [colors::ACCENT_GREEN[0], colors::ACCENT_GREEN[1], colors::ACCENT_GREEN[2], 0.05 * new_t],
                s(5.0), s(8.0));
        } else {
            ui.stroke_rounded(new_rect, s(5.0), 0.5, rest_border);
        }
        // Plus icon + label — icon uses accent green for visual pop
        let icon_t = (1.2 * text.scale).max(1.0);
        let icon_rect = Rect {
            x: new_rect.x, y: new_rect.y,
            width: s(24.0), height: new_rect.height,
        };
        let icon_fg = lerp_color(
            [colors::ACCENT_GREEN[0], colors::ACCENT_GREEN[1], colors::ACCENT_GREEN[2], 0.55],
            colors::ACCENT_GREEN,
            new_t,
        );
        ui.icon_plus(icon_rect, icon_t, s(4.0), icon_fg);
        let new_fg = lerp_color(
            [colors::FG_MUTED[0] * 0.7 + colors::ACCENT_GREEN[0] * 0.3,
             colors::FG_MUTED[1] * 0.7 + colors::ACCENT_GREEN[1] * 0.3,
             colors::FG_MUTED[2] * 0.7 + colors::ACCENT_GREEN[2] * 0.3,
             colors::FG_MUTED[3]],
            colors::FG_SECONDARY,
            new_t,
        );
        let new_bg = lerp_color(colors::BG_DARK, colors::BG_SURFACE, new_t);
        ui.text_ui(text, "New Session",
                new_rect.x + s(22.0),
                new_rect.y + text_y_off(compact_h),
                new_fg, new_bg);

        // Section divider above processes panel
        if !self.agents.is_empty() {
            let settings_row_h = s(28.0);
            let header_section_h = s(28.0);
            let agent_item_h = s(44.0);
            let agent_panel_h_est = header_section_h + self.agents.len() as f32 * agent_item_h + s(8.0);
            let panel_y_est = sidebar.bottom() - settings_row_h - agent_panel_h_est;
            let div_y = panel_y_est - s(4.0);
            if div_y > new_y + compact_h + s(4.0) {
                ui.hgroove_fade(sidebar.x + pad_h, div_y,
                         sidebar.width - pad_h * 2.0, groove_dark, groove_light, s(12.0));
            }
        }

        // Bottom panel: running agents/processes
        if !self.agents.is_empty() {
            let agent_item_h = s(44.0);
            let header_section_h = s(28.0);
            let agent_panel_h = header_section_h + self.agents.len() as f32 * agent_item_h + s(8.0);
            // Anchor agent panel above the bottom settings row
            let settings_row_h = s(28.0);
            let panel_y = sidebar.bottom() - settings_row_h - agent_panel_h;
            let panel = Rect {
                x: sidebar.x,
                y: panel_y,
                width: sidebar.width,
                height: agent_panel_h.min(bottom_panel_h),
            };

            // Panel container — rounded rect with subtle border for depth
            let panel_inset = Rect {
                x: panel.x + s(6.0),
                y: panel.y + s(2.0),
                width: panel.width - s(12.0),
                height: panel.height - s(4.0),
            };
            let panel_radius = s(6.0);
            let panel_border = [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.5];
            // Directional shadow — offset downward for natural top-left light
            // source depth.  More convincing than centered shadows.
            ui.fill_shadow_offset(
                panel_inset, [0.0, 0.0, 0.0, 0.12],
                panel_radius, s(8.0),
                0.0, s(2.0),
            );
            // Gradient panel background (slightly lighter top for 3D)
            let panel_top = [
                colors::BG_RAISED[0] * 1.06,
                colors::BG_RAISED[1] * 1.06,
                colors::BG_RAISED[2] * 1.06,
                colors::BG_RAISED[3],
            ];
            ui.fill_rounded_gradient(panel_inset, panel_top, colors::BG_RAISED, panel_radius);
            ui.stroke_rounded(panel_inset, panel_radius, 0.5, panel_border);
            // Subtle inner shadow for recessed card depth
            ui.fill_inner_shadow(panel_inset, [0.0, 0.0, 0.0, 0.06], panel_radius, s(4.0));

            // "PROCESSES" header (uppercase muted, matching SESSIONS section style)
            ui.text_ui(text, "PROCESSES",
                    sidebar.x + pad_h,
                    panel_y + (header_section_h - ch) / 2.0,
                    [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.65],
                    colors::BG_RAISED);
            // Agent count badge (right-aligned, pill-shaped — matches session count badge)
            let agent_count_str = format!("{}", self.agents.len());
            let agent_count_w = text.text_width(&agent_count_str);
            let agent_badge_pad_h = s(4.0);
            let agent_badge_h = ch * 0.85;
            let agent_badge_w = (agent_count_w + agent_badge_pad_h * 2.0).max(agent_badge_h);
            let agent_badge_x = panel_inset.right() - agent_badge_w - s(8.0);
            let agent_badge_y = panel_y + (header_section_h - agent_badge_h) / 2.0;
            let agent_badge_rect = Rect {
                x: agent_badge_x, y: agent_badge_y,
                width: agent_badge_w, height: agent_badge_h,
            };
            let agent_badge_radius = agent_badge_h / 2.0;
            ui.fill_rounded(agent_badge_rect, [
                colors::BG_SURFACE[0], colors::BG_SURFACE[1],
                colors::BG_SURFACE[2], 0.4,
            ], agent_badge_radius);
            ui.stroke_rounded(agent_badge_rect, agent_badge_radius, 0.5,
                [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.15]);
            let agent_count_text_x = agent_badge_x + (agent_badge_w - agent_count_w) / 2.0;
            let agent_count_text_y = agent_badge_y + (agent_badge_h - ch) / 2.0;
            ui.text(text, &agent_count_str,
                    agent_count_text_x, agent_count_text_y,
                    colors::FG_MUTED, colors::BG_SURFACE);

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

                // Agent item hover background (animated)
                let agent_hover_t = self.agent_hover_anim.get(ai);
                let agent_inset = Rect {
                    x: panel_inset.x + s(4.0),
                    y: ay + s(1.0),
                    width: panel_inset.width - s(8.0),
                    height: agent_item_h - s(2.0),
                };
                if agent_hover_t > 0.005 {
                    let ahover_bg = [
                        colors::BG_HOVER[0], colors::BG_HOVER[1], colors::BG_HOVER[2],
                        colors::BG_HOVER[3] * 0.5 * agent_hover_t,
                    ];
                    ui.fill_rounded(agent_inset, ahover_bg, s(3.0));
                }

                // Line 1: icon + agent name + status (right-aligned)
                let line1_y = ay + s(6.0);

                let panel_bg = colors::BG_RAISED;

                // Status indicator: small SDF colored dot with glow for active states
                let dot_r = s(2.5);
                let dot_size = dot_r * 2.0;
                let dot_rect = Rect {
                    x: sidebar.x + pad_h + (cw - dot_size) / 2.0,
                    y: line1_y + (ch - dot_size) / 2.0,
                    width: dot_size,
                    height: dot_size,
                };
                // Running agents get a breathing Gaussian glow + spinning orbit arc
                if matches!(agent.status, AgentStatus::Running) {
                    let breath = 0.80 + 0.20 * self.glow_phase.sin();
                    let glow_rect = Rect {
                        x: dot_rect.x - s(3.0), y: dot_rect.y - s(3.0),
                        width: dot_size + s(6.0), height: dot_size + s(6.0),
                    };
                    ui.fill_shadow(glow_rect, [status_color[0], status_color[1], status_color[2], 0.25 * breath], dot_r + s(3.0), s(6.0));

                    // Spinning orbit: two small accent-colored dots orbiting the
                    // center, 180° apart.  Uses glow_phase (which advances at
                    // ~1.8 rad/s) multiplied by 2 for a visible spin speed.
                    // Each orbiter is a tiny SDF circle with its own soft glow.
                    let orbit_r = dot_r + s(3.0); // radius of orbit path
                    let spin = self.glow_phase * 2.0;
                    let orbiter_sz = s(1.5);
                    let (dcx, dcy) = dot_rect.center();
                    for k in 0..2u32 {
                        let angle = spin + k as f32 * std::f32::consts::PI;
                        let ox = dcx + orbit_r * angle.cos() - orbiter_sz / 2.0;
                        let oy = dcy + orbit_r * angle.sin() - orbiter_sz / 2.0;
                        let orbit_color = [status_color[0], status_color[1], status_color[2], 0.6 * breath];
                        ui.fill_rounded(
                            Rect { x: ox, y: oy, width: orbiter_sz, height: orbiter_sz },
                            orbit_color, orbiter_sz / 2.0,
                        );
                    }
                    // Orbit trail: faint ring around the dot path for visual continuity
                    let ring_rect = Rect {
                        x: dcx - orbit_r - s(0.5), y: dcy - orbit_r - s(0.5),
                        width: orbit_r * 2.0 + s(1.0), height: orbit_r * 2.0 + s(1.0),
                    };
                    let ring_alpha = 0.10 * breath;
                    ui.stroke_rounded(ring_rect, orbit_r + s(0.5), 0.5,
                        [status_color[0], status_color[1], status_color[2], ring_alpha]);
                }
                ui.fill_rounded(dot_rect, status_color, dot_r);

                // Agent name (brightens on hover)
                let agent_name_fg = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, agent_hover_t * 0.4);
                ui.text_ui(text, &agent.name,
                        sidebar.x + pad_h + cw * 2.0,
                        line1_y,
                        agent_name_fg, panel_bg);

                // Status label (right-aligned, pill-shaped badge)
                let sw = text.text_width_ui(status_text);
                let status_badge_pad_h = s(4.0);
                let status_badge_h = ch * 0.85;
                let status_badge_w = sw + status_badge_pad_h * 2.0;
                let status_badge_x = sidebar.right() - status_badge_w - pad_h - s(6.0);
                let status_badge_y = line1_y + (ch - status_badge_h) / 2.0;
                let status_badge_rect = Rect {
                    x: status_badge_x, y: status_badge_y,
                    width: status_badge_w, height: status_badge_h,
                };
                let status_badge_r = status_badge_h / 2.0;
                // Subtle tinted background for the status badge
                let status_bg = [status_color[0], status_color[1], status_color[2], 0.12];
                ui.fill_rounded(status_badge_rect, status_bg, status_badge_r);
                ui.stroke_rounded(status_badge_rect, status_badge_r, 0.5,
                    [status_color[0], status_color[1], status_color[2], 0.25]);
                let status_text_x = status_badge_x + status_badge_pad_h;
                let status_text_y = status_badge_y + (status_badge_h - ch) / 2.0;
                ui.text_ui(text, status_text,
                        status_text_x, status_text_y,
                        status_color, panel_bg);

                // Line 2: task description (brightens on hover for readability)
                if !agent.task.is_empty() && agent.task != status_text {
                    let line2_y = line1_y + ch + s(2.0);
                    let task_max_chars = ((sidebar.width - pad_h * 2.0 - cw * 2.0) / cw).floor().max(1.0) as usize;
                    let task = if agent.task.len() > task_max_chars {
                        format!("{}\u{2026}", &agent.task[..task_max_chars.saturating_sub(1)])
                    } else {
                        agent.task.clone()
                    };
                    let task_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, agent_hover_t * 0.3);
                    ui.text_ui(text, &task,
                            sidebar.x + pad_h + cw * 2.0,
                            line2_y,
                            task_fg, panel_bg);
                }

                ay += agent_item_h;

                // Subtle separator between agent items (faded edges)
                if !std::ptr::eq(agent, self.agents.last().unwrap()) {
                    ui.hline_fade(
                        sidebar.x + pad_h + cw * 2.0,
                        ay - 1.0,
                        sidebar.width - pad_h * 2.0 - cw * 2.0,
                        1.0,
                        [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.3],
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
            // Top separator — groove for consistency
            ui.hgroove_fade(sidebar.x + pad_h, settings_y, sidebar.width - pad_h * 2.0,
                         [0.0, 0.0, 0.0, 0.12], [1.0, 1.0, 1.0, 0.03], s(8.0));

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
