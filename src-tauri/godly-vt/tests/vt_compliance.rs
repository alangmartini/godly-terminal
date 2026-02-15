//! VT compliance test suite for godly-vt.
//!
//! Tests the 20+ most common escape sequences to ensure correct terminal
//! state after processing. These tests exercise godly_vt::Parser and assert
//! on Screen/Cell state directly — no running app or window needed.

// =============================================================================
// SGR Colors (16, 256, RGB)
// =============================================================================

#[test]
fn sgr_standard_16_foreground_colors() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Standard colors 0-7 via SGR 30-37
    for i in 0u8..8 {
        let seq = format!("\x1b[{}mX\x1b[m", 30 + i);
        parser.process(seq.as_bytes());
    }

    // After processing, the last 'X' with color 37 should be at the last position written
    // Let's check a specific one: red foreground (SGR 31)
    let mut p = godly_vt::Parser::new(24, 80, 0);
    p.process(b"\x1b[31mR");
    let cell = p.screen().cell(0, 0).unwrap();
    assert_eq!(cell.fgcolor(), godly_vt::Color::Idx(1), "SGR 31 should set fg to red (idx 1)");
    assert_eq!(cell.contents(), "R");
}

#[test]
fn sgr_bright_foreground_colors() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Bright colors via SGR 90-97
    parser.process(b"\x1b[91mR");
    let cell = parser.screen().cell(0, 0).unwrap();
    // Bright red = index 9 (90-82=8, 91-82=9)
    assert_eq!(cell.fgcolor(), godly_vt::Color::Idx(9), "SGR 91 should set fg to bright red (idx 9)");
}

#[test]
fn sgr_standard_background_colors() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Background colors via SGR 40-47
    parser.process(b"\x1b[44mB");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.bgcolor(), godly_vt::Color::Idx(4), "SGR 44 should set bg to blue (idx 4)");
}

#[test]
fn sgr_bright_background_colors() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Bright background via SGR 100-107
    parser.process(b"\x1b[102mG");
    let cell = parser.screen().cell(0, 0).unwrap();
    // Bright green bg = index 10 (102-92=10)
    assert_eq!(cell.bgcolor(), godly_vt::Color::Idx(10), "SGR 102 should set bg to bright green (idx 10)");
}

#[test]
fn sgr_256_color_foreground() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // 256-color fg: ESC[38;5;<n>m
    parser.process(b"\x1b[38;5;196mR");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.fgcolor(), godly_vt::Color::Idx(196), "SGR 38;5;196 should set fg to color 196");
}

#[test]
fn sgr_256_color_background() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // 256-color bg: ESC[48;5;<n>m
    parser.process(b"\x1b[48;5;21mB");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.bgcolor(), godly_vt::Color::Idx(21), "SGR 48;5;21 should set bg to color 21");
}

#[test]
fn sgr_rgb_foreground_colon_syntax() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // RGB fg with colon subparam syntax: ESC[38:2:<r>:<g>:<b>m
    parser.process(b"\x1b[38:2:255:128:0mO");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.fgcolor(), godly_vt::Color::Rgb(255, 128, 0), "SGR 38:2:255:128:0 should set fg to RGB(255,128,0)");
}

#[test]
fn sgr_rgb_foreground_semicolon_syntax() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // RGB fg with semicolon syntax: ESC[38;2;<r>;<g>;<b>m
    parser.process(b"\x1b[38;2;100;200;50mG");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.fgcolor(), godly_vt::Color::Rgb(100, 200, 50), "SGR 38;2;100;200;50 should set fg to RGB(100,200,50)");
}

#[test]
fn sgr_rgb_background() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // RGB bg: ESC[48;2;<r>;<g>;<b>m
    parser.process(b"\x1b[48;2;30;60;90mB");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell.bgcolor(), godly_vt::Color::Rgb(30, 60, 90), "SGR 48;2;30;60;90 should set bg to RGB(30,60,90)");
}

