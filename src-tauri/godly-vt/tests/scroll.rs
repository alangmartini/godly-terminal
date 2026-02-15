use std::ops::RangeInclusive;

mod helpers;

#[test]
fn scroll_regions() {
    helpers::fixture("decstbm");
}

#[test]
fn origin_mode() {
    helpers::fixture("origin_mode");
}

#[test]
fn scrollback() {
    let mut parser = godly_vt::Parser::new(24, 80, 10);

    parser.process(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9\r\n10\r\n11\r\n12\r\n13\r\n14\r\n15\r\n16\r\n17\r\n18\r\n19\r\n20\r\n21\r\n22\r\n23\r\n24");
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.process(b"\r\n25\r\n26\r\n27\r\n28\r\n29\r\n30");
    assert_eq!(
        parser.screen().contents(),
        "7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30"
    );

    parser.screen_mut().set_scrollback(0);
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(
        parser.screen().contents(),
        "7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30"
    );

    parser.screen_mut().set_scrollback(1);
    assert_eq!(parser.screen().scrollback(), 1);
    assert_eq!(
        parser.screen().contents(),
        "6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29"
    );

    parser.screen_mut().set_scrollback(3);
    assert_eq!(parser.screen().scrollback(), 3);
    assert_eq!(
        parser.screen().contents(),
        "4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27"
    );

    parser.screen_mut().set_scrollback(6);
    assert_eq!(parser.screen().scrollback(), 6);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.screen_mut().set_scrollback(7);
    assert_eq!(parser.screen().scrollback(), 6);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.screen_mut().set_scrollback(0);
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(
        parser.screen().contents(),
        "7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30"
    );

    parser.screen_mut().set_scrollback(7);
    assert_eq!(parser.screen().scrollback(), 6);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.process(b"\r\n31");
    assert_eq!(parser.screen().scrollback(), 7);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.process(b"\r\n32");
    assert_eq!(parser.screen().scrollback(), 8);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.process(b"\r\n33");
    assert_eq!(parser.screen().scrollback(), 9);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.process(b"\r\n34");
    assert_eq!(parser.screen().scrollback(), 10);
    assert_eq!(
        parser.screen().contents(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24"
    );

    parser.process(b"\r\n35");
    assert_eq!(parser.screen().scrollback(), 10);
    assert_eq!(
        parser.screen().contents(),
        "2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25"
    );

    parser.process(b"\r\n36");
    assert_eq!(parser.screen().scrollback(), 10);
    assert_eq!(
        parser.screen().contents(),
        "3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26"
    );

    parser.screen_mut().set_scrollback(12);
    assert_eq!(parser.screen().scrollback(), 10);
    assert_eq!(
        parser.screen().contents(),
        "3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26"
    );

    parser.screen_mut().set_scrollback(0);
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(
        parser.screen().contents(),
        "13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36"
    );

    parser.process(b"\r\n37\r\n38");
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(
        parser.screen().contents(),
        "15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38"
    );

    parser.screen_mut().set_scrollback(5);
    assert_eq!(parser.screen().scrollback(), 5);
    assert_eq!(
        parser.screen().contents(),
        "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33"
    );

    parser.process(b"\r\n39\r\n40");
    assert_eq!(parser.screen().scrollback(), 7);
    assert_eq!(
        parser.screen().contents(),
        "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33"
    );
}

#[test]
fn edge_of_screen() {
    let mut parser = godly_vt::Parser::default();
    let screen = parser.screen().clone();

    parser.process(b"\x1b[31m\x1b[24;75Hfooba\x08r\x08\x1b[1@a");
    assert_eq!(parser.screen().cursor_position(), (23, 79));
    assert_eq!(
        parser.screen().contents(),
        "\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n                                                                          foobar"
    );
    assert_eq!(
        parser.screen().contents_formatted(),
        &b"\x1b[?25h\x1b[m\x1b[H\x1b[J\x1b[24;75H\x1b[31mfoobar\x1b[24;80H"[..]
    );
    assert_eq!(
        parser.screen().contents_diff(&screen),
        b"\x1b[24;75H\x1b[31mfoobar\x1b[24;80H"
    );
}

#[test]
fn scrollback_larger_than_rows() {
    let mut parser = godly_vt::Parser::new(3, 20, 10);

    parser.process(gen_nums(1..=10, "\r\n").as_bytes());

    // 1. Extra rows returned
    parser.screen_mut().set_scrollback(4);
    assert_eq!(parser.screen().contents(), gen_nums(4..=6, "\n"));

    // 2. Subtraction overflow
    parser.screen_mut().set_scrollback(10);
    assert_eq!(parser.screen().contents(), gen_nums(1..=3, "\n"));
}

#[test]
fn scrollback_count_tracks_stored_rows() {
    let mut parser = godly_vt::Parser::new(5, 20, 100);

    // No scrollback initially
    assert_eq!(parser.screen().scrollback_count(), 0);

    // Fill the 5-row viewport (no scrollback yet)
    parser.process(b"1\r\n2\r\n3\r\n4\r\n5");
    assert_eq!(parser.screen().scrollback_count(), 0);

    // Push one line off screen into scrollback
    parser.process(b"\r\n6");
    assert_eq!(parser.screen().scrollback_count(), 1);

    // Push more
    parser.process(b"\r\n7\r\n8\r\n9\r\n10");
    assert_eq!(parser.screen().scrollback_count(), 5);
}

#[test]
fn scrollback_count_capped_at_capacity() {
    let mut parser = godly_vt::Parser::new(3, 20, 5);

    // Push 10 lines through a 3-row terminal with scrollback capacity 5
    parser.process(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9\r\n10");
    // 7 lines scrolled off, but only 5 kept (capacity)
    assert_eq!(parser.screen().scrollback_count(), 5);

    // Push more — count stays at 5
    parser.process(b"\r\n11\r\n12");
    assert_eq!(parser.screen().scrollback_count(), 5);
}

#[test]
fn cell_access_respects_scrollback_offset() {
    let mut parser = godly_vt::Parser::new(3, 20, 10);

    // Write lines: "aaa", "bbb", "ccc", "ddd", "eee"
    parser.process(b"aaa\r\nbbb\r\nccc\r\nddd\r\neee");
    // Viewport shows: ccc, ddd, eee (lines 3-5)
    // Scrollback has: aaa, bbb

    // At offset 0 (live): row 0 starts with 'c'
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "c");

    // Scroll up 1: row 0 starts with 'b'
    parser.screen_mut().set_scrollback(1);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "b");

    // Scroll up 2: row 0 starts with 'a'
    parser.screen_mut().set_scrollback(2);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "a");

    // Back to live
    parser.screen_mut().set_scrollback(0);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "c");
}

