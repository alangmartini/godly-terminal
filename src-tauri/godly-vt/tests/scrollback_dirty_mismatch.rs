/// Bug #445: Dirty-flag row-index mismatch when scrollback_offset > 0.
///
/// When the user scrolls into scrollback history (scrollback_offset > 0),
/// the `dirty_rows` vector tracks drawing-row indices (active grid), but
/// `visible_cell()` / `visible_rows()` returns scrollback-blended rows.
///
/// Fix strategy (two parts):
///   1. `set_scrollback()` now calls `mark_all_dirty()` when the offset
///      changes, so the viewport shift itself triggers a full repaint.
///   2. The daemon's `extract_diff()` / `read_rich_grid_diff()` forces
///      `full_repaint = true` when `scrollback_offset > 0`, reading ALL
///      visible rows via `cell()` instead of relying on dirty flag indices.
///      This avoids the drawing-row vs visible-row index mismatch entirely.

/// Helper: create a parser with scrollback enabled.
fn parser_with_scrollback(rows: u16, cols: u16, scrollback_len: usize) -> godly_vt::Parser {
    let mut parser = godly_vt::Parser::new(rows, cols, scrollback_len);
    parser.process(b""); // ensure rows are allocated
    parser
}

/// Helper: consume dirty flags and return them.
fn take_dirty(parser: &mut godly_vt::Parser) -> Vec<bool> {
    parser.screen_mut().take_dirty_rows()
}

/// Helper: count how many rows are dirty.
fn count_dirty(flags: &[bool]) -> usize {
    flags.iter().filter(|&&d| d).count()
}

/// Helper: get visible row contents as strings.
fn visible_row_contents(parser: &godly_vt::Parser, cols: u16) -> Vec<String> {
    parser.screen().rows(0, cols).collect()
}

/// Helper: get the content of a specific visible cell.
fn visible_cell_content(parser: &godly_vt::Parser, row: u16, col: u16) -> String {
    parser
        .screen()
        .cell(row, col)
        .map(|c| c.contents().to_string())
        .unwrap_or_default()
}

// ── Test: set_scrollback must mark all rows dirty ──────────────────────

#[test]
fn set_scrollback_should_mark_all_rows_dirty() {
    // Bug #445: set_scrollback() changes which rows are visible (the entire
    // viewport shifts from live view to scrollback-blended view), but does NOT
    // call mark_all_dirty(). This means the next diff snapshot has no dirty
    // flags and sends nothing, leaving the frontend with stale cached data.
    let mut parser = parser_with_scrollback(5, 20, 50);

    // Fill screen + scrollback: write lines 1..15 into a 5-row terminal
    // This pushes 10 lines into scrollback, active grid shows lines 11-15
    for i in 1..=15 {
        if i > 1 {
            parser.process(b"\r\n");
        }
        parser.process(format!("line{:02}", i).as_bytes());
    }

    // Consume initial dirty flags
    let _ = take_dirty(&mut parser);
    assert!(!parser.screen().has_dirty_rows(), "should be clean after take");

    // Now scroll into history — this changes ALL visible rows
    parser.screen_mut().set_scrollback(3);

    // Verify the viewport actually changed
    let rows = visible_row_contents(&parser, 20);
    assert_eq!(rows[0].trim(), "line08", "first visible row should be from scrollback");

    // Bug: set_scrollback does NOT mark rows dirty, so the diff system
    // doesn't know the viewport changed. All rows SHOULD be dirty.
    assert!(
        parser.screen().has_dirty_rows(),
        "Bug #445: set_scrollback() must mark all rows dirty because the \
         entire visible viewport changed, but it doesn't"
    );

    let flags = take_dirty(&mut parser);
    assert_eq!(
        count_dirty(&flags),
        5,
        "Bug #445: all 5 rows should be dirty after set_scrollback, got {} dirty",
        count_dirty(&flags)
    );
}

// ── Test: dirty flags don't align with visible rows when scrolled ────────
//
// Dirty flags track DRAWING row indices, but cell() returns VISIBLE
// (scrollback-blended) rows. The daemon works around this by forcing a
// full repaint when scrollback_offset > 0 (reading ALL visible rows).

