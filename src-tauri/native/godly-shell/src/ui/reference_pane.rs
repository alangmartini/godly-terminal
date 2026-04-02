//! Deterministic cropped web-reference transcript for visual parity captures.
//!
//! This scene intentionally matches the visible composition of
//! `docs/references/web-reference.png`, which is a top-left crop of the fuller
//! JSX prototype rather than the entire shell viewport.

use std::cell::RefCell;

use super::builder::{colors, font_scale, UiBuilder, UiTextRenderer};
use super::reference_layout::{
    ReferencePaneLayoutEngine, BLOCK_COMMAND, BLOCK_CURSOR, BLOCK_EDITING, BLOCK_INTRO,
    BLOCK_PARAGRAPH_COLLAPSE, BLOCK_PARAGRAPH_TONE, BLOCK_RESIDUAL_HEADING,
    BLOCK_RESIDUAL_NUMBERED, BLOCK_RESIDUAL_PARAGRAPH, BLOCK_SMOKE_BULLET, BLOCK_THOUGHTS_ONE,
    BLOCK_THOUGHTS_TWO, BLOCK_USER_COMPACT, BLOCK_USER_FUN, BLOCK_VERIFICATION_BULLET,
    BLOCK_VERIFICATION_HEADING,
};
use super::widget::Rect;

const BODY_SCALE: f32 = font_scale::PX13;
const SMALL_SCALE: f32 = font_scale::PX12;
const HEADING_SCALE: f32 = 1.0;
const LINK_COLOR: [f32; 4] = colors::ACCENT_SKY;
const CODE_BG: [f32; 4] = [0.110, 0.125, 0.188, 1.0]; // #1c2030
const BODY_LINE_HEIGHT: f32 = 13.0 * 1.55;
const PARAGRAPH_LINE_HEIGHT: f32 = 13.0 * 1.6;
const INLINE_GAP: f32 = 8.0;
const THOUGHTS_GAP: f32 = 6.0;
const SUB_BULLET_INDENT: f32 = 12.0;

pub struct ReferencePane {
    layout_engine: RefCell<ReferencePaneLayoutEngine>,
}

impl ReferencePane {
    pub fn new() -> Self {
        Self {
            layout_engine: RefCell::new(ReferencePaneLayoutEngine::new()),
        }
    }