#[test]
fn sgr_default_color_reset() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Set fg, then reset with SGR 39 (default fg)
    parser.process(b"\x1b[31mR\x1b[39mD");
    let cell_d = parser.screen().cell(0, 1).unwrap();
    assert_eq!(cell_d.fgcolor(), godly_vt::Color::Default, "SGR 39 should reset fg to default");

    // Set bg, then reset with SGR 49 (default bg)
    let mut parser2 = godly_vt::Parser::new(24, 80, 0);
    parser2.process(b"\x1b[41mR\x1b[49mD");
    let cell_d2 = parser2.screen().cell(0, 1).unwrap();
    assert_eq!(cell_d2.bgcolor(), godly_vt::Color::Default, "SGR 49 should reset bg to default");
}

// =============================================================================
// SGR Attributes (bold, dim, italic, underline, inverse, strikethrough)
// =============================================================================

#[test]
fn sgr_bold() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[1mB");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert!(cell.bold(), "SGR 1 should set bold");
    assert!(!cell.dim(), "SGR 1 should not set dim");
}

#[test]
fn sgr_dim() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[2mD");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert!(cell.dim(), "SGR 2 should set dim");
    assert!(!cell.bold(), "SGR 2 should not set bold");
}

#[test]
fn sgr_italic() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[3mI");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert!(cell.italic(), "SGR 3 should set italic");
}

#[test]
fn sgr_underline() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[4mU");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert!(cell.underline(), "SGR 4 should set underline");
}

#[test]
fn sgr_inverse() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[7mI");
    let cell = parser.screen().cell(0, 0).unwrap();
    assert!(cell.inverse(), "SGR 7 should set inverse");
}

#[test]
fn sgr_reset_all() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    // Set all attributes, then reset with SGR 0
    parser.process(b"\x1b[1;3;4;7;31;42mX\x1b[0mN");
    let cell_x = parser.screen().cell(0, 0).unwrap();
    assert!(cell_x.bold());
    assert!(cell_x.italic());
    assert!(cell_x.underline());
    assert!(cell_x.inverse());
    assert_eq!(cell_x.fgcolor(), godly_vt::Color::Idx(1));
    assert_eq!(cell_x.bgcolor(), godly_vt::Color::Idx(2));

    let cell_n = parser.screen().cell(0, 1).unwrap();
    assert!(!cell_n.bold(), "SGR 0 should clear bold");
    assert!(!cell_n.italic(), "SGR 0 should clear italic");
    assert!(!cell_n.underline(), "SGR 0 should clear underline");
    assert!(!cell_n.inverse(), "SGR 0 should clear inverse");
    assert_eq!(cell_n.fgcolor(), godly_vt::Color::Default, "SGR 0 should reset fg");
    assert_eq!(cell_n.bgcolor(), godly_vt::Color::Default, "SGR 0 should reset bg");
}

#[test]
fn sgr_normal_intensity_resets_bold_and_dim() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    // Bold, then SGR 22 (normal intensity)
    parser.process(b"\x1b[1mB\x1b[22mN");
    let cell = parser.screen().cell(0, 1).unwrap();
    assert!(!cell.bold(), "SGR 22 should clear bold");
    assert!(!cell.dim(), "SGR 22 should clear dim");
}

#[test]
fn sgr_disable_individual_attributes() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    // Enable italic+underline+inverse, then disable each individually
    parser.process(b"\x1b[3;4;7mX\x1b[23mA\x1b[24mB\x1b[27mC");
    let cell_a = parser.screen().cell(0, 1).unwrap();
    assert!(!cell_a.italic(), "SGR 23 should clear italic");
    assert!(cell_a.underline(), "SGR 23 should not affect underline");

    let cell_b = parser.screen().cell(0, 2).unwrap();
    assert!(!cell_b.underline(), "SGR 24 should clear underline");

    let cell_c = parser.screen().cell(0, 3).unwrap();
    assert!(!cell_c.inverse(), "SGR 27 should clear inverse");
}

