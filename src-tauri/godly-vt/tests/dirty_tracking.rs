/// Tests for row-level dirty tracking used by differential grid snapshots.
///
/// Dirty tracking avoids full-grid serialization on every keystroke by
/// marking only changed rows. These tests verify that:
/// - All rows start dirty after creation
/// - take_dirty_rows() clears flags
/// - Individual character writes only dirty the affected row
/// - Bulk operations (scroll, clear, resize, insert/delete lines) mark all rows dirty
/// - Erase operations mark the correct range of rows

/// Helper: create a parser with a specific grid size.
fn parser_with_size(rows: u16, cols: u16) -> godly_vt::Parser {
    let mut parser = godly_vt::Parser::new(rows, cols, 0);
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

#[test]
fn all_rows_dirty_after_creation() {
    let parser = parser_with_size(24, 80);
    assert!(parser.screen().has_dirty_rows());
}

#[test]
fn take_dirty_rows_clears_flags() {
    let mut parser = parser_with_size(24, 80);
    let flags = take_dirty(&mut parser);
    // All 24 rows should have been dirty
    assert_eq!(flags.len(), 24);
    assert_eq!(count_dirty(&flags), 24);
    // After taking, no rows should be dirty
    assert!(!parser.screen().has_dirty_rows());
}

#[test]
fn take_dirty_rows_returns_all_clean_after_clear() {
    let mut parser = parser_with_size(10, 40);
    // Consume initial dirty flags
    let _ = take_dirty(&mut parser);
    // Now all should be clean
    let flags = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags), 0);
}

#[test]
fn single_character_dirties_only_one_row() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser); // consume initial

    // Write a single character at row 0
    parser.process(b"A");

    let flags = take_dirty(&mut parser);
    assert_eq!(flags.len(), 10);
    assert!(flags[0], "row 0 should be dirty after writing 'A'");
    // All other rows should be clean
    for (i, &flag) in flags.iter().enumerate().skip(1) {
        assert!(!flag, "row {} should be clean", i);
    }
}

#[test]
fn writing_on_specific_row_dirties_that_row() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Move cursor to row 5 and write
    parser.process(b"\x1b[6;1H"); // CSI row;col H (1-indexed)
    // cursor_position should be (5, 0)
    let _ = take_dirty(&mut parser); // consume any dirty from cursor move

    parser.process(b"Hello");

    let flags = take_dirty(&mut parser);
    assert!(flags[5], "row 5 should be dirty after writing");
    // Other rows should generally be clean (row 0 may or may not be dirty depending on cursor handling)
    let dirty_count = count_dirty(&flags);
    assert!(dirty_count >= 1 && dirty_count <= 2,
        "expected 1-2 dirty rows, got {}", dirty_count);
}

#[test]
fn scroll_up_marks_all_dirty() {
    let mut parser = parser_with_size(5, 40);
    let _ = take_dirty(&mut parser);

    // Fill screen and cause a scroll
    parser.process(b"line1\r\nline2\r\nline3\r\nline4\r\nline5\r\nline6");

    let flags = take_dirty(&mut parser);
    // Scroll should mark all rows dirty
    assert_eq!(count_dirty(&flags), 5, "all rows should be dirty after scroll");
}

#[test]
fn clear_screen_marks_all_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // ED 2 - Erase entire display
    parser.process(b"\x1b[2J");

    let flags = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags), 10, "all rows should be dirty after clear screen");
}

#[test]
fn erase_forward_marks_correct_rows() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Move to row 3, col 0
    parser.process(b"\x1b[4;1H");
    let _ = take_dirty(&mut parser);

    // ED 0 - Erase from cursor to end of display
    parser.process(b"\x1b[0J");

    let flags = take_dirty(&mut parser);
    // Rows 3-9 should be dirty (from cursor row to end)
    for i in 3..10 {
        assert!(flags[i], "row {} should be dirty after erase forward", i);
    }
    // Rows 0-2 should be clean
    for i in 0..3 {
        assert!(!flags[i], "row {} should be clean after erase forward", i);
    }
}

#[test]
fn erase_backward_marks_correct_rows() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Move to row 5, col 10
    parser.process(b"\x1b[6;11H");
    let _ = take_dirty(&mut parser);

    // ED 1 - Erase from start of display to cursor
    parser.process(b"\x1b[1J");

    let flags = take_dirty(&mut parser);
    // Rows 0-5 should be dirty (from start to cursor row)
    for i in 0..=5 {
        assert!(flags[i], "row {} should be dirty after erase backward", i);
    }
    // Rows 6-9 should be clean
    for i in 6..10 {
        assert!(!flags[i], "row {} should be clean after erase backward", i);
    }
}