    pub fn build(&self, ui: &mut UiBuilder, pane: Rect, text: &UiTextRenderer) {
        let bg = colors::BG_BASE;
        let s = |v: f32| text.s(v);
        let layout = self.layout_engine.borrow_mut().compute(pane, text);
        let body_x = layout.content.x;
        let bullet_text_x = body_x + self.inline_lead(text, "\u{2022}", BODY_SCALE, INLINE_GAP);

        self.draw_bullet_text(
            ui,
            text,
            layout.blocks[BLOCK_INTRO].x,
            layout.blocks[BLOCK_INTRO].y,
            "Tightened the support story so the README stays clearly tmux-first while still acknowledging the experimental zellij code in-repo.",
            bg,
        );

        self.draw_heading(
            ui,
            text,
            layout.blocks[BLOCK_VERIFICATION_HEADING].x,
            layout.blocks[BLOCK_VERIFICATION_HEADING].y,
            "Verification Notes",
            bg,
        );

        self.draw_bullet_runs(
            ui,
            text,
            layout.blocks[BLOCK_VERIFICATION_BULLET].x,
            layout.blocks[BLOCK_VERIFICATION_BULLET].y,
            &[
                InlineRun::Text("The rewritten "),
                InlineRun::Link("README.md"),
                InlineRun::Text(" now reflects the actual sidebar surface:"),
            ],
            bg,
        );
        self.draw_sub_bullets(
            ui,
            text,
            bullet_text_x + s(SUB_BULLET_INDENT),
            layout.blocks[BLOCK_VERIFICATION_BULLET].y + s(BODY_LINE_HEIGHT) + s(2.0),
            &[
                "session list shows branch and agent status",
                "detail panel shows working directory, ports, agent rows, and thread names",
                "users can hide/restore/kill/reorder sessions and switch themes",
                "detected localhost ports open from the UI",
            ],
            bg,
        );

        self.draw_bullet_runs(
            ui,
            text,
            layout.blocks[BLOCK_SMOKE_BULLET].x,
            layout.blocks[BLOCK_SMOKE_BULLET].y,
            &[
                InlineRun::Text("I also verified the smoke-test path directly: "),
                InlineRun::Code("bun test"),
                InlineRun::Text(" from the repo root passes right now with 167 passing tests."),
            ],
            bg,
        );

        self.draw_heading(
            ui,
            text,
            layout.blocks[BLOCK_RESIDUAL_HEADING].x,
            layout.blocks[BLOCK_RESIDUAL_HEADING].y,
            "One Residual Note",
            bg,
        );

        self.draw_paragraph(
            ui,
            text,
            layout.blocks[BLOCK_RESIDUAL_PARAGRAPH].x,
            layout.blocks[BLOCK_RESIDUAL_PARAGRAPH].y,
            "I only changed the README. While auditing, I noticed some secondary docs still have minor drift from the current UI behavior, especially the feature/keybinding docs.",
            bg,
        );

        self.draw_numbered_runs(
            ui,
            text,
            layout.blocks[BLOCK_RESIDUAL_NUMBERED].x,
            layout.blocks[BLOCK_RESIDUAL_NUMBERED].y,
            &[
                InlineRun::Text("If you want, I can do the same line-by-line cleanup pass on "),
                InlineRun::Link("docs/reference/features-and-keybindings.md"),
                InlineRun::Text(" and the rest of "),
                InlineRun::Code("docs/"),
                InlineRun::Text(" next."),
            ],
            bg,
        );

        self.draw_user_message(
            ui,
            text,
            layout.blocks[BLOCK_USER_FUN].x,
            layout.blocks[BLOCK_USER_FUN].y,
            "also make it a bit more fun! (interrupted)",
            bg,
        );
        self.draw_user_message(
            ui,
            text,
            layout.blocks[BLOCK_USER_COMPACT].x,
            layout.blocks[BLOCK_USER_COMPACT].y,
            "and compact!",
            bg,
        );

        self.draw_thoughts_row(
            ui,
            text,
            layout.blocks[BLOCK_THOUGHTS_ONE].x,
            layout.blocks[BLOCK_THOUGHTS_ONE].y,
            2,
            bg,
        );
        self.draw_paragraph(
            ui,
            text,
            layout.blocks[BLOCK_PARAGRAPH_TONE].x,
            layout.blocks[BLOCK_PARAGRAPH_TONE].y,
            "I'm tightening the README again, this time for tone and density: shorter sections, less repetition, and a little more personality without getting cute enough to obscure what the project does.",
            bg,
        );

        self.draw_command_row(
            ui,
            text,
            layout.blocks[BLOCK_COMMAND].x,
            layout.blocks[BLOCK_COMMAND].y,
            layout.blocks[BLOCK_COMMAND].width,
            "$ rtk proxy nl -ba README.md | sed -n '1,220p'",
            bg,
        );

        self.draw_thoughts_row(
            ui,
            text,
            layout.blocks[BLOCK_THOUGHTS_TWO].x,
            layout.blocks[BLOCK_THOUGHTS_TWO].y,
            2,
            bg,
        );
        self.draw_paragraph(
            ui,
            text,
            layout.blocks[BLOCK_PARAGRAPH_COLLAPSE].x,
            layout.blocks[BLOCK_PARAGRAPH_COLLAPSE].y,
            "I'm collapsing the README structure now: fewer headings, less \"verification report\" tone, and more \"here's why this is useful\" energy. The goal is that the first screen reads fast and still feels grounded in the actual code.",
            bg,
        );

        self.draw_editing_row(
            ui,
            text,
            layout.blocks[BLOCK_EDITING].x,
            layout.blocks[BLOCK_EDITING].y,
            bg,
        );
        self.draw_cursor(
            ui,
            text,
            layout.blocks[BLOCK_CURSOR].x,
            layout.blocks[BLOCK_CURSOR].y,
        );
    }

    fn draw_heading(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        label: &str,
        bg: [f32; 4],
    ) -> f32 {
        ui.text_mono_bold_scaled_mixed(text, label, x, y, colors::FG_BRIGHT, bg, HEADING_SCALE);
        y + text.s(14.0)
    }

