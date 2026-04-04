//! Deterministic cropped web-reference transcript for visual parity captures.
//!
//! This scene intentionally matches the visible composition of
//! `docs/references/web-reference.png`, which is a top-left crop of the fuller
//! JSX prototype rather than the entire shell viewport.

use std::cell::RefCell;

use super::builder::{colors, UiBuilder, UiTextRenderer};
use super::reference_layout::{
    ReferencePaneLayoutEngine, BLOCK_COMMAND, BLOCK_COUNT, BLOCK_CURSOR, BLOCK_EDITING,
    BLOCK_INTRO, BLOCK_PARAGRAPH_COLLAPSE, BLOCK_PARAGRAPH_TONE, BLOCK_RESIDUAL_HEADING,
    BLOCK_RESIDUAL_NUMBERED, BLOCK_RESIDUAL_PARAGRAPH, BLOCK_SMOKE_BULLET, BLOCK_THOUGHTS_ONE,
    BLOCK_THOUGHTS_TWO, BLOCK_USER_COMPACT, BLOCK_USER_FUN, BLOCK_VERIFICATION_BULLET,
    BLOCK_VERIFICATION_HEADING,
};
use super::widget::Rect;

const BODY_SCALE: f32 = 13.0 / 14.0; // web: fontSize 13
const SMALL_SCALE: f32 = 12.0 / 14.0; // web: fontSize 12
const HEADING_SCALE: f32 = 14.0 / 14.0; // web: fontSize 14
const LINK_COLOR: [f32; 4] = colors::ACCENT_SKY;
const CODE_BG: [f32; 4] = [0.110, 0.125, 0.188, 1.0]; // #1c2030
const BODY_LINE_HEIGHT: f32 = 13.0 * 1.55; // web: fontSize 13, lineHeight 1.55
const PARAGRAPH_LINE_HEIGHT: f32 = 13.0 * 1.6; // web: fontSize 13, lineHeight 1.6
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

    pub fn build(&self, ui: &mut UiBuilder, pane: Rect, text: &UiTextRenderer, phase: f32) {
        let bg = colors::BG_BASE;
        let s = |v: f32| text.s(v);

        // Pre-compute line counts for blocks that may wrap.
        // Content width available for text (pane - 2*padding).
        let content_w = (pane.width - s(20.0) * 2.0).max(0.0);
        let bullet_lead = self.inline_lead(text, "\u{2022}", BODY_SCALE, INLINE_GAP);
        let para_avail = content_w;
        let bullet_avail = content_w - bullet_lead;

        let mut line_counts = [1usize; BLOCK_COUNT];
        line_counts[BLOCK_INTRO] = wrap_mono_line_count(
            "Tightened the support story so the README stays clearly tmux-first while still acknowledging the experimental zellij code in-repo.",
            bullet_avail, text, BODY_SCALE);
        line_counts[BLOCK_RESIDUAL_PARAGRAPH] = wrap_mono_line_count(
            "I only changed the README. While auditing, I noticed some secondary docs still have minor drift from the current UI behavior, especially the feature/keybinding docs.",
            para_avail, text, BODY_SCALE);
        line_counts[BLOCK_PARAGRAPH_TONE] = wrap_mono_line_count(
            "I\u{2019}m tightening the README again, this time for tone and density: shorter sections, less repetition, and a little more personality without getting cute enough to obscure what the project does.",
            para_avail, text, BODY_SCALE);
        line_counts[BLOCK_PARAGRAPH_COLLAPSE] = wrap_mono_line_count(
            "I\u{2019}m collapsing the README structure now: fewer headings, less \"verification report\" tone, and more \"here\u{2019}s why this is useful\" energy. The goal is that the first screen reads fast and still feels grounded in the actual code.",
            para_avail, text, BODY_SCALE);
        line_counts[BLOCK_VERIFICATION_BULLET] = inline_runs_line_count(
            &[InlineRun::Text("The rewritten "), InlineRun::Link("README.md"),
              InlineRun::Text(" now reflects the actual sidebar surface:")],
            bullet_avail, text, BODY_SCALE);
        line_counts[BLOCK_SMOKE_BULLET] = inline_runs_line_count(
            &[InlineRun::Text("I also verified the smoke-test path directly: "),
              InlineRun::Code("bun test"),
              InlineRun::Text(" from the repo root passes right now with 167 passing tests.")],
            bullet_avail, text, BODY_SCALE);
        let numbered_lead = self.inline_lead(text, "1.", BODY_SCALE, INLINE_GAP);
        let numbered_avail = content_w - numbered_lead;
        line_counts[BLOCK_RESIDUAL_NUMBERED] = inline_runs_line_count(
            &[InlineRun::Text("If you want, I can do the same line-by-line cleanup pass on "),
              InlineRun::Link("docs/reference/features-and-keybindings.md"),
              InlineRun::Text(" and the rest of "), InlineRun::Code("docs/"),
              InlineRun::Text(" next.")],
            numbered_avail, text, BODY_SCALE);

        let layout = self.layout_engine.borrow_mut().compute_wrapped(pane, text, &line_counts);
        let body_x = layout.content.x;
        let bullet_text_x = body_x + bullet_lead;

        self.draw_bullet_text(
            ui,
            text,
            layout.blocks[BLOCK_INTRO].x,
            layout.blocks[BLOCK_INTRO].y,
            layout.blocks[BLOCK_INTRO].width,
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

        let vb_end = self.draw_bullet_runs(
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
            content_w,
        );
        self.draw_sub_bullets(
            ui,
            text,
            bullet_text_x + s(SUB_BULLET_INDENT),
            vb_end + s(2.0),
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
            content_w,
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
            layout.blocks[BLOCK_RESIDUAL_PARAGRAPH].width,
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
            content_w,
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
            layout.blocks[BLOCK_PARAGRAPH_TONE].width,
            "I\u{2019}m tightening the README again, this time for tone and density: shorter sections, less repetition, and a little more personality without getting cute enough to obscure what the project does.",
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
            layout.blocks[BLOCK_PARAGRAPH_COLLAPSE].width,
            "I\u{2019}m collapsing the README structure now: fewer headings, less \"verification report\" tone, and more \"here\u{2019}s why this is useful\" energy. The goal is that the first screen reads fast and still feels grounded in the actual code.",
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
            phase,
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
        // Half-leading offset: (lineHeight 1.5 * fontSize 14 - fontSize 14) / 2 = 3.5
        let half_leading = text.s(3.5);
        ui.text_mono_bold_scaled_mixed(text, label, x, y + half_leading, colors::FG_BRIGHT, bg, HEADING_SCALE);
        ui.set_last_letter_spacing(0.2); // web: letterSpacing 0.2
        y + text.s(21.0) // web: fontSize 14 * lineHeight 1.5
    }

    fn draw_paragraph(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        width: f32,
        label: &str,
        bg: [f32; 4],
    ) -> f32 {
        let lines = wrap_mono_text(label, width, text, BODY_SCALE);
        let line_h = text.s(PARAGRAPH_LINE_HEIGHT);
        let mut cy = y;
        for line in &lines {
            ui.text_mono_scaled_mixed(text, line, x, cy, colors::FG_PRIMARY, bg, BODY_SCALE);
            cy += line_h;
        }
        cy
    }

    fn draw_bullet_text(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        width: f32,
        label: &str,
        bg: [f32; 4],
    ) -> f32 {
        let lead = self.inline_lead(text, "\u{2022}", BODY_SCALE, INLINE_GAP);
        ui.text_mono_scaled_mixed(text, "\u{2022}", x, y, colors::FG_MUTED, bg, BODY_SCALE);
        let text_avail = width - lead;
        let lines = wrap_mono_text(label, text_avail, text, BODY_SCALE);
        let line_h = text.s(BODY_LINE_HEIGHT);
        let mut cy = y;
        for line in &lines {
            ui.text_mono_scaled_mixed(text, line, x + lead, cy, colors::FG_PRIMARY, bg, BODY_SCALE);
            cy += line_h;
        }
        cy
    }

    fn draw_bullet_runs(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        runs: &[InlineRun<'_>],
        bg: [f32; 4],
        content_w: f32,
    ) -> f32 {
        let lead = self.inline_lead(text, "\u{2022}", BODY_SCALE, INLINE_GAP);
        let avail = content_w - lead;
        ui.text_mono_scaled_mixed(text, "\u{2022}", x, y, colors::FG_MUTED, bg, BODY_SCALE);
        self.draw_inline_runs(ui, text, x + lead, y, runs, BODY_SCALE, bg, avail)
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
        // Web: fontSize 12, inherited lineHeight 1.5, padding "1px 0"
        // Row height = 12 * 1.5 + 2 = 20.0 CSS pixels
        let half_leading = text.s(3.0); // (18 - 12) / 2
        let row_pad = text.s(1.0); // padding "1px 0" top
        let row_h = text.s(20.0); // 12 * 1.5 + 2.0
        for line in lines {
            let text_y = y + row_pad + half_leading;
            ui.text_mono_scaled_mixed(
                text,
                "\u{2022}",
                x,
                text_y,
                colors::STATUS_DEFAULT,
                bg,
                SMALL_SCALE,
            );
            ui.text_mono_scaled_mixed(
                text,
                line,
                x + self.inline_lead(text, "\u{2022}", SMALL_SCALE, INLINE_GAP),
                text_y,
                colors::FG_SECONDARY,
                bg,
                SMALL_SCALE,
            );
            y += row_h;
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
        content_w: f32,
    ) -> f32 {
        let lead = self.inline_lead(text, "1.", BODY_SCALE, INLINE_GAP);
        let avail = content_w - lead;
        ui.text_mono_scaled_mixed(text, "1.", x, y, colors::FG_MUTED, bg, BODY_SCALE);
        self.draw_inline_runs(ui, text, x + lead, y, runs, BODY_SCALE, bg, avail)
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
        // Web: fontSize 13, inherited lineHeight 1.5, padding "6px 12px"
        // Total height = 13 * 1.5 + 12 = 31.5; half-leading = (19.5 - 13) / 2 = 3.25
        let rect = Rect {
            x,
            y,
            width: text.text_width_mono_scaled(label, BODY_SCALE) + s(24.0),
            height: s(31.5),
        };
        // Web: borderLeft "3px solid #6366f1", borderRadius "0 4px 4px 0"
        // Single SDF quad with per-corner radii + left-only border
        ui.fill_rounded_custom_border_sides(
            rect,
            colors::BG_ACTIVE_ACC,
            [0.0, s(4.0), s(4.0), 0.0],    // radii: [TL, TR, BR, BL]
            [0.0, 0.0, 0.0, s(3.0)],        // border_widths: [top, right, bottom, left]
            colors::ACCENT_BLUE,
        );
        ui.text_mono_medium_scaled_mixed(
            text,
            label,
            rect.x + s(12.0),
            rect.y + s(9.25), // 6px padding + 3.25px half-leading
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
        let label = format!("{count} thoughts \u{25B8}");
        // Web: checkmark at fontSize 13, label at fontSize 12, inherited lineHeight 1.5
        // Half-leading = (12 * 1.5 - 12) / 2 = 3.0
        let half_leading = text.s(3.0);
        ui.text_mono_scaled_mixed(
            text,
            "\u{2713}",
            x,
            y + half_leading,
            colors::ACCENT_GREEN,
            bg,
            BODY_SCALE,
        );
        ui.text_mono_scaled_mixed(
            text,
            &label,
            x + self.inline_lead(text, "\u{2713}", BODY_SCALE, THOUGHTS_GAP),
            y + half_leading,
            colors::FG_MUTED,
            bg,
            SMALL_SCALE,
        );
        y + text.s(18.0) // web: fontSize 12 * lineHeight 1.5
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
        // Web: fontSize 12, inherited lineHeight 1.5, padding "6px 10px"
        // Total height = 12 * 1.5 + 12 = 30; text y = 6px padding + 3px half-leading = 9px
        let rect = Rect {
            x,
            y,
            width,
            height: s(30.0),
        };
        ui.fill_rounded(rect, [0.039, 0.047, 0.063, 1.0], s(5.0)); // #0a0c10
        ui.stroke_rounded(rect, s(5.0), 1.0, [0.118, 0.129, 0.157, 1.0]); // #1e2128
        ui.text_mono_scaled_mixed(
            text,
            command,
            rect.x + s(10.0),
            rect.y + s(9.0), // 6px padding + 3px half-leading
            colors::FG_MUTED, // web: color "#6e7681"
            bg,
            SMALL_SCALE,
        );
        let arrow = "\u{25B8}";
        let arrow_w = text.text_width_mono_scaled(arrow, SMALL_SCALE);
        ui.text_mono_scaled_mixed(
            text,
            arrow,
            rect.right() - arrow_w - s(10.0),
            rect.y + s(9.0), // 6px padding + 3px half-leading
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
        // Web: fontSize 12, inherited lineHeight 1.5; half-leading = 3.0
        let half_leading = text.s(3.0);
        ui.text_mono_scaled_mixed(text, "::", x, y + half_leading, colors::STATUS_PATH, bg, SMALL_SCALE);
        let lead = self.inline_lead(text, "::", SMALL_SCALE, THOUGHTS_GAP);
        ui.text_mono_scaled_mixed(
            text,
            "Editing files",
            x + lead,
            y + half_leading,
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
            y + half_leading,
            colors::STATUS_PATH,
            bg,
            SMALL_SCALE,
        );
        y + text.s(18.0) // web: fontSize 12 * lineHeight 1.5
    }

    fn draw_cursor(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        x: f32,
        y: f32,
        phase: f32,
    ) {
        // Web: width 8, height 16, backgroundColor #6366f1, borderRadius 1,
        // animation: blink 1s infinite — 0%,100% opacity:1, 50% opacity:0
        // glow_phase runs at TAU/3.5 rad/s. Convert to 1s blink cycle:
        // time_seconds = phase / (TAU / 3.5) = phase * 3.5 / TAU
        let time_s = phase * 3.5 / std::f32::consts::TAU;
        let visible = (time_s % 1.0) < 0.5;
        if !visible {
            return;
        }
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

    /// Draw inline runs with word-wrapping. Continuation lines wrap to `start_x`
    /// (the bullet/number indent level). Returns the y position after the last line.
    fn draw_inline_runs(
        &self,
        ui: &mut UiBuilder,
        text: &UiTextRenderer,
        start_x: f32,
        y: f32,
        runs: &[InlineRun<'_>],
        scale: f32,
        bg: [f32; 4],
        avail_width: f32,
    ) -> f32 {
        let s = |v: f32| text.s(v);
        let mut cx = start_x;
        let mut cy = y;
        let right_edge = start_x + avail_width;
        let line_h = text.s(BODY_LINE_HEIGHT);

        for run in runs {
            match run {
                InlineRun::Text(value) => {
                    let mut remaining = *value;
                    while !remaining.is_empty() {
                        let avail = (right_edge - cx).max(0.0);
                        let full_w = text.text_width_mono_scaled(remaining, scale);
                        if full_w <= avail {
                            ui.text_mono_scaled_mixed(
                                text, remaining, cx, cy, colors::FG_PRIMARY, bg, scale,
                            );
                            cx += full_w;
                            break;
                        }
                        let brk = find_word_break(remaining, avail, text, scale);
                        if brk > 0 {
                            let piece = &remaining[..brk];
                            ui.text_mono_scaled_mixed(
                                text, piece, cx, cy, colors::FG_PRIMARY, bg, scale,
                            );
                            remaining = remaining[brk..].trim_start();
                        } else if cx > start_x + 0.5 {
                            // No break found but not at line start — wrap
                        } else {
                            // At line start, no break — render and move on
                            ui.text_mono_scaled_mixed(
                                text, remaining, cx, cy, colors::FG_PRIMARY, bg, scale,
                            );
                            cx += full_w;
                            break;
                        }
                        cy += line_h;
                        cx = start_x;
                    }
                }
                InlineRun::Link(value) => {
                    let width = text.text_width_mono_scaled(value, scale);
                    if cx + width > right_edge && cx > start_x + 0.5 {
                        cy += line_h;
                        cx = start_x;
                    }
                    ui.text_mono_scaled_mixed(text, value, cx, cy, LINK_COLOR, bg, scale);
                    ui.hline_aa(
                        cx,
                        cy + text.cell_height * scale + s(1.0),
                        width,
                        1.0,
                        LINK_COLOR,
                    );
                    cx += width;
                }
                InlineRun::Code(value) => {
                    let code_scale = SMALL_SCALE;
                    let pad_x = s(5.0);
                    let code_w = text.text_width_mono_scaled(value, code_scale);
                    let total_w = code_w + pad_x * 2.0;
                    if cx + total_w > right_edge && cx > start_x + 0.5 {
                        cy += line_h;
                        cx = start_x;
                    }
                    let code_h = s(14.0); // web: fontSize 12 + padding 1px*2
                    let rect = Rect {
                        x: cx,
                        y: cy + s(1.0),
                        width: total_w,
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
                    cx += rect.width;
                }
            }
        }
        cy + line_h
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

/// Find the byte offset of the last space in `s` whose rendered prefix fits
/// within `avail` pixels.  Returns 0 if no suitable break point exists.
fn find_word_break(s: &str, avail: f32, text: &UiTextRenderer, scale: f32) -> usize {
    let mut last_space = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if ch == ' ' {
            let prefix_w = text.text_width_mono_scaled(&s[..byte_idx], scale);
            if prefix_w <= avail {
                last_space = byte_idx;
            } else {
                break;
            }
        }
    }
    last_space
}

/// Pre-compute how many visual lines a set of inline runs will occupy when
/// word-wrapped within `avail_width` pixels. `avail_width` should already
/// account for the bullet/number marker lead.
fn inline_runs_line_count(
    runs: &[InlineRun<'_>],
    avail_width: f32,
    text: &UiTextRenderer,
    scale: f32,
) -> usize {
    let mut lines = 1usize;
    let right_edge = avail_width;
    let mut cx: f32 = 0.0;

    for run in runs {
        match run {
            InlineRun::Text(value) => {
                let mut remaining = *value;
                while !remaining.is_empty() {
                    let avail = (right_edge - cx).max(0.0);
                    let full_w = text.text_width_mono_scaled(remaining, scale);
                    if full_w <= avail {
                        cx += full_w;
                        break;
                    }
                    let brk = find_word_break(remaining, avail, text, scale);
                    if brk > 0 {
                        remaining = remaining[brk..].trim_start();
                    } else if cx > 0.5 {
                        // wrap without consuming text
                    } else {
                        // forced: can't break, just advance
                        cx += full_w;
                        break;
                    }
                    lines += 1;
                    cx = 0.0;
                }
            }
            InlineRun::Link(value) => {
                let w = text.text_width_mono_scaled(value, scale);
                if cx + w > right_edge && cx > 0.5 {
                    lines += 1;
                    cx = 0.0;
                }
                cx += w;
            }
            InlineRun::Code(value) => {
                let code_w = text.text_width_mono_scaled(value, SMALL_SCALE)
                    + text.s(5.0) * 2.0;
                if cx + code_w > right_edge && cx > 0.5 {
                    lines += 1;
                    cx = 0.0;
                }
                cx += code_w;
            }
        }
    }
    lines
}

/// Compute how many wrapped lines a monospace text block occupies at the given
/// scale within `avail_width` pixels.
fn wrap_mono_line_count(text_str: &str, avail_width: f32, text: &UiTextRenderer, scale: f32) -> usize {
    wrap_mono_text(text_str, avail_width, text, scale).len().max(1)
}

/// Word-wrap a monospace text string into lines that fit within `avail_width`.
/// Breaks at word boundaries (spaces). Falls back to character-break for very
/// long words.
fn wrap_mono_text<'a>(text_str: &'a str, avail_width: f32, text: &UiTextRenderer, scale: f32) -> Vec<&'a str> {
    let char_w = text.text_width_mono_scaled("M", scale);
    if char_w <= 0.0 || avail_width <= 0.0 {
        return vec![text_str];
    }
    let chars_per_line = (avail_width / char_w).floor().max(1.0) as usize;

    if text_str.chars().count() <= chars_per_line {
        return vec![text_str];
    }

    let mut lines = Vec::new();
    let mut line_start = 0;
    let mut line_chars = 0;
    let mut last_space_byte = None;
    let mut last_space_chars = 0;

    for (byte_idx, ch) in text_str.char_indices() {
        if ch == ' ' {
            last_space_byte = Some(byte_idx);
            last_space_chars = line_chars;
        }
        line_chars += 1;
        if line_chars > chars_per_line {
            if let Some(space_byte) = last_space_byte {
                if space_byte > line_start {
                    lines.push(&text_str[line_start..space_byte]);
                    line_start = space_byte + 1; // skip the space
                    line_chars = line_chars - last_space_chars - 1;
                    last_space_byte = None;
                    continue;
                }
            }
            // No space found — break at character boundary
            lines.push(&text_str[line_start..byte_idx]);
            line_start = byte_idx;
            line_chars = 1;
            last_space_byte = None;
        }
    }
    if line_start < text_str.len() {
        lines.push(&text_str[line_start..]);
    }
    lines
}
