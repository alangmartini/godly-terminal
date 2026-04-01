//! Left sidebar: session list with names, active indicator, and new session button.

use super::anim::{self, AnimVec, lerp_color, lerp};
use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const HEADER_HEIGHT: f32 = 30.0;
const ITEM_HEIGHT: f32 = 46.0;       // web: padding 7+7 + name 16 + gap 2 + branch 13 = ~45px
const ITEM_HEIGHT_COMPACT: f32 = 32.0; // web: padding 7+7 + name 16 = ~30px
const ITEM_PADDING_H: f32 = 14.0;
const ACTIVE_INDICATOR_W: f32 = 3.5;
const BOTTOM_PANEL_HEIGHT: f32 = 160.0;
/// Estimated height of the bottom action-shortcuts bar (2 rows + padding).
const SHORTCUTS_BAR_HEIGHT: f32 = 42.0;

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
    // Smooth animation state
    item_hover_anim: AnimVec,
    active_anim: AnimVec,
    agent_hover_anim: AnimVec,
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
                    branch: "fix/pdf-export_".into(),
                    description: "".into(),
                    active: false,
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
                    active: true,
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
            ],
            item_hover_anim: AnimVec::default(),
            active_anim: AnimVec::default(),
            agent_hover_anim: AnimVec::default(),
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
        self.active_anim.ensure_len(self.items.len());
        for i in 0..self.items.len() {
            self.active_anim.set(i, if self.items[i].active { 1.0 } else { 0.0 });
        }
        let mut animating = false;
        animating |= self.active_anim.tick(hl, dt);
        animating |= self.item_hover_anim.tick(hl, dt);
        animating |= self.agent_hover_anim.tick(hl, dt);

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

        // "Sessions {count} ⚡ 1" header — inline mixed-case matching web reference
        // Web: padding "12px 14px 4px", display "flex", gap 6, fontSize 12, color "#6e7681"
        let header_rect = Rect {
            x: sidebar.x,
            y: sidebar.y,
            width: sidebar.width,
            height: header_h,
        };
        let header_label = format!("Sessions {}", self.items.len());
        let header_text_x = header_rect.x + pad_h;
        let header_text_y = header_rect.y + text_y_off(header_h);
        ui.text_ui_scaled(
            text,
            &header_label,
            header_text_x,
            header_text_y,
            colors::FG_MUTED, // web: color "#6e7681" at full opacity
            colors::BG_DARK,
            font_scale::PX12, // web: fontSize 12
        );
        // Web: <span style={{ color: "#484f58", fontSize: 10 }}>⚡ 1</span>
        let lightning_x = header_text_x + text.text_width_ui_scaled(&header_label, font_scale::PX12) + s(6.0);
        ui.text_ui_scaled(text, "\u{26A1} 1", lightning_x, header_text_y,
            colors::STATUS_DEFAULT, // #484f58
            colors::BG_DARK,
            font_scale::PX10); // web: fontSize 10

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

            // Session number — web: fontSize 12, fontWeight 500, color #555d6b (FG_DIM)
            let num_str = format!("{}", item.number);
            let num_fg = lerp_color(colors::FG_DIM, colors::FG_SECONDARY, hover_t * 0.5);
            ui.text_ui_scaled(text, &num_str, num_x, text_y, num_fg, item_bg, font_scale::PX12);

            // Session name (truncated to fit) — web: #e6edf3 active, #9198a1 inactive
            let inactive_name = lerp_color(colors::FG_SECONDARY, colors::FG_BRIGHT, hover_t * 0.6);
            let name_fg = lerp_color(inactive_name, colors::FG_BRIGHT, active_t); // web: #e6edf3, not white
            let name_max_w = (rect.right() - name_x - pad_h).max(s(30.0));
            let name = truncate_to_width(&item.label, name_max_w, text);
            // Web reference: fontWeight 600, fontSize 13 for all session names
            ui.text_ui_bold_scaled(text, &name, name_x, text_y, name_fg, item_bg, font_scale::PX13);
            if item.active {
                // Web reference: "::" indicator right-aligned, fontSize 11, color #484f58
                let indicator_x = inset_rect.right() - text.text_width_ui_scaled("::", font_scale::PX11) - s(4.0);
                ui.text_ui_scaled(text, "::", indicator_x, text_y, colors::STATUS_DEFAULT, item_bg, font_scale::PX11);
            }

            // Second line: branch (web: paddingLeft ~20px, color #484f58)
            // Web always shows branch below the session name.
            if !item.branch.is_empty() {
                let branch_x = name_x; // indented to align with name
                // Web: color #484f58 (STATUS_DEFAULT), brightens slightly on hover/active
                let branch_fg = lerp_color(
                    colors::STATUS_DEFAULT, // #484f58
                    colors::FG_MUTED,
                    hover_t * 0.4 + active_t * 0.3,
                );
                ui.text_ui_scaled(text, &item.branch, branch_x, rect.y + line2_y_off, branch_fg, item_bg, font_scale::PX11);
            } else if !item.description.is_empty() {
                let desc_avail = sidebar.width - pad_h * 2.0 - ui_cw * 2.0;
                let desc = truncate_to_width(&item.description, desc_avail, text);
                let desc_fg = lerp_color(colors::FG_MUTED, colors::FG_SECONDARY, 0.40 + hover_t * 0.3);
                ui.text_ui_scaled(text, &desc, name_x, rect.y + line2_y_off, desc_fg, item_bg, font_scale::PX11);
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

        // Bottom panel: running agents/processes — matches web reference layout:
        // borderTop "1px solid #1a1d25", directory path header, two-line items
        // with icon + name + status badge + description.
        if !self.agents.is_empty() {
            // Web: each process item is ~48px (two lines: name+badge ~24px + desc ~24px)
            // plus 5px top/bottom padding and 1px separator
            let agent_item_h = s(48.0);
            let header_section_h = s(24.0); // directory path header
            let agent_panel_h = header_section_h + self.agents.len() as f32 * agent_item_h + s(4.0);
            let shortcuts_h = s(SHORTCUTS_BAR_HEIGHT);
            let panel_y = sidebar.bottom() - shortcuts_h - agent_panel_h;

            // Solid top border — web: borderTop "1px solid #1a1d25"
            ui.hline(sidebar.x, panel_y, sidebar.width, 1.0, colors::BORDER);

            // Directory path header — web: padding "8px 10px 4px", fontSize 10,
            // color "#484f58", letterSpacing 0.5
            ui.text_ui_scaled(text, "\u{2026}ments/work/opensessions",
                    sidebar.x + s(10.0),
                    panel_y + s(4.0),
                    colors::STATUS_DEFAULT, // #484f58
                    colors::BG_DARK,
                    font_scale::PX10); // web: fontSize 10

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

                // First line: icon + name + status badge + dismiss ×
                let line1_y = ay + s(5.0);
                let panel_bg = colors::BG_DARK;

                // Status icon — web uses text symbols: ⓘ (running), ⚠ (stopped), ● (waiting)
                let icon_str = match agent.status {
                    AgentStatus::Running => "\u{24D8}", // ⓘ
                    AgentStatus::Stopped => "\u{26A0}", // ⚠
                    AgentStatus::Waiting => "\u{25CF}", // ●
                };
                ui.text_ui_scaled(text, icon_str,
                    sidebar.x + s(10.0), line1_y,
                    status_color, panel_bg,
                    font_scale::PX11); // web: fontSize 11

                // Agent name — web: color "#9198a1", fontWeight 600
                let agent_name_x = sidebar.x + s(10.0) + text.text_width_ui_scaled(icon_str, font_scale::PX11) + s(6.0);
                let agent_name_fg = lerp_color(
                    [0.569, 0.596, 0.631, 1.0], // #9198a1
                    colors::FG_PRIMARY,
                    agent_hover_t * 0.4,
                );
                ui.text_ui_bold_scaled(text, &agent.name, agent_name_x, line1_y,
                    agent_name_fg, panel_bg, font_scale::PX12); // web: fontSize 12

                // Status badge — web: fontSize 10, borderRadius 3 (NOT pill),
                // backgroundColor: color + "18" (~0.094 opacity)
                let sw = text.text_width_ui_scaled(status_text, font_scale::PX10);
                let status_badge_pad_h = s(6.0);
                let status_badge_h = ch * font_scale::PX10;
                let status_badge_w = sw + status_badge_pad_h * 2.0;
                let name_end_x = agent_name_x + text.text_width_ui_scaled(&agent.name, font_scale::PX12) + s(6.0);
                let status_badge_x = name_end_x;
                let status_badge_y = line1_y + (ch - status_badge_h) / 2.0;
                let status_badge_rect = Rect {
                    x: status_badge_x, y: status_badge_y,
                    width: status_badge_w, height: status_badge_h,
                };
                let status_badge_r = s(3.0); // web: borderRadius 3, not pill
                let status_bg = [status_color[0], status_color[1], status_color[2], 0.094]; // web: "18" hex = ~9.4%
                ui.fill_rounded(status_badge_rect, status_bg, status_badge_r);
                let status_text_x = status_badge_x + status_badge_pad_h;
                let status_text_y = status_badge_y + (status_badge_h - ch) / 2.0;
                ui.text_ui_scaled(text, status_text,
                        status_text_x, status_text_y,
                        status_color, panel_bg,
                        font_scale::PX10); // web: fontSize 10

                // Dismiss × button — web: color "#3b4048", marginLeft auto, fontSize 13
                let dismiss_x = sidebar.right() - pad_h - text.text_width_ui_scaled("\u{00D7}", font_scale::PX13);
                ui.text_ui_scaled(text, "\u{00D7}", dismiss_x, line1_y,
                    colors::STATUS_PATH, // #3b4048
                    panel_bg,
                    font_scale::PX13); // web: fontSize 13

                // Second line: task description — web: fontSize 11, color "#484f58",
                // paddingLeft 20, lineHeight 1.3
                if !agent.task.is_empty() {
                    let desc_x = sidebar.x + s(10.0) + text.text_width_ui_scaled(icon_str, font_scale::PX11) + s(14.0);
                    let desc_y = line1_y + ch + s(2.0);
                    let desc_avail = sidebar.width - (desc_x - sidebar.x) - pad_h;
                    let desc = truncate_to_width(&agent.task, desc_avail, text);
                    ui.text_ui_scaled(text, &desc, desc_x, desc_y,
                        colors::STATUS_DEFAULT, // #484f58
                        panel_bg,
                        font_scale::PX11); // web: fontSize 11
                }

                ay += agent_item_h;

                // Separator between items — web: borderBottom "1px solid #13161d"
                if !std::ptr::eq(agent, self.agents.last().unwrap()) {
                    let sep_color = [0.075, 0.086, 0.114, 1.0]; // #13161d
                    ui.hline(sidebar.x, ay - 1.0, sidebar.width, 1.0, sep_color);
                }
            }
        }

        // Action shortcuts bar — anchored to very bottom, matching web reference
        // Web: borderTop "1px solid #1a1d25", padding "6px 10px",
        //      flexWrap "wrap", gap "4px 10px", fontSize 10, color "#3b4048"
        {
            let shortcuts: &[&str] = &["~ cycle", "\u{2298} go", "d remove", "u restore", "x kill", "t theme"];
            let shortcut_gap_x = s(10.0);
            let shortcut_gap_y = s(4.0);
            let shortcut_pad = s(10.0);
            let shortcut_top_pad = s(6.0);

            // Measure total height needed (wrap layout)
            let avail_w = sidebar.width - shortcut_pad * 2.0;
            let sc_scale = font_scale::PX10; // web: fontSize 10
            let mut rows = 1u32;
            let mut row_x = 0.0f32;
            for (i, &sc) in shortcuts.iter().enumerate() {
                let w = text.text_width_ui_scaled(sc, sc_scale);
                if i > 0 && row_x + w > avail_w {
                    rows += 1;
                    row_x = w + shortcut_gap_x;
                } else {
                    row_x += w + if i > 0 { shortcut_gap_x } else { 0.0 };
                }
            }
            let line_h = ch * sc_scale;
            let shortcuts_h = shortcut_top_pad * 2.0 + rows as f32 * line_h + (rows - 1) as f32 * shortcut_gap_y;
            let shortcuts_y = sidebar.bottom() - shortcuts_h;

            // Top border — solid 1px matching web borderTop
            ui.hline(sidebar.x, shortcuts_y, sidebar.width, 1.0, colors::BORDER);

            // Render shortcuts in wrapping row layout
            let mut cx = sidebar.x + shortcut_pad;
            let mut cy = shortcuts_y + shortcut_top_pad;
            for (i, &sc) in shortcuts.iter().enumerate() {
                let w = text.text_width_ui_scaled(sc, sc_scale);
                if i > 0 && (cx - sidebar.x - shortcut_pad) + w > avail_w {
                    cx = sidebar.x + shortcut_pad;
                    cy += line_h + shortcut_gap_y;
                }
                // Web: color "#3b4048" = STATUS_PATH
                ui.text_ui_scaled(text, sc, cx, cy, colors::STATUS_PATH, colors::BG_DARK, sc_scale);
                cx += w + shortcut_gap_x;
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, sidebar: Rect, scale: f32) -> Option<UiAction> {
        if sidebar.width < 1.0 { return None; }
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_index = None;
                self.hovered_agent = None;
                for (i, _) in self.items.iter().enumerate() {
                    if self.item_rect(i, sidebar, scale).contains(x, y) { self.hovered_index = Some(i); }
                }
                // Agent item hover detection
                if !self.agents.is_empty() {
                    let agent_item_h = (48.0 * scale).round();
                    let header_section_h = (24.0 * scale).round();
                    let shortcuts_h = (SHORTCUTS_BAR_HEIGHT * scale).round();
                    let agent_panel_h = header_section_h + self.agents.len() as f32 * agent_item_h + (4.0 * scale).round();
                    let panel_y = sidebar.bottom() - shortcuts_h - agent_panel_h;
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