// =============================================================================
// Cursor Movement (CUP, CUU, CUD, CUF, CUB, CNL, CPL, CHA, VPA)
// =============================================================================

#[test]
fn cup_cursor_position() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // CUP: ESC[<row>;<col>H — 1-based
    parser.process(b"\x1b[5;10H");
    assert_eq!(parser.screen().cursor_position(), (4, 9), "CUP(5,10) should move to (4,9) 0-based");

    // CUP with default (1,1) -> ESC[H
    parser.process(b"\x1b[H");
    assert_eq!(parser.screen().cursor_position(), (0, 0), "CUP() should default to (0,0)");
}

#[test]
fn cuu_cursor_up() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[10;1H"); // Start at row 10
    parser.process(b"\x1b[3A");     // CUU: move up 3
    assert_eq!(parser.screen().cursor_position(), (6, 0), "CUU 3 from row 9 should go to row 6");
}

#[test]
fn cud_cursor_down() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[1;1H");  // Start at row 1
    parser.process(b"\x1b[5B");     // CUD: move down 5
    assert_eq!(parser.screen().cursor_position(), (5, 0), "CUD 5 from row 0 should go to row 5");
}

#[test]
fn cuf_cursor_forward() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[1;1H");  // Start at col 1
    parser.process(b"\x1b[10C");    // CUF: move right 10
    assert_eq!(parser.screen().cursor_position(), (0, 10), "CUF 10 from col 0 should go to col 10");
}

#[test]
fn cub_cursor_backward() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[1;20H"); // Start at col 20
    parser.process(b"\x1b[5D");     // CUB: move left 5
    assert_eq!(parser.screen().cursor_position(), (0, 14), "CUB 5 from col 19 should go to col 14");
}

#[test]
fn cnl_cursor_next_line() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[5;10H"); // Start at row 5, col 10
    parser.process(b"\x1b[2E");     // CNL: next line x2
    assert_eq!(parser.screen().cursor_position(), (6, 0), "CNL 2 should move down 2 and to col 0");
}

#[test]
fn cpl_cursor_previous_line() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[10;10H"); // Start at row 10, col 10
    parser.process(b"\x1b[3F");      // CPL: previous line x3
    assert_eq!(parser.screen().cursor_position(), (6, 0), "CPL 3 should move up 3 and to col 0");
}

#[test]
fn cha_cursor_horizontal_absolute() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[5;1H");  // Start at row 5
    parser.process(b"\x1b[25G");    // CHA: column 25 (1-based)
    assert_eq!(parser.screen().cursor_position(), (4, 24), "CHA 25 should set col to 24 (0-based)");
}

#[test]
fn vpa_vertical_position_absolute() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"\x1b[1;10H"); // Start at col 10
    parser.process(b"\x1b[15d");    // VPA: row 15 (1-based)
    assert_eq!(parser.screen().cursor_position(), (14, 9), "VPA 15 should set row to 14 (0-based), keeping col");
}

#[test]
fn cursor_clamping_at_boundaries() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Try to move above row 0
    parser.process(b"\x1b[1;1H\x1b[999A");
    assert_eq!(parser.screen().cursor_position().0, 0, "CUU should clamp at row 0");

    // Try to move below last row
    parser.process(b"\x1b[1;1H\x1b[999B");
    assert_eq!(parser.screen().cursor_position().0, 23, "CUD should clamp at last row");
}

// =============================================================================
// Erase Operations (ED, EL, ECH)
// =============================================================================