#[test]
fn dirty_flags_track_drawing_rows_not_visible_rows_when_scrolled() {
    // Bug #445: Verify the mismatch exists and that a full repaint
    // (reading all visible rows) produces correct data regardless.
    //
    // Setup: 5-row terminal, scrollback_offset = 3
    //   visible_row(0) = scrollback[-3] = "line08"
    //   visible_row(3) = drawing_row(0)  = "line11"
    let mut parser = parser_with_scrollback(5, 20, 50);

    for i in 1..=15 {
        if i > 1 {
            parser.process(b"\r\n");
        }
        parser.process(format!("line{:02}", i).as_bytes());
    }

    parser.screen_mut().set_scrollback(3);
    let _ = take_dirty(&mut parser);

    // Modify drawing row 0 (visible at position 3 due to offset)
    parser.process(b"\x1b[1;1H");
    parser.process(b"CHANGED!");

    let flags = take_dirty(&mut parser);

    // dirty_flags[0] is set (drawing row index), NOT flags[3] (visible position)
    assert!(flags[0], "drawing row 0 should be dirty after write");

    // cell(0, 0) returns scrollback data because it reads visible rows
    let cell_at_0 = visible_cell_content(&parser, 0, 0);
    assert_eq!(cell_at_0, "l", "cell(0,0) reads visible_row(0) = scrollback 'line08'");

    // The modification lives at visible position 3 (offset + drawing row 0)
    let cell_at_3 = visible_cell_content(&parser, 3, 0);
    assert_eq!(cell_at_3, "C", "cell(3,0) reads visible_row(3) = drawing row 0 = 'CHANGED!'");

    // Full repaint (reading ALL visible rows) captures the change correctly
    let all_rows = visible_row_contents(&parser, 20);
    assert_eq!(all_rows[0].trim(), "line08", "scrollback row intact");
    assert_eq!(all_rows[3].trim(), "CHANGED!", "modification visible at correct position");
}

#[test]
fn full_repaint_captures_drawing_row_change_at_correct_visible_position() {
    // Bug #445: Drawing row 0 maps to visible position offset+0 = 3 when
    // scrollback_offset = 3. A full repaint (reading all visible rows via
    // cell()) captures the change at the correct position.
    let mut parser = parser_with_scrollback(5, 20, 50);

    for i in 1..=15 {
        if i > 1 {
            parser.process(b"\r\n");
        }
        parser.process(format!("line{:02}", i).as_bytes());
    }

    parser.screen_mut().set_scrollback(3);
    let _ = take_dirty(&mut parser);

    // Modify drawing row 0
    parser.process(b"\x1b[1;1H");
    parser.process(b"CHANGED!");

    // The change appears at visible position 3 (offset=3 + drawing_row=0)
    let visible_at_3 = visible_cell_content(&parser, 3, 0);
    assert_eq!(
        visible_at_3, "C",
        "Drawing row 0 should be visible at position 3 (offset=3 + row=0)"
    );

    // Full repaint: read ALL visible rows — this is what the daemon does
    // when scrollback > 0. Every row is read via cell() which returns the
    // correct scrollback-blended content.
    let all_rows = visible_row_contents(&parser, 20);
    assert_eq!(all_rows[0].trim(), "line08", "scrollback row 0 correct");
    assert_eq!(all_rows[1].trim(), "line09", "scrollback row 1 correct");
    assert_eq!(all_rows[2].trim(), "line10", "scrollback row 2 correct");
    assert_eq!(all_rows[3].trim(), "CHANGED!", "drawing row 0 change at visible pos 3");
    assert_eq!(all_rows[4].trim(), "line12", "drawing row 1 unchanged at visible pos 4");
}

// ── Test: simulating the exact diff extraction logic ────────────────────

