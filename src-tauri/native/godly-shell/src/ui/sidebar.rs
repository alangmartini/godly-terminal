//! Left sidebar: session list with names, active indicator, and new session button.

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::widget::{Rect, UiAction, MouseEvent};

const HEADER_HEIGHT: f32 = 38.0;
const ITEM_HEIGHT: f32 = 50.0;
const ITEM_HEIGHT_COMPACT: f32 = 34.0;
const ITEM_PADDING_H: f32 = 14.0;
const ACTIVE_INDICATOR_W: f32 = 3.0;
const BOTTOM_PANEL_HEIGHT: f32 = 160.0;

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
    pub hovered_new: bool,
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
            hovered_new: false,
        }
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

        // Sidebar background
        ui.fill(sidebar, colors::BG_DARK);

        // Right border separator
        ui.vline(sidebar.right() - 1.0, sidebar.y, sidebar.height, 1.0, colors::BORDER);

        // "Sessions" header with count
        let header_rect = Rect {
            x: sidebar.x,
            y: sidebar.y,
            width: sidebar.width,
            height: header_h,
        };
        ui.text(
            text,
            "Sessions",
            header_rect.x + pad_h,
            header_rect.y + text_y_off(header_h),
            colors::FG_MUTED,
            colors::BG_DARK,
        );
        // Session count (right-aligned in header)
        let count_str = format!("{}", self.items.len());
        let count_w = text.text_width(&count_str);
        ui.text(
            text,
            &count_str,
            header_rect.right() - count_w - pad_h,
            header_rect.y + text_y_off(header_h),
            colors::FG_MUTED,
            colors::BG_DARK,
        );
        // Header bottom separator
        ui.hline(sidebar.x + pad_h, header_rect.bottom() - 1.0,
                 sidebar.width - pad_h * 2.0, 1.0, colors::BORDER);

        // Layout: [pad][num][gap][name...][gap][branch][pad]
        // Two-line items: line 1 = number + name + branch, line 2 = description
        let num_x = sidebar.x + pad_h;
        let name_x = num_x + cw * 2.0;
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

            // Hover background (rounded)
            let item_radius = s(4.0);
            let inset_rect = Rect {
                x: rect.x + s(6.0),
                y: rect.y + s(2.0),
                width: rect.width - s(12.0),
                height: rect.height - s(4.0),
            };
            if self.hovered_index == Some(i) && !item.active {
                ui.fill_rounded(inset_rect, colors::BG_HOVER, item_radius);
            }

            // Active item background (rounded with subtle border)
            if item.active {
                ui.fill_rounded_bordered(
                    inset_rect, colors::BG_ACTIVE, item_radius,
                    0.5, colors::BORDER,
                );
            }

            // Active indicator (left colored bar, pill shape via SDF)
            if item.active {
                let indicator_rect = Rect {
                    x: rect.x + s(3.0),
                    y: rect.y + s(8.0),
                    width: indicator_w,
                    height: rect.height - s(16.0),
                };
                ui.fill_rounded(indicator_rect, colors::ACCENT_BLUE, indicator_w / 2.0);
            }

            // Text y position: centered for compact, top-aligned for two-line
            let text_y = if item.description.is_empty() {
                rect.y + text_y_off(this_item_h)
            } else {
                rect.y + line1_y_off
            };

            // Session number
            let item_bg = if item.active { colors::BG_ACTIVE } else if self.hovered_index == Some(i) { colors::BG_HOVER } else { colors::BG_DARK };
            let num_str = format!("{}", item.number);
            let fg = if item.active { colors::ACCENT_BLUE } else { colors::FG_MUTED };
            ui.text(text, &num_str,
                    num_x,
                    text_y,
                    fg, item_bg);

            // Session name (truncated to fit)
            let name_fg = if item.active { colors::FG_PRIMARY } else { colors::FG_SECONDARY };
            let name = if item.label.len() > name_max_chars {
                format!("{}\u{2026}", &item.label[..name_max_chars.saturating_sub(1)])
            } else {
                item.label.clone()
            };
            ui.text(text, &name,
                    name_x,
                    text_y,
                    name_fg, item_bg);

            // Branch info (right-aligned, truncated)
            if !item.branch.is_empty() && sidebar.width > s(150.0) {
                let branch = if item.branch.len() > branch_max_chars {
                    format!("{}\u{2026}", &item.branch[..branch_max_chars - 1])
                } else {
                    item.branch.clone()
                };
                let branch_w = text.text_width(&branch);
                ui.text(text, &branch,
                        rect.right() - branch_w - pad_h,
                        text_y,
                        colors::FG_MUTED, item_bg);
            }

            // Description line (second row, muted)
            if !item.description.is_empty() {
                let desc_max_chars = ((sidebar.width - pad_h * 2.0 - cw * 2.0) / cw).floor().max(1.0) as usize;
                let desc = if item.description.len() > desc_max_chars {
                    format!("{}\u{2026}", &item.description[..desc_max_chars.saturating_sub(1)])
                } else {
                    item.description.clone()
                };
                ui.text(text, &desc,
                        name_x,
                        rect.y + line2_y_off,
                        colors::FG_MUTED, item_bg);
            }
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
        let new_bg = if self.hovered_new { colors::BG_SURFACE } else { colors::BG_DARK };
        if self.hovered_new {
            ui.fill_rounded(new_rect, colors::BG_SURFACE, s(4.0));
        }
        ui.text(text, "+ New Session",
                new_rect.x + s(8.0),
                new_rect.y + text_y_off(compact_h),
                colors::FG_MUTED, new_bg);

        // Bottom panel: running agents/processes
        if !self.agents.is_empty() {
            let agent_item_h = s(44.0);
            let header_section_h = s(28.0);
            let agent_panel_h = header_section_h + self.agents.len() as f32 * agent_item_h + s(8.0);
            // Anchor agent panel to the actual bottom of the sidebar
            let panel_y = sidebar.bottom() - agent_panel_h;
            let panel = Rect {
                x: sidebar.x,
                y: panel_y,
                width: sidebar.width,
                height: agent_panel_h.min(bottom_panel_h),
            };

            // Panel background (slightly raised)
            ui.fill(panel, colors::BG_RAISED);

            // Top separator
            ui.hline(sidebar.x + pad_h, panel_y, sidebar.width - pad_h * 2.0, 1.0, colors::BORDER);

            // "Processes" header
            ui.text(text, "Processes",
                    sidebar.x + pad_h,
                    panel_y + (header_section_h - ch) / 2.0,
                    colors::FG_MUTED, colors::BG_RAISED);

            let mut ay = panel_y + header_section_h;
            for agent in &self.agents {
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

                // Line 1: icon + agent name + status (right-aligned)
                let line1_y = ay + s(6.0);

                let panel_bg = colors::BG_RAISED;

                // Status indicator icon
                ui.text(text, agent.icon, sidebar.x + pad_h, line1_y, status_color, panel_bg);

                // Agent name
                ui.text(text, &agent.name,
                        sidebar.x + pad_h + cw * 2.0,
                        line1_y,
                        colors::FG_SECONDARY, panel_bg);

                // Status label (right-aligned)
                let sw = text.text_width(status_text);
                ui.text(text, status_text,
                        sidebar.right() - sw - pad_h - 1.0,
                        line1_y,
                        status_color, panel_bg);

                // Line 2: task description (muted, indented)
                if !agent.task.is_empty() && agent.task != status_text {
                    let line2_y = line1_y + ch + s(2.0);
                    let task_max_chars = ((sidebar.width - pad_h * 2.0 - cw * 2.0) / cw).floor().max(1.0) as usize;
                    let task = if agent.task.len() > task_max_chars {
                        format!("{}\u{2026}", &agent.task[..task_max_chars.saturating_sub(1)])
                    } else {
                        agent.task.clone()
                    };
                    ui.text(text, &task,
                            sidebar.x + pad_h + cw * 2.0,
                            line2_y,
                            colors::FG_MUTED, panel_bg);
                }

                ay += agent_item_h;
            }
        }
    }

    pub fn on_mouse(&mut self, event: MouseEvent, sidebar: Rect, scale: f32) -> Option<UiAction> {
        if sidebar.width < 1.0 { return None; }
        match event {
            MouseEvent::Move { x, y } => {
                self.hovered_index = None;
                self.hovered_new = false;
                for (i, _) in self.items.iter().enumerate() {
                    if self.item_rect(i, sidebar, scale).contains(x, y) { self.hovered_index = Some(i); }
                }
                if self.new_button_rect(sidebar, scale).contains(x, y) { self.hovered_new = true; }
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
