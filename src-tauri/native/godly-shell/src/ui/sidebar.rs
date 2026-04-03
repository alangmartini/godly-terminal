//! Left sidebar: session list with names, active indicator, and new session button.

use super::anim::{self, lerp_color, AnimVec};
use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::text_layout::FontWeight;
use super::sidebar_layout::{
    compute_sidebar_session_layout, SessionStackItemSpec, SidebarSessionLayout, ACTIVE_BORDER_W,
    HEADER_LABEL_FONT_PX, HEADER_LIGHTNING_FONT_PX, HEADER_PAD_X, LIST_PAD_X, ROW_GAP_X,
    SESSION_NAME_FONT_PX, SESSION_NUMBER_FONT_PX, SESSION_NUMBER_MIN_WIDTH,
    SESSION_SECONDARY_FONT_PX,
};
use super::widget::{MouseEvent, Rect, UiAction};

const BOTTOM_PANEL_HEIGHT: f32 = 160.0;
/// Estimated height of the bottom action-shortcuts bar (2 rows + padding).
const SHORTCUTS_BAR_HEIGHT: f32 = 42.0;

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
    pub show_footer_sections: bool,
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
                    icon: "!",
                    name: "amp".into(),
                    task: "Verify and clean README documentation".into(),
                    status: AgentStatus::Running,
                },
                AgentItem {
                    icon: "\u{26A0}",
                    name: "amp".into(),
                    task: "Verify README against codebase".into(),
                    status: AgentStatus::Stopped,
                },
                AgentItem {
                    icon: "\u{25CF}",
                    name: "claude-code".into(),
                    task: String::new(),
                    status: AgentStatus::Waiting,
                },
            ],
            hovered_index: None,
            hovered_agent: None,
            show_footer_sections: true,
            glow_phase: 0.0,
        }
    }

    /// Advance all hover animations. `dt` = seconds since last frame. Returns `true` if still animating.
    pub fn tick_animations(&mut self, dt: f32) -> bool {
        let hl = anim::timing::HOVER;
        self.item_hover_anim.ensure_len(self.items.len());
        for i in 0..self.items.len() {
            self.item_hover_anim.set(
                i,
                if self.hovered_index == Some(i) {
                    1.0
                } else {
                    0.0
                },
            );
        }
        self.agent_hover_anim.ensure_len(self.agents.len());
        for i in 0..self.agents.len() {
            self.agent_hover_anim.set(
                i,
                if self.hovered_agent == Some(i) {
                    1.0
                } else {
                    0.0
                },
            );
        }
        self.active_anim.ensure_len(self.items.len());
        for i in 0..self.items.len() {
            self.active_anim
                .set(i, if self.items[i].active { 1.0 } else { 0.0 });
        }
        let mut animating = false;
        animating |= self.active_anim.tick(hl, dt);
        animating |= self.item_hover_anim.tick(hl, dt);
        animating |= self.agent_hover_anim.tick(hl, dt);

        // Ambient breathing glow: ~3.5s period, frame-rate independent.
        let has_active = self.items.iter().any(|i| i.active)
            || self
                .agents
                .iter()
                .any(|a| matches!(a.status, AgentStatus::Running));
        if has_active {
            self.glow_phase += dt * std::f32::consts::TAU / 3.5;
            if self.glow_phase > std::f32::consts::TAU {
                self.glow_phase -= std::f32::consts::TAU;
            }
            animating = true;
        }

        animating
    }

    fn session_layout(&self, sidebar: Rect, scale: f32) -> SidebarSessionLayout {
        let specs: Vec<SessionStackItemSpec> = self
            .items
            .iter()
            .map(|item| SessionStackItemSpec {
                has_secondary: !item.branch.is_empty() || !item.description.is_empty(),
            })
            .collect();
        compute_sidebar_session_layout(sidebar, scale, &specs)
    }

    pub fn build(&self, ui: &mut UiBuilder, sidebar: Rect, text: &UiTextRenderer) {
        if sidebar.width < 1.0 {
            return;
        }

        let s = |v: f32| text.s(v);
        let ch = text.cell_height;
        let pad_h = s(HEADER_PAD_X);
        let session_layout = self.session_layout(sidebar, text.scale);
        let _bottom_panel_h = s(BOTTOM_PANEL_HEIGHT);

        // Sidebar background — flat fill matching web reference
        // (backgroundColor: "#0b0d12").
        ui.fill(sidebar, colors::BG_DARK);

        // (Convexity gradient removed — flat surface matches Zed/VS Code restraint)

        // Right border separator — solid 1px hairline matching web reference
        // (borderRight: "1px solid #1a1d25").
        ui.vline(
            sidebar.right() - 1.0,
            sidebar.y,
            sidebar.height,
            1.0,
            colors::BORDER,
        );
        // (Web reference uses clean flat sidebar with no shadow effects)

        // "Sessions {count} ⚡ 1" header — inline mixed-case matching web reference
        // Web: padding "12px 14px 4px", display "flex", gap 6, fontSize 12, color "#6e7681"
        let header_label = format!("Sessions {}", self.items.len());
        let header_label_h = s(HEADER_LABEL_FONT_PX);
        let header_lightning_h = s(HEADER_LIGHTNING_FONT_PX);
        let header_text_x = session_layout.header_content.x;
        let header_text_y = session_layout.header_content.y
            + (session_layout.header_content.height - header_label_h) / 2.0;
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
        let lightning_x =
            header_text_x + text.text_width_ui_scaled(&header_label, font_scale::PX12) + s(6.0);
        ui.text_ui_scaled(
            text,
            "\u{26A1} 1",
            lightning_x,
            session_layout.header_content.y
                + (session_layout.header_content.height - header_lightning_h) / 2.0,
            colors::STATUS_DEFAULT, // #484f58
            colors::BG_DARK,
            font_scale::PX10,
        ); // web: fontSize 10

        // Truncate a string to fit within max_w pixels using exact per-glyph widths.
        // Returns the truncated string with ellipsis if needed.
        let truncate_to_width = |s: &str, max_w: f32, text: &UiTextRenderer| -> String {
            let full_w = text.text_width_ui(s);
            if full_w <= max_w {
                return s.to_string();
            }
            let ellipsis_w = text.text_width_ui("\u{2026}");
            let target_w = max_w - ellipsis_w;
            let mut w = 0.0f32;
            let mut end = 0;
            for (i, ch) in s.char_indices() {
                let ch_w = text.text_width_ui(&s[i..i + ch.len_utf8()]);
                if w + ch_w > target_w {
                    break;
                }
                w += ch_w;
                end = i + ch.len_utf8();
            }
            format!("{}\u{2026}", &s[..end])
        };
        for (i, item) in self.items.iter().enumerate() {
            let Some(layout_item) = session_layout.items.get(i) else {
                continue;
            };

            // Hover background (rounded, animated)
            let item_radius = s(6.0);
            let hover_t = self.item_hover_anim.get(i);
            let active_t = self.active_anim.get(i);
            // Inactive hover state — flat fill, no glow/gradient/border
            // Web: backgroundColor transitions to a subtle surface tone on hover
            if active_t < 0.995 && hover_t > 0.005 {
                let inv_active = 1.0 - active_t;
                let hover_bg = [
                    colors::BG_SURFACE[0],
                    colors::BG_SURFACE[1],
                    colors::BG_SURFACE[2],
                    hover_t * inv_active,
                ];
                ui.fill_rounded(layout_item.outer, hover_bg, item_radius);
            }

            // Active state — flat background + 3px left accent bar
            // Web reference always uses #6366f1 (indigo/ACCENT_BLUE) for active border
            if active_t > 0.005 {
                // Flat active background (#171b24)
                let active_bg = lerp_color(colors::BG_DARK, colors::BG_ACTIVE, active_t);
                ui.fill_rounded(layout_item.outer, active_bg, item_radius);

                // 3px flat left border — full-height like the web reference.
                let indicator_rect = Rect {
                    x: layout_item.outer.x,
                    y: layout_item.outer.y,
                    width: s(ACTIVE_BORDER_W),
                    height: layout_item.outer.height,
                };
                ui.fill_rounded(
                    indicator_rect,
                    [
                        colors::ACCENT_BLUE[0],
                        colors::ACCENT_BLUE[1],
                        colors::ACCENT_BLUE[2],
                        active_t,
                    ],
                    s(1.5),
                );
            }

            // Session number — text bg must exactly match the drawn background
            // so ClearType subpixel compositing has no visible color fringing.
            let item_bg = lerp_color(
                lerp_color(colors::BG_DARK, colors::BG_SURFACE, hover_t),
                colors::BG_ACTIVE,
                active_t,
            );
            let num_str = format!("{}", item.number);
            let number_h = s(SESSION_NUMBER_FONT_PX);
            let name_h = s(SESSION_NAME_FONT_PX);
            let secondary_h = s(SESSION_SECONDARY_FONT_PX);
            let number_col_w = s(SESSION_NUMBER_MIN_WIDTH)
                .max(text.text_width_ui_weighted_scaled(&num_str, font_scale::PX12, FontWeight::Medium));
            let num_x = layout_item.first_row.x;
            let num_y = layout_item.first_row.y + (layout_item.first_row.height - number_h) / 2.0;
            let name_x = num_x + number_col_w + s(ROW_GAP_X);
            let name_y = layout_item.first_row.y + (layout_item.first_row.height - name_h) / 2.0;

            // Session number — web: fontSize 12, fontWeight 500, color #555d6b (FG_DIM)
            let num_fg = lerp_color(colors::FG_DIM, colors::FG_SECONDARY, hover_t * 0.5);
            ui.text_ui_medium_scaled(
                text,
                &num_str,
                num_x,
                num_y,
                num_fg,
                item_bg,
                font_scale::PX12,
            );

            // Session name (truncated to fit) — web: #e6edf3 active, #9198a1 inactive
            let inactive_name = lerp_color(colors::FG_INACTIVE, colors::FG_PRIMARY, hover_t * 0.6);
            let name_fg = lerp_color(inactive_name, colors::FG_BRIGHT, active_t); // web: active #e6edf3, inactive #9198a1
            let indicator_text_w = if item.active {
                text.text_width_ui_scaled("::", font_scale::PX11)
            } else {
                0.0
            };
            let name_max_w = (layout_item.first_row.right()
                - name_x
                - if item.active { indicator_text_w } else { 0.0 })
            .max(s(30.0));
            let name = truncate_to_width(&item.label, name_max_w, text);
            // Web reference: fontWeight 600, fontSize 13 for all session names
            ui.text_ui_semibold_scaled(
                text,
                &name,
                name_x,
                name_y,
                name_fg,
                item_bg,
                font_scale::PX13,
            );
            if item.active {
                // Web reference: "::" indicator right-aligned, fontSize 11, color #484f58
                let indicator_x = layout_item.first_row.right() - indicator_text_w;
                ui.text_ui_scaled(
                    text,
                    "::",
                    indicator_x,
                    layout_item.first_row.y + (layout_item.first_row.height - secondary_h) / 2.0,
                    colors::STATUS_DEFAULT,
                    item_bg,
                    font_scale::PX11,
                );
            }

            // Second line: branch (web: paddingLeft ~20px, color #484f58)
            // Web always shows branch below the session name.
            if !item.branch.is_empty() {
                let branch_rect = layout_item.secondary_row.unwrap_or(layout_item.first_row);
                let branch_fg = lerp_color(
                    colors::STATUS_DEFAULT, // #484f58 — matches web exactly
                    colors::FG_DIM,
                    hover_t * 0.4 + active_t * 0.3,
                );
                ui.text_ui_scaled(
                    text,
                    &item.branch,
                    branch_rect.x,
                    branch_rect.y + (branch_rect.height - secondary_h) / 2.0,
                    branch_fg,
                    item_bg,
                    font_scale::PX11,
                );
            } else if !item.description.is_empty() {
                let branch_rect = layout_item.secondary_row.unwrap_or(layout_item.first_row);
                let desc_avail = branch_rect.width.min(sidebar.width - s(LIST_PAD_X * 2.0));
                let desc = truncate_to_width(&item.description, desc_avail, text);
                let desc_fg =
                    lerp_color(colors::STATUS_DEFAULT, colors::FG_DIM, 0.40 + hover_t * 0.3);
                ui.text_ui_scaled(
                    text,
                    &desc,
                    branch_rect.x,
                    branch_rect.y + (branch_rect.height - secondary_h) / 2.0,
                    desc_fg,
                    item_bg,
                    font_scale::PX11,
                );
            }

            // Web reference: no separator lines between sessions, just marginBottom spacing
        }

        // Scrollbar — matches web CSS: ::-webkit-scrollbar { width: 6px }
        // Track: transparent, Thumb: #2d333b (BG_HOVER), hover: #3b4048, radius: 3px
        {
            let track_w = s(6.0);
            let track_x = sidebar.right() - track_w - s(2.0);
            let track_y = session_layout.list.y;
            let track_h = (session_layout.items_bottom() - session_layout.list.y).max(0.0);
            let radius = s(3.0);
            if track_h > s(10.0) {
                // Track: transparent (web: background: transparent) — no fill
                // Thumb — only shown when content overflows (matches web CSS behavior)
                let any_hover = self.hovered_index.is_some();
                let visible_ratio = 1.0_f32; // all items visible for now
                if visible_ratio < 1.0 {
                    let thumb_h = (track_h * visible_ratio).max(s(16.0)).min(track_h);
                    let thumb_y = track_y; // scroll_offset * (track_h - thumb_h) for real scrolling
                    let thumb_rect = Rect {
                        x: track_x,
                        y: thumb_y,
                        width: track_w,
                        height: thumb_h,
                    };
                    // Web: thumb #2d333b, hover #3b4048
                    let thumb_color = if any_hover {
                        [0.231, 0.251, 0.282, 1.0] // #3b4048
                    } else {
                        [
                            colors::BG_HOVER[0],
                            colors::BG_HOVER[1],
                            colors::BG_HOVER[2],
                            1.0,
                        ] // #2d333b
                    };
                    ui.fill_rounded(thumb_rect, thumb_color, radius);
                }
            }
        }

        // Session list bottom fade — gradient overlay for smooth visual clipping
        // before the New Session button. Professional sidebars (Zed, VS Code) use
        // this to indicate scrollable content.
        {
            let fade_h = s(16.0);
            let fade_y = session_layout.items_bottom() - fade_h;
            if fade_y > session_layout.list.y {
                let fade_rect = Rect {
                    x: sidebar.x,
                    y: fade_y,
                    width: sidebar.width - s(6.0), // leave room for scrollbar
                    height: fade_h,
                };
                ui.fill_gradient(
                    fade_rect,
                    [
                        colors::BG_DARK[0],
                        colors::BG_DARK[1],
                        colors::BG_DARK[2],
                        0.0,
                    ],
                    [
                        colors::BG_DARK[0],
                        colors::BG_DARK[1],
                        colors::BG_DARK[2],
                        0.5,
                    ],
                );
            }
        }

        // Bottom panel: running agents/processes — matches web reference layout:
        // borderTop "1px solid #1a1d25", directory path header, two-line items
        // with icon + name + status badge (right-aligned) + × + wrapped description.
        if self.show_footer_sections && !self.agents.is_empty() {
            let header_section_h = s(24.0); // directory path header
            let shortcuts_h = s(SHORTCUTS_BAR_HEIGHT);
            let panel_bg = colors::BG_DARK;
            let desc_sc = font_scale::PX11;
            let desc_line_h = ch * desc_sc * 1.3; // web: lineHeight 1.3

            // Pre-compute per-agent heights (dynamic based on description wrapping)
            let desc_indent = s(20.0); // web: paddingLeft 20px
            let desc_avail = sidebar.width - desc_indent - pad_h;
            let agent_heights: Vec<f32> = self
                .agents
                .iter()
                .map(|agent| {
                    let main_row_h = ch + s(2.0); // icon line + gap
                    let desc_h = if agent.task.is_empty() {
                        0.0
                    } else {
                        let lines = wrap_ui_text(&agent.task, desc_avail, text, desc_sc);
                        lines.max(1) as f32 * desc_line_h
                    };
                    s(5.0) * 2.0 + main_row_h + desc_h // top/bottom padding + content
                })
                .collect();
            let agent_panel_h: f32 =
                header_section_h + agent_heights.iter().sum::<f32>() + s(4.0);
            let panel_y = sidebar.bottom() - shortcuts_h - agent_panel_h;

            // Solid top border — web: borderTop "1px solid #1a1d25"
            ui.hline(sidebar.x, panel_y, sidebar.width, 1.0, colors::BORDER);

            // Directory path header — web: padding "8px 10px 4px", fontSize 10,
            // color "#484f58", letterSpacing 0.5
            ui.text_ui_scaled(
                text,
                "\u{2026}ments/work/opensessions",
                sidebar.x + s(10.0),
                panel_y + s(4.0),
                colors::STATUS_DEFAULT, // #484f58
                panel_bg,
                font_scale::PX10,
            ); // web: fontSize 10
            ui.set_last_letter_spacing(0.5); // web: letterSpacing 0.5

            let mut ay = panel_y + header_section_h;
            for (ai, agent) in self.agents.iter().enumerate() {
                let status_color = match agent.status {
                    AgentStatus::Running => colors::ACCENT_GREEN,
                    AgentStatus::Waiting => colors::ACCENT_BLUE,
                    AgentStatus::Stopped => colors::ACCENT_RED,
                };
                let status_text = match agent.status {
                    AgentStatus::Running => "running",
                    AgentStatus::Waiting => "waiting",
                    AgentStatus::Stopped => "stopped",
                };

                let item_h = agent_heights[ai];
                let agent_hover_t = self.agent_hover_anim.get(ai);
                let agent_inset = Rect {
                    x: sidebar.x + s(6.0),
                    y: ay + s(1.0),
                    width: sidebar.width - s(12.0),
                    height: item_h - s(2.0),
                };
                if agent_hover_t > 0.005 {
                    let ahover_bg = [
                        colors::BG_SURFACE[0],
                        colors::BG_SURFACE[1],
                        colors::BG_SURFACE[2],
                        agent_hover_t,
                    ];
                    ui.fill_rounded(agent_inset, ahover_bg, s(3.0));
                }

                // First line: icon + name ... status badge (right-aligned) + ×
                let line1_y = ay + s(5.0);

                // Status icon — web uses text symbols: ⓘ (running), ⚠ (stopped), ● (waiting)
                let icon_str = match agent.status {
                    AgentStatus::Running => "\u{24D8}", // ⓘ
                    AgentStatus::Stopped => "\u{26A0}", // ⚠
                    AgentStatus::Waiting => "\u{25CF}", // ●
                };
                ui.text_ui_scaled(
                    text,
                    icon_str,
                    sidebar.x + s(10.0),
                    line1_y,
                    status_color,
                    panel_bg,
                    font_scale::PX11,
                ); // web: fontSize 11

                // Agent name — web: color "#9198a1", fontWeight 600
                let agent_name_x = sidebar.x
                    + s(10.0)
                    + text.text_width_ui_scaled(icon_str, font_scale::PX11)
                    + s(6.0);
                let agent_name_fg = lerp_color(
                    colors::FG_INACTIVE, // #9198a1
                    colors::FG_PRIMARY,
                    agent_hover_t * 0.4,
                );
                ui.text_ui_semibold_scaled(
                    text,
                    &agent.name,
                    agent_name_x,
                    line1_y,
                    agent_name_fg,
                    panel_bg,
                    font_scale::PX12,
                ); // web: fontSize 12, fontWeight 600

                // Dismiss × — web: color "#3b4048", fontSize 13, placed at far right
                let dismiss_w = text.text_width_ui_scaled("\u{00D7}", font_scale::PX13);
                let dismiss_x = sidebar.right() - pad_h - dismiss_w;
                ui.text_ui_scaled(
                    text,
                    "\u{00D7}",
                    dismiss_x,
                    line1_y,
                    colors::STATUS_PATH, // #3b4048
                    panel_bg,
                    font_scale::PX13,
                ); // web: fontSize 13

                // Status badge — right-aligned before ×
                // Web: fontSize 10, borderRadius 3, backgroundColor: color+"18",
                //       marginLeft "auto" pushes badge to the right
                let sw = text.text_width_ui_weighted_scaled(status_text, font_scale::PX10, FontWeight::SemiBold);
                let status_badge_pad_h = s(6.0); // web: padding "1px 6px" horizontal
                let status_badge_pad_v = s(1.0); // web: padding "1px 6px" vertical
                let status_badge_h = ch * font_scale::PX10 + status_badge_pad_v * 2.0;
                let status_badge_w = sw + status_badge_pad_h * 2.0;
                let status_badge_x = dismiss_x - s(6.0) - status_badge_w;
                let status_badge_y = line1_y + (ch - status_badge_h) / 2.0;
                let status_badge_rect = Rect {
                    x: status_badge_x,
                    y: status_badge_y,
                    width: status_badge_w,
                    height: status_badge_h,
                };
                let status_badge_r = s(3.0); // web: borderRadius 3
                // web: backgroundColor color+"18" → 0x18/0xFF ≈ 0.094 alpha
                let status_bg =
                    [status_color[0], status_color[1], status_color[2], 0.094];
                ui.fill_rounded(status_badge_rect, status_bg, status_badge_r);
                let status_text_x = status_badge_x + status_badge_pad_h;
                let status_text_y =
                    status_badge_y + (status_badge_h - ch * font_scale::PX10) / 2.0;
                ui.text_ui_semibold_scaled(
                    text,
                    status_text,
                    status_text_x,
                    status_text_y,
                    status_color,
                    panel_bg,
                    font_scale::PX10,
                ); // web: fontSize 10, fontWeight 600

                // Description lines — web: fontSize 11, color "#555d6b",
                // paddingLeft 20, lineHeight 1.3, word-wrapped
                if !agent.task.is_empty() {
                    let desc_x = sidebar.x + desc_indent;
                    let mut desc_y = line1_y + ch + s(2.0);
                    let lines = wrap_ui_text_lines(&agent.task, desc_avail, text, desc_sc);
                    for line in &lines {
                        ui.text_ui_scaled(
                            text,
                            line,
                            desc_x,
                            desc_y,
                            colors::STATUS_DEFAULT, // #484f58 — matches web exactly
                            panel_bg,
                            desc_sc,
                        );
                        desc_y += desc_line_h;
                    }
                }

                ay += item_h;

                // Separator between items — web: borderBottom "1px solid #13161d"
                if ai + 1 < self.agents.len() {
                    let sep_color = [0.075, 0.086, 0.114, 1.0]; // #13161d
                    ui.hline(sidebar.x, ay - 1.0, sidebar.width, 1.0, sep_color);
                }
            }
        }

        // Action shortcuts bar — anchored to very bottom, matching web reference
        // Web: borderTop "1px solid #1a1d25", padding "6px 10px",
        //      flexWrap "wrap", gap "4px 10px", fontSize 10, color "#3b4048"
        if self.show_footer_sections {
            let shortcuts: &[&str] = &[
                "~ cycle",
                "\u{2298} go",
                "d remove",
                "u restore",
                "x kill",
                "t theme",
            ];
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
            let shortcuts_h =
                shortcut_top_pad * 2.0 + rows as f32 * line_h + (rows - 1) as f32 * shortcut_gap_y;
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
                ui.text_ui_scaled(
                    text,
                    sc,
                    cx,
                    cy,
                    colors::STATUS_PATH,
                    colors::BG_DARK,
                    sc_scale,
                );
                cx += w + shortcut_gap_x;
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, sidebar: Rect, scale: f32) -> Option<UiAction> {
        if sidebar.width < 1.0 {
            return None;
        }
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_index = None;
                self.hovered_agent = None;
                let session_layout = self.session_layout(sidebar, scale);
                for (i, _) in self.items.iter().enumerate() {
                    if session_layout
                        .item_rect(i)
                        .is_some_and(|rect| rect.contains(x, y))
                    {
                        self.hovered_index = Some(i);
                    }
                }
                // Agent item hover detection — approximate heights (no text renderer here)
                if self.show_footer_sections && !self.agents.is_empty() {
                    let s = |v: f32| (v * scale).round();
                    // Approximate: each agent item ~44px base + ~14px per extra desc line
                    let header_section_h = s(24.0);
                    let shortcuts_h = s(SHORTCUTS_BAR_HEIGHT);
                    let base_item_h = s(44.0);
                    let total_h: f32 = self.agents.len() as f32 * base_item_h;
                    let agent_panel_h = header_section_h + total_h + s(4.0);
                    let panel_y = sidebar.bottom() - shortcuts_h - agent_panel_h;
                    let mut ay = panel_y + header_section_h;
                    for i in 0..self.agents.len() {
                        let agent_rect = Rect {
                            x: sidebar.x,
                            y: ay,
                            width: sidebar.width,
                            height: base_item_h,
                        };
                        if agent_rect.contains(x, y) {
                            self.hovered_agent = Some(i);
                        }
                        ay += base_item_h;
                    }
                }
                None
            }
            MouseEvent::Press { x, y } => {
                let session_layout = self.session_layout(sidebar, scale);
                for (i, item) in self.items.iter().enumerate() {
                    if session_layout
                        .item_rect(i)
                        .is_some_and(|rect| rect.contains(x, y))
                    {
                        return Some(UiAction::SwitchTab(item.id.clone()));
                    }
                }
                None
            }
            _ => None,
        }
    }
}

