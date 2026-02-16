/// Tests documenting grid behavior during tab-switch resize scenarios.
///
/// Bug: When switching terminal tabs, the hidden pane's container has display:none
/// (0×0 dimensions). The frontend's fit() method reads 0×0, computes grid size as
/// 1×1 (clamped by Math.max(1)), and sends Resize(1, 1) to the daemon. This causes
/// the godly-vt grid to be physically truncated to 1 row × 1 column, destroying
/// all content.
///
/// Fix: The frontend's fit() guards against hidden containers by checking
/// offsetWidth/offsetHeight before sending resize. These tests verify:
/// 1. The grid DOES lose content on 1×1 resize (confirming the guard is needed)
/// 2. The grid preserves content for reasonable resize scenarios

/// Helper: create a parser with a specific grid size.
fn parser_with_size(rows: u16, cols: u16) -> godly_vt::Parser {
    let mut parser = godly_vt::Parser::new(rows, cols, 0);
    parser.process(b""); // ensure rows are allocated
    parser
}

/// Helper: write several lines of text to the parser.
fn write_multiline_content(parser: &mut godly_vt::Parser) {
    parser.process(b"Line 1: Hello World\r\n");
    parser.process(b"Line 2: Cargo build\r\n");
    parser.process(b"Line 3: npm install\r\n");
    parser.process(b"Line 4: git status\r\n");
    parser.process(b"Line 5: test output");
}

// ── Tests confirming why the frontend guard is necessary ─────────────

#[test]
fn resize_to_1x1_destroys_content() {
    // Confirms that the grid physically truncates to 1×1, losing all content.
    // This is WHY the frontend must guard against sending resize(1,1) to the
    // daemon when a pane is hidden.
    let mut parser = parser_with_size(24, 80);
    write_multiline_content(&mut parser);

    let before = parser.screen().contents();
    assert!(before.contains("Line 1: Hello World"));

    // Simulate what happens when the daemon receives resize(1, 1)
    parser.screen_mut().set_size(1, 1);

    let after = parser.screen().contents();
    // Only the first character survives (grid truncated to 1 cell)
    assert_eq!(after.trim(), "L", "Grid truncated to 1×1 keeps only first char");
}

#[test]
fn resize_round_trip_1x1_loses_all_lines() {
    // After 24×80 → 1×1 → 24×80, all lines except partial first are gone.
    // This confirms the data loss is permanent and irreversible.
    let mut parser = parser_with_size(24, 80);
    write_multiline_content(&mut parser);

    parser.screen_mut().set_size(1, 1);
    parser.screen_mut().set_size(24, 80);

    let contents = parser.screen().contents();
    let non_empty: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();

    // Only 1 line remains (with just "L" padded by spaces from resize)
    assert_eq!(
        non_empty.len(), 1,
        "Only 1 partial line survives 1×1 round-trip; got: {:?}", non_empty
    );
    assert!(
        !contents.contains("Hello World"),
        "Full text is gone after 1×1 round-trip"
    );
}

// ── Tests confirming content survives reasonable resize ──────────────

#[test]
fn content_survives_moderate_shrink_and_expand() {
    // Resizing to a smaller-but-reasonable size preserves visible rows.
    let mut parser = parser_with_size(24, 80);
    write_multiline_content(&mut parser);

    // Shrink to 5 rows × 20 cols — first 5 rows survive
    parser.screen_mut().set_size(5, 20);
    parser.screen_mut().set_size(24, 80);

    let contents = parser.screen().contents();
    assert!(
        contents.contains("Line 1"),
        "First line should survive moderate resize. Got: {:?}", contents
    );
}

#[test]
fn same_size_resize_preserves_all_content() {
    // Resizing to the same dimensions must not lose any content.
    // This is what happens when the frontend sends resize with unchanged dimensions.
    let mut parser = parser_with_size(24, 80);
    write_multiline_content(&mut parser);

    let before = parser.screen().contents();

    // Same-size resize (e.g., tab switch where pane dimensions haven't changed)
    parser.screen_mut().set_size(24, 80);

    let after = parser.screen().contents();
    assert_eq!(before, after, "Same-size resize must not alter content");
}

#[test]
fn grid_dimensions_restored_after_round_trip() {
    let mut parser = parser_with_size(24, 80);
    write_multiline_content(&mut parser);

    parser.screen_mut().set_size(1, 1);
    assert_eq!(parser.screen().size(), (1, 1));

    parser.screen_mut().set_size(24, 80);
    assert_eq!(parser.screen().size(), (24, 80), "Dimensions restored correctly");
}