#[test]
fn insert_lines_marks_all_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Move to row 3
    parser.process(b"\x1b[4;1H");
    let _ = take_dirty(&mut parser);

    // IL - Insert 2 lines
    parser.process(b"\x1b[2L");

    let flags = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags), 10, "all rows should be dirty after insert lines");
}

#[test]
fn delete_lines_marks_all_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Move to row 3
    parser.process(b"\x1b[4;1H");
    let _ = take_dirty(&mut parser);

    // DL - Delete 2 lines
    parser.process(b"\x1b[2M");

    let flags = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags), 10, "all rows should be dirty after delete lines");
}

#[test]
fn resize_marks_all_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Resize the terminal
    parser.screen_mut().set_size(15, 60);

    let flags = take_dirty(&mut parser);
    assert_eq!(flags.len(), 15, "dirty flags should match new row count");
    assert_eq!(count_dirty(&flags), 15, "all rows should be dirty after resize");
}

#[test]
fn resize_smaller_marks_all_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    parser.screen_mut().set_size(5, 40);

    let flags = take_dirty(&mut parser);
    assert_eq!(flags.len(), 5, "dirty flags should match new row count");
    assert_eq!(count_dirty(&flags), 5, "all rows should be dirty after resize");
}

#[test]
fn erase_row_dirties_current_row_only() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Move to row 4
    parser.process(b"\x1b[5;1H");
    let _ = take_dirty(&mut parser);

    // EL 2 - Erase entire line
    parser.process(b"\x1b[2K");

    let flags = take_dirty(&mut parser);
    assert!(flags[4], "row 4 should be dirty after erase line");
    // Other rows should be clean (though cursor row may pick up an extra dirty)
    let dirty_count = count_dirty(&flags);
    assert!(dirty_count <= 2, "expected at most 2 dirty rows from erase line, got {}", dirty_count);
}

#[test]
fn has_dirty_rows_reflects_state() {
    let mut parser = parser_with_size(5, 40);
    // Initially all dirty
    assert!(parser.screen().has_dirty_rows());

    // Consume
    let _ = take_dirty(&mut parser);
    assert!(!parser.screen().has_dirty_rows());

    // Write a character
    parser.process(b"X");
    assert!(parser.screen().has_dirty_rows());

    // Consume again
    let _ = take_dirty(&mut parser);
    assert!(!parser.screen().has_dirty_rows());
}

#[test]
fn multiple_writes_same_row_still_one_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Write multiple characters on the same row
    parser.process(b"Hello World! This is a test.");

    let flags = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags), 1, "only one row should be dirty for same-row writes");
    assert!(flags[0]);
}

#[test]
fn newline_dirties_at_least_two_rows() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Write text then newline then text
    parser.process(b"first\r\nsecond");

    let flags = take_dirty(&mut parser);
    assert!(flags[0], "row 0 should be dirty");
    assert!(flags[1], "row 1 should be dirty");
    // At least 2 rows must be dirty (the rows actually written to).
    // More may be dirty due to internal cursor/wrap mechanics.
    assert!(count_dirty(&flags) >= 2,
        "expected at least 2 dirty rows, got {}", count_dirty(&flags));
}

#[test]
fn alternate_screen_switch_marks_all_dirty() {
    let mut parser = parser_with_size(10, 40);
    let _ = take_dirty(&mut parser);

    // Write to primary screen
    parser.process(b"primary content");
    let _ = take_dirty(&mut parser);

    // Switch to alternate screen
    parser.process(b"\x1b[?1049h");

    let flags = take_dirty(&mut parser);
    // Alternate screen starts with all rows dirty (new grid)
    assert_eq!(count_dirty(&flags), 10, "all rows should be dirty on alternate screen entry");
}

#[test]
fn scroll_down_marks_all_dirty() {
    let mut parser = parser_with_size(5, 40);
    let _ = take_dirty(&mut parser);

    // Set scroll region and scroll down
    parser.process(b"\x1b[1;5r"); // Set scroll region to full screen
    parser.process(b"\x1b[T"); // Scroll down

    let flags = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags), 5, "all rows should be dirty after scroll down");
}

#[test]
fn dirty_flags_survive_multiple_take_cycles() {
    let mut parser = parser_with_size(5, 40);

    // Cycle 1: initial all-dirty
    let flags1 = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags1), 5);

    // Cycle 2: should be all-clean
    let flags2 = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags2), 0);

    // Cycle 3: write something, should get 1 dirty
    parser.process(b"x");
    let flags3 = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags3), 1);

    // Cycle 4: clean again
    let flags4 = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags4), 0);

    // Cycle 5: scroll marks all dirty
    parser.process(b"\r\n\r\n\r\n\r\n\r\noverflow");
    let flags5 = take_dirty(&mut parser);
    assert_eq!(count_dirty(&flags5), 5);
}