/// Count how many lines a proportional-font string wraps to within `avail` pixels.
fn wrap_ui_text(s: &str, avail: f32, text: &UiTextRenderer, scale: f32) -> usize {
    if s.is_empty() || avail <= 0.0 {
        return 0;
    }
    let mut lines = 1usize;
    let mut line_w = 0.0f32;
    for word in s.split_whitespace() {
        let w = text.text_width_ui_scaled(word, scale);
        let sp = text.text_width_ui_scaled(" ", scale);
        if line_w > 0.0 && line_w + sp + w > avail {
            lines += 1;
            line_w = w;
        } else {
            line_w += if line_w > 0.0 { sp } else { 0.0 } + w;
        }
    }
    lines
}

/// Word-wrap a proportional-font string and return the resulting lines.
fn wrap_ui_text_lines<'a>(s: &'a str, avail: f32, text: &UiTextRenderer, scale: f32) -> Vec<String> {
    if s.is_empty() || avail <= 0.0 {
        return vec![];
    }
    let sp_w = text.text_width_ui_scaled(" ", scale);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0f32;
    for word in s.split_whitespace() {
        let w = text.text_width_ui_scaled(word, scale);
        if cur_w > 0.0 && cur_w + sp_w + w > avail {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
            cur_w = w;
        } else {
            if !cur.is_empty() {
                cur.push(' ');
                cur_w += sp_w;
            }
            cur.push_str(word);
            cur_w += w;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
