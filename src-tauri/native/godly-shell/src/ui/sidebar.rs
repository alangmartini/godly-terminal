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
        // Two-line only if there's an explicit description (CWD is shown
        // in the breadcrumb bar and status bar, so it's not repeated here).
        if item.description.is_empty() {
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

        // Sidebar background — nearly flat with very subtle darkening at bottom.
        // Zed/VS Code use flat sidebar backgrounds; a 4% gradient is just enough
        // to hint at depth without visible banding.
        let sidebar_bottom_color = [
            colors::BG_DARK[0] * 0.96,
            colors::BG_DARK[1] * 0.96,
            colors::BG_DARK[2] * 0.96,
            colors::BG_DARK[3],
        ];
        ui.fill_gradient(sidebar, colors::BG_DARK, sidebar_bottom_color);

        // (Convexity gradient removed — flat surface matches Zed/VS Code restraint)

        // Right border separator — near-invisible hairline; the color difference
        // between BG_DARK sidebar and BG_BASE content does the heavy lifting.
        ui.vline(sidebar.right() - 1.0, sidebar.y, sidebar.height, 1.0,
            [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12]);
        // Soft inward shadow (gradient from right edge inward)
        let shadow_w = s(3.0);
        ui.fill_gradient_h(
            Rect { x: sidebar.right() - shadow_w - 1.0, y: sidebar.y, width: shadow_w, height: sidebar.height },
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.03],
        );
        // SDF inner shadow — minimal Gaussian falloff for gentle recessed depth.
        ui.fill_inner_shadow(sidebar, [0.0, 0.0, 0.0, 0.02], 0.0, s(3.0));

        // "Sessions" header with count badge
        let header_rect = Rect {
            x: sidebar.x,
            y: sidebar.y,
            width: sidebar.width,
            height: header_h,
        };
        // Disclosure triangle + all-caps section header (Zed/VS Code pattern)
        let disclosure_sz = ch * 0.55;
        let disclosure_t = (0.8 * text.scale).max(0.5);
        let disclosure_rect = Rect {
            x: header_rect.x + pad_h,
            y: header_rect.y + (header_h - disclosure_sz) / 2.0,
            width: disclosure_sz,
            height: disclosure_sz,
        };
        ui.icon_disclosure_down(disclosure_rect, disclosure_sz, disclosure_t,
            [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.45]);
        let header_text_x = header_rect.x + pad_h + disclosure_sz + s(4.0);
        ui.text_ui(
            text,
            "SESSIONS",
            header_text_x,
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
        // Header bottom separator — single thin line (modern, clean)
        ui.hline_fade(sidebar.x + pad_h, header_rect.bottom() - 1.0,
                 sidebar.width - pad_h * 2.0, 1.0,
                 [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.15], s(12.0));

        // Layout: [pad][dot][gap][num][gap][name...][gap][branch][pad]
        // Two-line items: line 1 = dot + number + name + branch, line 2 = description
        let num_x = sidebar.x + pad_h;
        let dot_space = s(7.0) + s(4.0); // accent dot (7px) + gap (4px)
        let name_x = num_x + dot_space + cw * 2.0;
        let branch_max_chars: usize = 6;
        // Use proportional UI font advance for text width estimation (sidebar labels
        // render with text_ui, not monospace). Proportional chars are ~75% of cell_width.
        let ui_cw = if text.ui_avg_advance > 0.0 { text.ui_avg_advance } else { cw * 0.75 };
        let branch_reserve = ui_cw * (branch_max_chars as f32) + pad_h + ui_cw;
        let name_max_w = sidebar.width - (name_x - sidebar.x) - branch_reserve;
        let name_max_chars = (name_max_w / ui_cw).floor().max(1.0) as usize;
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
                }
            }

            // Active state (fades in with active_t)
            // Uses the session's own accent color from the rotating palette
            // for visual continuity with the tab bar's colored badges.
            if active_t > 0.005 {
                let ac = session_accent;
                let breath = 0.92 + 0.08 * self.glow_phase.sin();
                let glow_rect = Rect {
                    x: inset_rect.x - s(3.0),
                    y: inset_rect.y - s(3.0),
                    width: inset_rect.width + s(6.0),
                    height: inset_rect.height + s(6.0),
                };
                ui.fill_shadow(glow_rect, [ac[0], ac[1], ac[2], 0.10 * breath * active_t], item_radius + s(3.0), s(8.0));
                ui.fill_shadow(inset_rect, [0.0, 0.0, 0.0, 0.08 * active_t], item_radius, s(4.0));
                let active_border = [
                    ac[0] * 0.40,
                    ac[1] * 0.40,
                    ac[2] * 0.40,
                    0.65 * active_t,
                ];
                // Warmer active background: blend more toward accent-tinted surface
                let accent_bg = [
                    colors::BG_ACTIVE[0] * 0.88 + ac[0] * 0.12,
                    colors::BG_ACTIVE[1] * 0.88 + ac[1] * 0.12,
                    colors::BG_ACTIVE[2] * 0.88 + ac[2] * 0.12,
                    colors::BG_ACTIVE[3],
                ];
                let active_bg = lerp_color(colors::BG_DARK, accent_bg, active_t);
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
                    [ac[0], ac[1], ac[2], 0.08 * active_t],
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
                let breath = 0.92 + 0.08 * self.glow_phase.sin();
                let glow_alpha = 0.14 * breath * active_t;
                ui.fill_shadow(indicator_rect, [ac[0], ac[1], ac[2], glow_alpha], indicator_w, s(5.0));
                ui.fill_rounded(indicator_rect, [ac[0], ac[1], ac[2], active_t], indicator_w / 2.0);

                let trail_rect = Rect {
                    x: indicator_rect.right(),
                    y: indicator_rect.y + indicator_rect.height * 0.15,
                    width: s(18.0),
                    height: indicator_rect.height * 0.7,
                };
                ui.fill_shadow(trail_rect,
                    [ac[0], ac[1], ac[2], 0.04 * breath * active_t],
                    0.0, s(10.0));
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

            // Session accent dot — small colored circle matching the tab
            // accent color cycle.  Clean and minimal at sidebar scale.
            let dot_sz = s(7.0);
            let dot_x = num_x; // left-aligned in dot column
            let dot_y = text_y + (ch - dot_sz) / 2.0;
            let dot_rect = Rect {
                x: dot_x, y: dot_y, width: dot_sz, height: dot_sz,
            };
            let dot_alpha = lerp(0.62, 0.92, active_t.max(hover_t * 0.5));
            let dot_color = [session_accent[0], session_accent[1], session_accent[2], dot_alpha];
            ui.fill_rounded(dot_rect, dot_color, dot_sz / 2.0);
            // Subtle glow ring on active session dot
            if active_t > 0.005 {
                let breath = 0.92 + 0.08 * self.glow_phase.sin();
                ui.stroke_rounded(dot_rect, dot_sz / 2.0, 0.5,
                    [session_accent[0], session_accent[1], session_accent[2], 0.20 * breath * active_t]);
            }

            // Session number (shifted right to make room for accent dot)
            let num_x_shifted = num_x + dot_space;
            let num_str = format!("{}", item.number);
            let inactive_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, hover_t);
            let fg = lerp_color(inactive_fg, session_accent, active_t);
            ui.text(text, &num_str, num_x_shifted, text_y, fg, item_bg);

            // Session name (truncated to fit) — text brightens on hover and active
            // Active session name gets full brightness for clear visual hierarchy
            let inactive_name = lerp_color(colors::FG_SECONDARY, colors::FG_PRIMARY, hover_t * 0.6);
            let name_fg = lerp_color(inactive_name, colors::WHITE, active_t);
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

            // Shell type pill badge (right-aligned, next to branch)
            // Small pill showing "pwsh", "bash", etc. for at-a-glance shell identification
            let mut right_edge = rect.right() - pad_h;
            if !item.shell_type.is_empty() && sidebar.width > s(130.0) {
                let shell_w = text.text_width_ui(&item.shell_type);
                let pill_pad_h = s(4.0);
                let pill_h = ch * 0.75;
                let pill_w = shell_w + pill_pad_h * 2.0;
                let pill_x = right_edge - pill_w;
                let pill_y = text_y + (ch - pill_h) / 2.0;
                let pill_rect = Rect { x: pill_x, y: pill_y, width: pill_w, height: pill_h };
                let pill_r = pill_h / 2.0;
                // Muted pill background — very subtle, doesn't compete with session name
                let pill_bg_alpha = lerp(0.25, 0.40, hover_t.max(active_t * 0.5));
                ui.fill_rounded(pill_rect, [
                    colors::BG_SURFACE[0], colors::BG_SURFACE[1],
                    colors::BG_SURFACE[2], pill_bg_alpha,
                ], pill_r);
                ui.stroke_rounded(pill_rect, pill_r, 0.5,
                    [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], 0.12]);
                let pill_fg = lerp_color(
                    [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.7],
                    colors::FG_MUTED,
                    hover_t * 0.3,
                );
                let pill_text_x = pill_x + pill_pad_h;
                let pill_text_y = pill_y + (pill_h - ch) / 2.0;
                ui.text_ui(text, &item.shell_type, pill_text_x, pill_text_y, pill_fg, item_bg);
                right_edge = pill_x - s(4.0);
            }

            // Branch info (right-aligned, before shell pill)
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
                        right_edge - branch_w,
                        text_y,
                        branch_fg, item_bg);
            }

            // Description line (second row, only if explicit description exists)
            // CWD is NOT shown here — it's already in the breadcrumb bar and
            // status bar. Keeping the sidebar compact and focused on session identity.
            let (second_line, is_cwd) = if !item.description.is_empty() {
                (item.description.clone(), false)
            } else {
                (String::new(), false)
            };
            if !second_line.is_empty() {
                // Reserve space for timestamp if present
                let ts_reserve = if !item.timestamp.is_empty() {
                    text.text_width_ui(&item.timestamp) + s(8.0)
                } else {
                    0.0
                };
                let desc_avail = sidebar.width - pad_h * 2.0 - ui_cw * 2.0 - ts_reserve;
                let desc_max_chars = (desc_avail / ui_cw).floor().max(1.0) as usize;
                let desc = if second_line.len() > desc_max_chars {
                    format!("{}\u{2026}", &second_line[..desc_max_chars.saturating_sub(1)])
                } else {
                    second_line
                };
                // Start from a blend between FG_MUTED and FG_SECONDARY for better baseline readability
                let base_desc = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, 0.40);
                let inactive_desc = lerp_color(base_desc, colors::FG_SECONDARY, hover_t * 0.4);
                let desc_fg = lerp_color(inactive_desc, colors::FG_SECONDARY, active_t * 0.5);
                // Render a small folder icon for CWD paths to distinguish from descriptions
                let desc_x = if is_cwd {
                    let icon_fg = [desc_fg[0], desc_fg[1], desc_fg[2], desc_fg[3] * 0.7];
                    let icon_sz = ch * 0.75;
                    let icon_t = (0.8 * text.scale).max(0.5);
                    let icon_rect = Rect {
                        x: name_x,
                        y: rect.y + line2_y_off + (ch - icon_sz) / 2.0,
                        width: icon_sz,
                        height: icon_sz,
                    };
                    ui.icon_folder(icon_rect, icon_t, icon_fg);
                    name_x + icon_sz + s(3.0)
                } else {
                    name_x
                };
                ui.text_ui(text, &desc,
                        desc_x,
                        rect.y + line2_y_off,
                        desc_fg, item_bg);
            }

            // Timestamp label — right-aligned, very muted.
            // On two-line items (with description): second line.
            // On compact items (no description): first line, after branch.
            if !item.timestamp.is_empty() {
                let ts_w = text.text_width_ui(&item.timestamp);
                let ts_x = rect.right() - pad_h - ts_w;
                let ts_y = if item.description.is_empty() {
                    text_y  // first line for compact items
                } else {
                    rect.y + line2_y_off  // second line for two-line items
                };
                let ts_alpha = lerp(0.42, 0.60, hover_t.max(active_t * 0.3));
                let ts_fg = [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], ts_alpha];
                ui.text_ui(text, &item.timestamp, ts_x, ts_y, ts_fg, item_bg);
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

        // Thin scrollbar track — decorative track on the right edge of the
        // session list area.  Shows a small "thumb" proportional to the visible
        // items / total items ratio.  Professional sidebars always show this.
        {
            let items_total_h = self.items_y_offset(self.items.len(), text.scale);
            let track_x = sidebar.right() - s(5.0);
            let track_y = sidebar.y + header_h + s(4.0);
            let track_h = items_total_h - s(4.0);
            let track_w = s(2.0);
            if track_h > s(10.0) {
                // Track rail — nearly invisible at rest
                let track_rect = Rect { x: track_x, y: track_y, width: track_w, height: track_h };
                let any_hover = self.hovered_index.is_some();
                let track_alpha = if any_hover { 0.06 } else { 0.03 };
                ui.fill_rounded(track_rect,
                    [colors::BORDER[0], colors::BORDER[1], colors::BORDER[2], track_alpha],
                    track_w / 2.0);
                // Thumb — proportional, fades in on hover
                let visible_ratio = 1.0_f32; // all items visible for now
                let thumb_h = (track_h * visible_ratio).max(s(16.0)).min(track_h);
                let thumb_y = track_y; // scroll_offset * (track_h - thumb_h) for real scrolling
                let thumb_rect = Rect { x: track_x, y: thumb_y, width: track_w, height: thumb_h };
                let thumb_alpha = if any_hover { 0.18 } else { 0.08 };
                ui.fill_rounded(thumb_rect,
                    [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], thumb_alpha],
                    track_w / 2.0);
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

            // "PROCESSES" header (disclosure triangle + uppercase muted, matching SESSIONS style)
            let proc_disc_sz = ch * 0.55;
            let proc_disc_t = (0.8 * text.scale).max(0.5);
            let proc_disc_rect = Rect {
                x: sidebar.x + pad_h,
                y: panel_y + (header_section_h - proc_disc_sz) / 2.0,
                width: proc_disc_sz,
                height: proc_disc_sz,
            };
            ui.icon_disclosure_down(proc_disc_rect, proc_disc_sz, proc_disc_t,
                [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.45]);
            ui.text_ui(text, "PROCESSES",
                    sidebar.x + pad_h + proc_disc_sz + s(4.0),
                    panel_y + (header_section_h - ch) / 2.0,
                    [colors::FG_MUTED[0], colors::FG_MUTED[1], colors::FG_MUTED[2], 0.65],
                    colors::BG_DARK);
            // Agent count badge (right-aligned, pill-shaped — matches session count badge)
            let agent_count_str = format!("{}", self.agents.len());
            let agent_count_w = text.text_width(&agent_count_str);
            let agent_badge_pad_h = s(4.0);
            let agent_badge_h = ch * 0.85;
            let agent_badge_w = (agent_count_w + agent_badge_pad_h * 2.0).max(agent_badge_h);
            let agent_badge_x = sidebar.right() - pad_h - agent_badge_w;
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