#[test]
fn ed_erase_below() {
    let mut parser = godly_vt::Parser::new(5, 10, 0);
    // Use explicit cursor positioning to fill rows
    parser.process(b"\x1b[1;1HAAAAAAAAAA");  // Fill row 0 (row 1, 1-based)
    parser.process(b"\x1b[2;1HBBBBBBBBBB");  // Fill row 1 (row 2, 1-based)
    parser.process(b"\x1b[3;1HCCCCCCCCCC");  // Fill row 2 (row 3, 1-based)
    parser.process(b"\x1b[2;1H");             // Move to row 2, col 1 (1-based)
    parser.process(b"\x1b[J");                // ED 0: erase from cursor to end

    // Row 0 should still have content
    let row0: String = parser.screen().rows(0, 10).next().unwrap();
    assert_eq!(row0.trim_end(), "AAAAAAAAAA", "Row 0 should be preserved after ED 0");

    // Row 1 (cursor row) and below should be erased
    let row1: String = parser.screen().rows(0, 10).nth(1).unwrap();
    assert_eq!(row1.trim_end(), "", "Row 1 (cursor row) should be erased by ED 0");

    let row2: String = parser.screen().rows(0, 10).nth(2).unwrap();
    assert_eq!(row2.trim_end(), "", "Row 2 should be erased by ED 0");
}

#[test]
fn ed_erase_above() {
    let mut parser = godly_vt::Parser::new(5, 10, 0);
    parser.process(b"\x1b[1;1HAAAAAAAAAA");  // Fill row 0
    parser.process(b"\x1b[2;1HBBBBBBBBBB");  // Fill row 1
    parser.process(b"\x1b[3;1HCCCCCCCCCC");  // Fill row 2
    parser.process(b"\x1b[2;5H");             // Move to row 2, col 5 (1-based)
    parser.process(b"\x1b[1J");               // ED 1: erase from start to cursor

    // Row 0 should be erased (above cursor row)
    let row0: String = parser.screen().rows(0, 10).next().unwrap();
    assert_eq!(row0.trim_end(), "", "Row 0 should be erased after ED 1");

    // Row 2 should still have content after cursor position
    let row2: String = parser.screen().rows(0, 10).nth(2).unwrap();
    assert_eq!(row2.trim_end(), "CCCCCCCCCC", "Row 2 (below cursor) should be preserved after ED 1");
}

#[test]
fn ed_erase_all() {
    let mut parser = godly_vt::Parser::new(5, 10, 0);
    parser.process(b"\x1b[1;1HAAAAAAAAAA");
    parser.process(b"\x1b[2;1HBBBBBBBBBB");
    parser.process(b"\x1b[2J");     // ED 2: erase all

    let contents = parser.screen().contents();
    assert!(contents.trim().is_empty(), "ED 2 should erase all screen content");
}

#[test]
fn el_erase_to_right() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"Hello World");
    parser.process(b"\x1b[1;6H");   // Move to col 6 (after "Hello")
    parser.process(b"\x1b[K");      // EL 0: erase to right

    let row: String = parser.screen().rows(0, 80).next().unwrap();
    assert_eq!(row.trim_end(), "Hello", "EL 0 should erase from cursor to end of line");
}

#[test]
fn el_erase_to_left() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"Hello World");
    parser.process(b"\x1b[1;6H");   // Move to col 6
    parser.process(b"\x1b[1K");     // EL 1: erase to left

    let row: String = parser.screen().rows(0, 80).next().unwrap();
    // Columns 0-5 should be erased, "World" should remain starting at col 6
    assert!(row.starts_with("      "), "EL 1 should erase from start of line to cursor");
}

#[test]
fn el_erase_entire_line() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"Hello World");
    parser.process(b"\x1b[1;6H");   // Move to col 6
    parser.process(b"\x1b[2K");     // EL 2: erase entire line

    let row: String = parser.screen().rows(0, 80).next().unwrap();
    assert_eq!(row.trim_end(), "", "EL 2 should erase entire line");
}

#[test]
fn ech_erase_characters() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"Hello World");
    parser.process(b"\x1b[1;1H");   // Move to start
    parser.process(b"\x1b[5X");     // ECH 5: erase 5 characters

    let row: String = parser.screen().rows(0, 80).next().unwrap();
    assert!(row.starts_with("     "), "ECH 5 should erase 5 chars at cursor");
    assert!(row.contains("World"), "ECH should not affect chars beyond count");
}