#[test]
fn scrollback_auto_adjusts_on_new_output() {
    // Bug regression: when scrolled up, new output should keep viewport stable
    let mut parser = godly_vt::Parser::new(3, 20, 10);

    parser.process(b"aaa\r\nbbb\r\nccc\r\nddd\r\neee");
    // Scrollback: aaa, bbb. Viewport: ccc, ddd, eee

    parser.screen_mut().set_scrollback(2);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "a");
    assert_eq!(parser.screen().scrollback(), 2);

    // New output arrives while scrolled up
    parser.process(b"\r\nfff");
    // Offset should auto-increment to keep viewport stable
    assert_eq!(parser.screen().scrollback(), 3);
    // Same content visible (row 0 still 'a')
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "a");
}

#[test]
fn zero_scrollback_capacity_stores_nothing() {
    let mut parser = godly_vt::Parser::new(3, 20, 0);

    parser.process(b"1\r\n2\r\n3\r\n4\r\n5");
    assert_eq!(parser.screen().scrollback_count(), 0);
    assert_eq!(parser.screen().scrollback(), 0);

    // set_scrollback clamps to 0 since nothing stored
    parser.screen_mut().set_scrollback(5);
    assert_eq!(parser.screen().scrollback(), 0);
}

#[cfg(test)]
fn gen_nums(range: RangeInclusive<u8>, join: &str) -> String {
    range
        .map(|num| num.to_string())
        .collect::<Vec<String>>()
        .join(join)
}