#[test]
fn full_repaint_diff_extraction_returns_correct_data_when_scrolled() {
    // Bug #445: Simulate the FIXED extract_diff() behavior from
    // daemon/session.rs. When scrollback_offset > 0, extract_diff forces
    // full_repaint = true, reading ALL visible rows via cell(). This avoids
    // the dirty-flag-to-visible-row index mismatch entirely.
    let mut parser = parser_with_scrollback(5, 20, 50);

    for i in 1..=15 {
        if i > 1 {
            parser.process(b"\r\n");
        }
        parser.process(format!("line{:02}", i).as_bytes());
    }

    parser.screen_mut().set_scrollback(3);
    let _ = take_dirty(&mut parser);

    // Modify drawing row 0 (visible at position 3)
    parser.process(b"\x1b[1;1H");
    parser.process(b"MODIFIED_ROW");

    // Take dirty flags
    let flags = take_dirty(&mut parser);
    let dirty_count = count_dirty(&flags);
    let total_rows = flags.len();
    let scrollback_offset = parser.screen().scrollback();

    // The daemon's fix: force full repaint when scrollback > 0
    let full_repaint = dirty_count * 2 >= total_rows || scrollback_offset > 0;
    assert!(full_repaint, "full_repaint should be forced when scrollback_offset > 0");

    // Simulate full repaint: read ALL visible rows via cell()
    let mut diff_rows: Vec<(usize, String)> = Vec::new();
    let num_rows = total_rows;
    for row_idx in 0..num_rows {
        let mut row_content = String::new();
        for col in 0..20u16 {
            let content = visible_cell_content(&parser, row_idx as u16, col);
            row_content.push_str(&content);
        }
        diff_rows.push((row_idx, row_content.trim_end().to_string()));
    }

    // Full repaint captures ALL rows correctly
    assert_eq!(diff_rows.len(), 5, "full repaint should include all 5 rows");
    assert_eq!(diff_rows[0].1, "line08", "visible row 0 = scrollback");
    assert_eq!(diff_rows[1].1, "line09", "visible row 1 = scrollback");
    assert_eq!(diff_rows[2].1, "line10", "visible row 2 = scrollback");
    assert_eq!(diff_rows[3].1, "MODIFIED_ROW", "visible row 3 = modified drawing row 0");
    assert_eq!(diff_rows[4].1, "line12", "visible row 4 = drawing row 1");

    // The modification is correctly captured at visible position 3
    let has_modification = diff_rows.iter().any(|(_, content)| content.contains("MODIFIED"));
    assert!(has_modification, "full repaint must include the MODIFIED_ROW change");
}

#[test]
fn scrollback_content_unchanged_after_drawing_row_modification() {
    // Bug #445: Verify that modifying a drawing row while scrolled doesn't
    // corrupt the scrollback content visible in the viewport.
    let mut parser = parser_with_scrollback(5, 20, 50);

    for i in 1..=15 {
        if i > 1 {
            parser.process(b"\r\n");
        }
        parser.process(format!("line{:02}", i).as_bytes());
    }

    parser.screen_mut().set_scrollback(3);

    // Scrollback rows should be: line08, line09, line10 (visible rows 0-2)
    let rows_before = visible_row_contents(&parser, 20);
    assert_eq!(rows_before[0].trim(), "line08");
    assert_eq!(rows_before[1].trim(), "line09");
    assert_eq!(rows_before[2].trim(), "line10");

    // Modify drawing row 0
    parser.process(b"\x1b[1;1H");
    parser.process(b"CHANGED!");

    // Scrollback rows (visible 0-2) should be UNCHANGED
    let rows_after = visible_row_contents(&parser, 20);
    assert_eq!(
        rows_after[0].trim(),
        "line08",
        "Scrollback row at visible position 0 should be unchanged"
    );
    assert_eq!(
        rows_after[1].trim(),
        "line09",
        "Scrollback row at visible position 1 should be unchanged"
    );
    assert_eq!(
        rows_after[2].trim(),
        "line10",
        "Scrollback row at visible position 2 should be unchanged"
    );

    // Drawing row 0 should show the change at visible position 3
    assert_eq!(
        rows_after[3].trim(),
        "CHANGED!",
        "Drawing row 0 (visible position 3 with offset=3) should show the change"
    );
}