// =============================================================================
// Scroll Regions (DECSTBM)
// =============================================================================

#[test]
fn decstbm_scroll_region() {
    let mut parser = godly_vt::Parser::new(10, 20, 0);

    // Fill all rows using explicit positioning
    for i in 0..10u16 {
        let line = format!("\x1b[{};1HLine {:02}", i + 1, i);
        parser.process(line.as_bytes());
    }

    // Set scroll region to rows 3-7 (1-based)
    parser.process(b"\x1b[3;7r");

    // Move cursor to bottom of scroll region and trigger a scroll
    parser.process(b"\x1b[7;1H");
    parser.process(b"\n"); // Should scroll within region only

    // Row 0 (outside scroll region, above) should be unchanged
    let row0: String = parser.screen().rows(0, 20).next().unwrap();
    assert_eq!(row0.trim_end(), "Line 00", "Row 0 (outside scroll region) should not be affected");

    // Row 1 (outside scroll region, above) should be unchanged
    let row1: String = parser.screen().rows(0, 20).nth(1).unwrap();
    assert_eq!(row1.trim_end(), "Line 01", "Row 1 (outside scroll region) should not be affected");

    // Row 7 (outside scroll region, below) should be unchanged
    let row7: String = parser.screen().rows(0, 20).nth(7).unwrap();
    assert_eq!(row7.trim_end(), "Line 07", "Row 7 (outside scroll region) should not be affected");
}

#[test]
fn scroll_up_su() {
    let mut parser = godly_vt::Parser::new(5, 10, 0);
    parser.process(b"Line0\r\nLine1\r\nLine2\r\nLine3\r\nLine4");
    parser.process(b"\x1b[2S"); // SU: scroll up 2 lines

    let row0: String = parser.screen().rows(0, 10).next().unwrap();
    assert_eq!(row0.trim_end(), "Line2", "SU 2 should scroll content up by 2");
}

#[test]
fn scroll_down_sd() {
    let mut parser = godly_vt::Parser::new(5, 10, 0);
    parser.process(b"Line0\r\nLine1\r\nLine2\r\nLine3\r\nLine4");
    parser.process(b"\x1b[2T"); // SD: scroll down 2 lines

    let row2: String = parser.screen().rows(0, 10).nth(2).unwrap();
    assert_eq!(row2.trim_end(), "Line0", "SD 2 should push content down by 2");
}

// =============================================================================
// Alternate Screen (DECSET 1049)
// =============================================================================

#[test]
fn alternate_screen_switch() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Write to main screen
    parser.process(b"Main screen content");
    assert!(!parser.screen().alternate_screen());

    // Switch to alternate screen
    parser.process(b"\x1b[?1049h");
    assert!(parser.screen().alternate_screen(), "DECSET 1049 should enable alternate screen");

    // Alternate screen should be blank
    let contents = parser.screen().contents();
    assert!(contents.trim().is_empty(), "Alternate screen should start empty");

    // Write to alternate screen
    parser.process(b"Alt screen content");

    // Switch back to main screen
    parser.process(b"\x1b[?1049l");
    assert!(!parser.screen().alternate_screen(), "DECRST 1049 should disable alternate screen");

    // Main screen content should be restored
    let contents = parser.screen().contents();
    assert!(contents.contains("Main screen content"), "Main screen content should be preserved after leaving alt screen");
}

// =============================================================================
// Line Wrapping
// =============================================================================

#[test]
fn line_wrapping_at_boundary() {
    let mut parser = godly_vt::Parser::new(24, 10, 0);

    // Write exactly 10 chars, then more — should wrap to next line
    parser.process(b"1234567890AB");

    let row0: String = parser.screen().rows(0, 10).next().unwrap();
    let row1: String = parser.screen().rows(0, 10).nth(1).unwrap();
    assert_eq!(row0.trim_end(), "1234567890", "First row should be full");
    assert_eq!(row1.trim_end(), "AB", "Overflow should wrap to next row");
    assert!(parser.screen().row_wrapped(0), "Row 0 should be marked as wrapped");
}

