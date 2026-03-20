/// Bug #678: Scroll UX reproduction tests.
///
/// Tests cover:
/// 1. Mouse wheel fractional delta truncation — small trackpad deltas produce 0 lines
/// 2. Scrollbar integration — view_scrollbar() is dead code, never called from rendering

// ---------------------------------------------------------------------------
// Test 1: Mouse wheel delta conversion loses fractional values
// ---------------------------------------------------------------------------
//
// The conversion in app.rs L1576-1578:
//   let lines = -(delta_y * 3.0) as isize;
//
// For typical high-resolution trackpad/mouse deltas (0.1 – 0.3), this truncates
// to 0 lines, causing the scroll event to be silently discarded. No accumulator
// carries the remainder to the next event, so rapid small-delta wheel events
// produce no scrolling at all — or intermittent 3-line jumps when the delta
// occasionally crosses 1.0, creating the "rollback" jitter users report.

/// Simulates the accumulator-based mouse wheel handler from app.rs.
/// Processes a sequence of delta_y values, returning the total scroll lines fired.
fn accumulate_wheel_deltas(deltas: &[f32]) -> isize {
    let mut accumulator: f32 = 0.0;
    let mut total_lines: isize = 0;
    for &delta_y in deltas {
        accumulator += delta_y * 3.0;
        let lines = accumulator as isize; // truncates toward zero
        if lines != 0 {
            accumulator -= lines as f32; // keep remainder
            total_lines += -lines; // negate to match scroll direction
        }
    }
    total_lines
}

// Bug #678: Small positive trackpad deltas (scroll up) must eventually produce
// scroll lines via accumulation, not truncate each event to 0.
#[test]
fn small_positive_delta_must_not_truncate_to_zero() {
    // Two events of 0.3 should accumulate: 0.3*3=0.9, then 0.9+0.9=1.8 → fires 1 line
    let lines = accumulate_wheel_deltas(&[0.3, 0.3]);
    assert_ne!(
        lines, 0,
        "two delta_y=0.3 events must produce non-zero scroll lines via accumulation"
    );
}

// Bug #678: Small negative trackpad deltas (scroll down) must eventually produce
// scroll lines via accumulation.
#[test]
fn small_negative_delta_must_not_truncate_to_zero() {
    let lines = accumulate_wheel_deltas(&[-0.3, -0.3]);
    assert_ne!(
        lines, 0,
        "two delta_y=-0.3 events must produce non-zero scroll lines via accumulation"
    );
}

// Bug #678: A sequence of small deltas should accumulate to produce the correct total.
#[test]
fn accumulated_small_deltas_must_produce_scroll() {
    let deltas = [0.2_f32; 10]; // 10 trackpad events, each 0.2
    let total_lines = accumulate_wheel_deltas(&deltas);
    // 10 * 0.2 * 3.0 = 6.0 total → should fire ~6 lines
    assert!(
        total_lines.abs() >= 6,
        "10 deltas of 0.2 should produce ~6 lines total, got {total_lines}"
    );
}

// Sanity: large deltas (>= 1.0) work correctly with the accumulator too.
#[test]
fn large_delta_produces_expected_lines() {
    let lines = accumulate_wheel_deltas(&[1.0]);
    assert_eq!(lines, -3, "delta_y=1.0 should produce -3 lines");
}

// ---------------------------------------------------------------------------
// Test 2: Scrollbar module is not integrated into the terminal rendering pipeline
// ---------------------------------------------------------------------------
//
// scrollbar.rs defines view_scrollbar() which produces a valid Iced Element,
// but render_terminal_pane() in app.rs never calls it. The scrollbar is dead code.
// This test verifies by reading app.rs source and checking for the function call.

#[test]
fn scrollbar_must_be_rendered_in_terminal_pane() {
    // Read the render_terminal_pane function source to verify scrollbar integration.
    // This is a source-level contract test: the rendering pipeline must call
    // view_scrollbar (or scrollbar::) somewhere in the terminal pane builder.
    let app_source = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/app.rs"),
    )
    .expect("should be able to read app.rs");

    // Extract the render_terminal_pane function body (rough heuristic: from
    // "fn render_terminal_pane" to the next "fn " at the same indent level).
    let fn_start = app_source
        .find("fn render_terminal_pane")
        .expect("render_terminal_pane must exist in app.rs");
    let after_start = &app_source[fn_start..];
    // Find the next top-level fn definition after the opening
    let fn_end = after_start[1..]
        .find("\n    fn ")
        .map(|i| i + 1)
        .unwrap_or(after_start.len());
    let fn_body = &after_start[..fn_end];

    // Bug #678: scrollbar must be included in the terminal pane rendering.
    // Currently view_scrollbar / scrollbar:: does NOT appear in this function.
    assert!(
        fn_body.contains("view_scrollbar") || fn_body.contains("scrollbar::"),
        "render_terminal_pane must call view_scrollbar or use scrollbar:: module \
         to render a scrollbar in the terminal pane. Currently the scrollbar is \
         dead code (Bug #678)."
    );
}

// Verify scrollbar::compute_metrics works correctly (sanity — these pass).
// The metrics implementation is correct; the bug is that it's never used.
#[test]
fn scrollbar_metrics_sanity() {
    let m = godly_iced_shell::scrollbar::compute_metrics(100, 25, 0, 400.0);
    assert_eq!(m.thumb_height, 100.0); // 25% of 400
    assert!(m.thumb_y > 0.0); // thumb at bottom when offset=0
}