    fn draw_paragraph(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        label: &str,
        bg: [f32; 4],
    ) -> f32 {
        ui.text_mono_scaled_mixed(text, label, x, y, colors::FG_PRIMARY, bg, BODY_SCALE);
        y + text.s(PARAGRAPH_LINE_HEIGHT)
    }

    fn draw_bullet_text(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        label: &str,
        bg: [f32; 4],
    ) -> f32 {
        ui.text_mono_scaled_mixed(text, "\u{2022}", x, y, colors::FG_MUTED, bg, BODY_SCALE);
        ui.text_mono_scaled_mixed(
            text,
            label,
            x + self.inline_lead(text, "\u{2022}", BODY_SCALE, INLINE_GAP),
            y,
            colors::FG_PRIMARY,
            bg,
            BODY_SCALE,
        );
        y + text.s(BODY_LINE_HEIGHT)
    }

    fn draw_bullet_runs(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        runs: &[InlineRun<'_>],
        bg: [f32; 4],
    ) -> f32 {
        ui.text_mono_scaled_mixed(text, "\u{2022}", x, y, colors::FG_MUTED, bg, BODY_SCALE);
        self.draw_inline_runs(
            ui,
            text,
            x + self.inline_lead(text, "\u{2022}", BODY_SCALE, INLINE_GAP),
            y,
            runs,
            BODY_SCALE,
            bg,
        );
        y + text.s(BODY_LINE_HEIGHT)
    }

    fn draw_sub_bullets(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        mut y: f32,
        lines: &[&str],
        bg: [f32; 4],
    ) -> f32 {
        for line in lines {
            ui.text_mono_scaled_mixed(
                text,
                "\u{2022}",
                x,
                y,
                colors::STATUS_DEFAULT,
                bg,
                SMALL_SCALE,
            );
            ui.text_mono_scaled_mixed(
                text,
                line,
                x + self.inline_lead(text, "\u{2022}", SMALL_SCALE, INLINE_GAP),
                y,
                colors::FG_SECONDARY,
                bg,
                SMALL_SCALE,
            );
            y += text.s(14.0);
        }
        y
    }

    fn draw_numbered_runs(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        runs: &[InlineRun<'_>],
        bg: [f32; 4],
    ) -> f32 {
        ui.text_mono_scaled_mixed(text, "1.", x, y, colors::FG_MUTED, bg, BODY_SCALE);
        self.draw_inline_runs(
            ui,
            text,
            x + self.inline_lead(text, "1.", BODY_SCALE, INLINE_GAP),
            y,
            runs,
            BODY_SCALE,
            bg,
        );
        y + text.s(BODY_LINE_HEIGHT)
    }

    fn draw_user_message(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        label: &str,
        bg: [f32; 4],
    ) -> f32 {
        let s = |v: f32| text.s(v);
        let text_h = s(13.0);
        let rect = Rect {
            x,
            y,
            width: text.text_width_mono_scaled(label, BODY_SCALE) + s(24.0),
            height: text_h + s(12.0),
        };
        ui.fill_rounded(
            Rect {
                x: rect.x + s(3.0),
                y: rect.y,
                width: rect.width - s(3.0),
                height: rect.height,
            },
            colors::BG_ACTIVE_ACC,
            s(4.0),
        );
        ui.fill(
            Rect {
                x: rect.x,
                y: rect.y,
                width: s(3.0),
                height: rect.height,
            },
            colors::ACCENT_BLUE,
        );
        ui.text_mono_scaled_mixed(
            text,
            label,
            rect.x + s(12.0),
            rect.y + s(6.0),
            [0.769, 0.698, 0.541, 1.0], // #c4b28a
            bg,
            BODY_SCALE,
        );
        rect.bottom()
    }

    fn draw_thoughts_row(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        count: u32,
        bg: [f32; 4],
    ) -> f32 {
        let label = format!("{count} thoughts >");
        ui.text_mono_scaled_mixed(
            text,
            "\u{2713}",
            x,
            y,
            colors::ACCENT_GREEN,
            bg,
            SMALL_SCALE,
        );
        ui.text_mono_scaled_mixed(
            text,
            &label,
            x + self.inline_lead(text, "\u{2713}", SMALL_SCALE, THOUGHTS_GAP),
            y,
            colors::FG_MUTED,
            bg,
            SMALL_SCALE,
        );
        y + text.s(13.0)
    }

    fn draw_command_row(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        width: f32,
        command: &str,
        bg: [f32; 4],
    ) -> f32 {
        let s = |v: f32| text.s(v);
        let rect = Rect {
            x,
            y,
            width,
            height: s(24.0),
        };
        ui.fill_rounded(rect, [0.039, 0.047, 0.063, 1.0], s(5.0)); // #0a0c10
        ui.stroke_rounded(rect, s(5.0), 1.0, [0.118, 0.129, 0.157, 1.0]); // #1e2128
        ui.text_mono_scaled_mixed(
            text,
            command,
            rect.x + s(10.0),
            rect.y + s(6.0),
            colors::FG_SECONDARY,
            bg,
            SMALL_SCALE,
        );
        let arrow = "\u{25B8}";
        let arrow_w = text.text_width_mono_scaled(arrow, SMALL_SCALE);
        ui.text_mono_scaled_mixed(
            text,
            arrow,
            rect.right() - arrow_w - s(10.0),
            rect.y + s(6.0),
            colors::STATUS_PATH,
            bg,
            SMALL_SCALE,
        );
        rect.bottom()
    }

    fn draw_editing_row(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        bg: [f32; 4],
    ) -> f32 {
        ui.text_mono_scaled_mixed(text, "::", x, y, colors::STATUS_PATH, bg, SMALL_SCALE);
        let lead = self.inline_lead(text, "::", SMALL_SCALE, THOUGHTS_GAP);
        ui.text_mono_scaled_mixed(
            text,
            "Editing files",
            x + lead,
            y,
            colors::FG_MUTED,
            bg,
            SMALL_SCALE,
        );
        ui.text_mono_scaled_mixed(
            text,
            "\u{25B8}",
            x + lead
                + text.text_width_mono_scaled("Editing files", SMALL_SCALE)
                + text.s(THOUGHTS_GAP),
            y,
            colors::STATUS_PATH,
            bg,
            SMALL_SCALE,
        );
        y + text.s(12.0)
    }

    fn draw_cursor(&self, ui: &mut UiBuilder, text: &UiTextRenderer, x: f32, y: f32) {
        ui.fill_rounded(
            Rect {
                x,
                y,
                width: text.s(8.0),
                height: text.s(16.0),
            },
            colors::ACCENT_BLUE,
            text.s(1.0),
        );
    }

    fn draw_inline_runs(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        mut x: f32,
        y: f32,
        runs: &[InlineRun<'_>],
        scale: f32,
        bg: [f32; 4],
    ) {
        let s = |v: f32| text.s(v);
        for run in runs {
            match run {
                InlineRun::Text(value) => {
                    ui.text_mono_scaled_mixed(text, value, x, y, colors::FG_PRIMARY, bg, scale);
                    x += text.text_width_mono_scaled(value, scale);
                }
                InlineRun::Link(value) => {
                    ui.text_mono_scaled_mixed(text, value, x, y, LINK_COLOR, bg, scale);
                    let width = text.text_width_mono_scaled(value, scale);
                    ui.hline_aa(
                        x,
                        y + text.cell_height * scale + s(1.0),
                        width,
                        1.0,
                        LINK_COLOR,
                    );
                    x += width;
                }
                InlineRun::Code(value) => {
                    let code_scale = SMALL_SCALE;
                    let pad_x = s(5.0);
                    let code_w = text.text_width_mono_scaled(value, code_scale);
                    let code_h = s(16.0);
                    let rect = Rect {
                        x,
                        y: y + s(1.0),
                        width: code_w + pad_x * 2.0,
                        height: code_h,
                    };
                    ui.fill_rounded(rect, CODE_BG, s(3.0));
                    ui.text_mono_scaled_mixed(
                        text,
                        value,
                        rect.x + pad_x,
                        rect.y + s(1.0),
                        colors::ACCENT_GOLD,
                        CODE_BG,
                        code_scale,
                    );
                    x += rect.width;
                }
            }
        }
    }

    fn inline_lead(&self, text: &UiTextRenderer, marker: &str, scale: f32, gap: f32) -> f32 {
        text.text_width_mono_scaled(marker, scale) + text.s(gap)
    }
}

enum InlineRun<'a> {
    Text(&'a str),
    Link(&'a str),
    Code(&'a str),
}