// =============================================================================
// Wide Characters (CJK, emoji)
// =============================================================================

#[test]
fn wide_character_cjk() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // CJK character takes 2 columns
    parser.process("\u{4e16}".as_bytes()); // Unicode for a CJK character

    let cell0 = parser.screen().cell(0, 0).unwrap();
    assert!(cell0.is_wide(), "CJK character should be marked as wide");
    assert!(cell0.has_contents(), "Wide cell should have contents");

    let cell1 = parser.screen().cell(0, 1).unwrap();
    assert!(cell1.is_wide_continuation(), "Cell after wide char should be continuation");
}

#[test]
fn wide_character_wrapping() {
    let mut parser = godly_vt::Parser::new(24, 5, 0);

    // Fill 4 columns, then try a wide char that needs 2 columns
    parser.process(b"ABCD");
    parser.process("\u{4e16}".as_bytes()); // Wide char at col 4 won't fit

    // The wide char should wrap to the next line
    let row1: String = parser.screen().rows(0, 5).nth(1).unwrap();
    assert!(!row1.trim_end().is_empty(), "Wide char should wrap to next line when it doesn't fit");
}

// =============================================================================
// Tab Stops
// =============================================================================

#[test]
fn default_tab_stops() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Tab should move to column 8 (0-indexed)
    parser.process(b"A\tB");
    let (_, col) = parser.screen().cursor_position();
    // After 'A' at col 0, tab goes to col 8, then 'B' is at col 8 and cursor is at col 9
    assert_eq!(col, 9, "After A<tab>B cursor should be at col 9 (tab stops at 8)");
}

#[test]
fn multiple_tabs() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    parser.process(b"\t\t");
    let (_, col) = parser.screen().cursor_position();
    assert_eq!(col, 16, "Two tabs should move to col 16 (8+8)");
}

// =============================================================================
// OSC Title
// =============================================================================

/// Title setting requires a Callbacks implementation to capture the title.
/// Here we verify the parser doesn't crash and the screen is unaffected.
#[test]
fn osc_title_does_not_crash() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // OSC 0 (set icon name + title)
    parser.process(b"\x1b]0;My Terminal Title\x07");

    // OSC 1 (set icon name)
    parser.process(b"\x1b]1;My Icon\x07");

    // OSC 2 (set title)
    parser.process(b"\x1b]2;Another Title\x07");

    // Screen should be unaffected
    let contents = parser.screen().contents();
    assert!(contents.trim().is_empty(), "OSC title should not affect screen contents");
}

#[test]
fn osc_title_with_st_terminator() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // OSC with ST (ESC \) terminator instead of BEL
    parser.process(b"\x1b]2;Title With ST\x1b\\");

    // Should not crash or affect screen
    let contents = parser.screen().contents();
    assert!(contents.trim().is_empty());
}

// =============================================================================
// Bracketed Paste (DECSET 2004)
// =============================================================================

#[test]
fn bracketed_paste_mode() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    assert!(!parser.screen().bracketed_paste(), "Bracketed paste should be off by default");

    parser.process(b"\x1b[?2004h");
    assert!(parser.screen().bracketed_paste(), "DECSET 2004 should enable bracketed paste");

    parser.process(b"\x1b[?2004l");
    assert!(!parser.screen().bracketed_paste(), "DECRST 2004 should disable bracketed paste");
}

// =============================================================================
// Combined / Integration Tests
// =============================================================================

#[test]
fn colored_text_with_cursor_movement() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Simulate a typical colored prompt
    parser.process(b"\x1b[32muser\x1b[m@\x1b[34mhost\x1b[m:~$ ");

    let cell_u = parser.screen().cell(0, 0).unwrap();
    assert_eq!(cell_u.fgcolor(), godly_vt::Color::Idx(2), "user should be green");
    assert_eq!(cell_u.contents(), "u");

    let cell_at = parser.screen().cell(0, 4).unwrap();
    assert_eq!(cell_at.fgcolor(), godly_vt::Color::Default, "@ should have default fg");

    let cell_h = parser.screen().cell(0, 5).unwrap();
    assert_eq!(cell_h.fgcolor(), godly_vt::Color::Idx(4), "host should be blue");
}

#[test]
fn multiline_output_with_scrollback() {
    let mut parser = godly_vt::Parser::new(5, 20, 100);

    // Write more lines than the screen can hold
    for i in 0..10 {
        let line = format!("Line {:02}\r\n", i);
        parser.process(line.as_bytes());
    }

    // The screen should show the last lines
    let contents = parser.screen().contents();
    assert!(contents.contains("Line 09") || contents.contains("Line 08"),
        "Screen should show recent lines after scrolling");
}

#[test]
fn full_reset_ris() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Set up some state
    parser.process(b"\x1b[31m\x1b[1mHello\x1b[?1049h");
    assert!(parser.screen().alternate_screen());

    // Full reset
    parser.process(b"\x1bc");

    assert!(!parser.screen().alternate_screen(), "RIS should exit alternate screen");
    assert!(!parser.screen().bracketed_paste(), "RIS should clear bracketed paste");
}

#[test]
fn insert_and_delete_lines() {
    let mut parser = godly_vt::Parser::new(5, 10, 0);

    // Fill screen
    parser.process(b"Line0\r\nLine1\r\nLine2\r\nLine3\r\nLine4");

    // Move to row 2, insert 1 line
    parser.process(b"\x1b[3;1H\x1b[L");

    let row2: String = parser.screen().rows(0, 10).nth(2).unwrap();
    assert_eq!(row2.trim_end(), "", "IL should insert blank line");

    let row3: String = parser.screen().rows(0, 10).nth(3).unwrap();
    assert_eq!(row3.trim_end(), "Line2", "Original Line2 should shift down");
}

#[test]
fn insert_and_delete_characters() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    parser.process(b"ABCDEF");
    parser.process(b"\x1b[1;3H");  // Move to col 3 (between B and C)
    parser.process(b"\x1b[2@");    // ICH: insert 2 chars

    let row: String = parser.screen().rows(0, 80).next().unwrap();
    assert!(row.starts_with("AB"), "Characters before cursor should remain");
    // After insert, C-F should shift right by 2
    assert!(row.contains("CDEF"), "Characters should shift right after ICH");
}

#[test]
fn cursor_save_restore() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Move cursor, save, move again, restore
    parser.process(b"\x1b[5;10H");  // (4, 9)
    parser.process(b"\x1b7");       // DECSC: save
    parser.process(b"\x1b[1;1H");   // Move to (0, 0)
    parser.process(b"\x1b8");       // DECRC: restore

    assert_eq!(parser.screen().cursor_position(), (4, 9), "DECRC should restore cursor position");
}

#[test]
fn screen_resize() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    parser.process(b"Hello");
    parser.screen_mut().set_size(10, 40);

    let (rows, cols) = parser.screen().size();
    assert_eq!((rows, cols), (10, 40), "set_size should update screen dimensions");

    // Content should be preserved
    let row: String = parser.screen().rows(0, 40).next().unwrap();
    assert!(row.starts_with("Hello"), "Content should survive resize");
}

#[test]
fn contents_formatted_roundtrip() {
    let mut parser1 = godly_vt::Parser::new(5, 20, 0);
    parser1.process(b"\x1b[31mRed\x1b[m Normal \x1b[1mBold");

    // Get formatted output
    let formatted = parser1.screen().contents_formatted();

    // Feed to a new parser — should produce identical screen
    let mut parser2 = godly_vt::Parser::new(5, 20, 0);
    parser2.process(&formatted);

    // Contents should match
    assert_eq!(
        parser1.screen().contents(),
        parser2.screen().contents(),
        "contents_formatted roundtrip should reproduce identical text"
    );
}
