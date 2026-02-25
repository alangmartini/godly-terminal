use godly_protocol::types::*;
use godly_renderer::{parse_hex_color, resolve_cell_colors, GpuRenderer, TerminalTheme};

/// Helper: create a RichGridCell with given content and defaults.
fn make_cell(content: &str) -> RichGridCell {
    RichGridCell {
        content: content.to_string(),
        fg: "default".to_string(),
        bg: "default".to_string(),
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        inverse: false,
        wide: false,
        wide_continuation: false,
    }
}

/// Helper: create a colored cell.
fn make_colored_cell(content: &str, fg: &str, bg: &str) -> RichGridCell {
    RichGridCell {
        content: content.to_string(),
        fg: fg.to_string(),
        bg: bg.to_string(),
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        inverse: false,
        wide: false,
        wide_continuation: false,
    }
}

/// Helper: create a styled cell.
fn make_styled_cell(content: &str, bold: bool, italic: bool, dim: bool) -> RichGridCell {
    RichGridCell {
        content: content.to_string(),
        fg: "#ffffff".to_string(),
        bg: "default".to_string(),
        bold,
        dim,
        italic,
        underline: false,
        inverse: false,
        wide: false,
        wide_continuation: false,
    }
}

/// Helper: pad a row of cells to the desired column count with spaces.
fn pad_row(mut cells: Vec<RichGridCell>, cols: usize) -> Vec<RichGridCell> {
    while cells.len() < cols {
        cells.push(make_cell(" "));
    }
    cells.truncate(cols);
    cells
}

/// Build a small 3-row x 10-col test grid with mixed content.
fn make_test_grid() -> RichGridData {
    let cols = 10usize;

    // Row 0: "Hello Wrld" in default colors
    let row0_text = "HelloWorld";
    let cells0: Vec<RichGridCell> = row0_text.chars().map(|ch| make_cell(&ch.to_string())).collect();

    // Row 1: colored text
    let cells1: Vec<RichGridCell> = "Red Green!".chars().enumerate().map(|(i, ch)| {
        if i < 3 {
            make_colored_cell(&ch.to_string(), "#ff0000", "default")
        } else if i < 9 {
            make_colored_cell(&ch.to_string(), "#00ff00", "#1e1e1e")
        } else {
            make_cell(&ch.to_string())
        }
    }).collect();

    // Row 2: bold + italic
    let cells2: Vec<RichGridCell> = "BoldItalic".chars().enumerate().map(|(i, ch)| {
        if i < 4 {
            make_styled_cell(&ch.to_string(), true, false, false)
        } else {
            make_styled_cell(&ch.to_string(), false, true, false)
        }
    }).collect();

    RichGridData {
        rows: vec![
            RichGridRow { cells: pad_row(cells0, cols), wrapped: false },
            RichGridRow { cells: pad_row(cells1, cols), wrapped: false },
            RichGridRow { cells: pad_row(cells2, cols), wrapped: false },
        ],
        cursor: CursorState { row: 0, col: 0 },
        dimensions: GridDimensions { rows: 3, cols: cols as u16 },
        alternate_screen: false,
        cursor_hidden: false,
        title: String::new(),
        scrollback_offset: 0,
        total_scrollback: 0,
    }
}

/// Build an empty grid (all spaces).
fn make_empty_grid() -> RichGridData {
    let cols = 5usize;
    let rows = 2usize;
    let cell_row: Vec<RichGridCell> = (0..cols).map(|_| make_cell(" ")).collect();
    RichGridData {
        rows: (0..rows)
            .map(|_| RichGridRow {
                cells: cell_row.clone(),
                wrapped: false,
            })
            .collect(),
        cursor: CursorState { row: 0, col: 0 },
        dimensions: GridDimensions {
            rows: rows as u16,
            cols: cols as u16,
        },
        alternate_screen: false,
        cursor_hidden: true,
        title: String::new(),
        scrollback_offset: 0,
        total_scrollback: 0,
    }
}

// ---- Color parsing tests ----

#[test]
fn test_color_parsing_red() {
    let c = parse_hex_color("#ff0000").unwrap();
    assert!((c[0] - 1.0).abs() < 0.01);
    assert!(c[1].abs() < 0.01);
    assert!(c[2].abs() < 0.01);
    assert!((c[3] - 1.0).abs() < 0.01);
}

#[test]
fn test_color_parsing_green() {
    let c = parse_hex_color("#00ff00").unwrap();
    assert!(c[0].abs() < 0.01);
    assert!((c[1] - 1.0).abs() < 0.01);
}

#[test]
fn test_color_parsing_blue() {
    let c = parse_hex_color("#0000ff").unwrap();
    assert!(c[0].abs() < 0.01);
    assert!(c[1].abs() < 0.01);
    assert!((c[2] - 1.0).abs() < 0.01);
}

#[test]
fn test_color_parsing_white() {
    let c = parse_hex_color("#ffffff").unwrap();
    assert!((c[0] - 1.0).abs() < 0.01);
    assert!((c[1] - 1.0).abs() < 0.01);
    assert!((c[2] - 1.0).abs() < 0.01);
}

#[test]
fn test_color_parsing_black() {
    let c = parse_hex_color("#000000").unwrap();
    assert!(c[0].abs() < 0.01);
    assert!(c[1].abs() < 0.01);
    assert!(c[2].abs() < 0.01);
}

#[test]
fn test_color_parsing_invalid() {
    assert!(parse_hex_color("default").is_none());
    assert!(parse_hex_color("").is_none());
    assert!(parse_hex_color("#fff").is_none());
    assert!(parse_hex_color("not-a-color").is_none());
}

#[test]
fn test_resolve_cell_colors_defaults() {
    let dfg = [1.0, 1.0, 1.0, 1.0];
    let dbg = [0.0, 0.0, 0.0, 1.0];
    let (fg, bg) = resolve_cell_colors("default", "default", false, dfg, dbg);
    assert_eq!(fg, dfg);
    assert_eq!(bg, dbg);
}

#[test]
fn test_resolve_cell_colors_inverse() {
    let dfg = [1.0, 1.0, 1.0, 1.0];
    let dbg = [0.0, 0.0, 0.0, 1.0];
    let (fg, bg) = resolve_cell_colors("default", "default", true, dfg, dbg);
    assert_eq!(fg, dbg);
    assert_eq!(bg, dfg);
}

// ---- GPU renderer tests ----
// These tests require a GPU adapter. They will fail in headless CI without one.

#[test]
fn test_render_basic_grid() {
    let grid = make_test_grid();
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    let (width, height, pixels) = renderer.render_to_pixels(&grid).expect("render failed");
    assert!(width > 0, "width should be positive");
    assert!(height > 0, "height should be positive");
    assert_eq!(
        pixels.len(),
        (width * height * 4) as usize,
        "pixel buffer size mismatch"
    );
    // Verify non-zero pixels (something was rendered).
    assert!(
        pixels.iter().any(|&b| b != 0),
        "pixels should not all be zero"
    );
}

#[test]
fn test_render_to_png() {
    let grid = make_test_grid();
    let mut renderer = match GpuRenderer::new("Cascadia Code", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    let png = renderer.render_to_png(&grid).expect("PNG render failed");
    assert!(png.len() > 100, "Valid PNG should be at least 100 bytes");
    // PNG magic bytes: 0x89 P N G
    assert_eq!(
        &png[..4],
        &[0x89, 0x50, 0x4E, 0x47],
        "PNG magic bytes mismatch"
    );
}

#[test]
fn test_render_empty_grid() {
    let grid = make_empty_grid();
    let mut renderer = match GpuRenderer::new("Consolas", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    let (width, height, pixels) = renderer.render_to_pixels(&grid).expect("render failed");
    assert!(width > 0);
    assert!(height > 0);
    assert_eq!(pixels.len(), (width * height * 4) as usize);
}

#[test]
fn test_render_with_custom_theme() {
    let grid = make_test_grid();
    let mut renderer = match GpuRenderer::new("Consolas", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    // Set a light theme.
    renderer.set_theme(TerminalTheme {
        foreground: [0.0, 0.0, 0.0, 1.0],
        background: [1.0, 1.0, 1.0, 1.0],
        cursor_color: [0.0, 0.0, 0.0, 1.0],
        selection_bg: [0.6, 0.8, 1.0, 0.5],
    });

    let (width, height, pixels) = renderer.render_to_pixels(&grid).expect("render failed");
    assert!(width > 0);
    assert!(height > 0);
    // With a white background, most pixels should be bright.
    let bright_pixels = pixels.chunks_exact(4).filter(|px| px[0] > 200).count();
    let total_pixels = (width * height) as usize;
    assert!(
        bright_pixels > total_pixels / 2,
        "Light theme should have mostly bright pixels"
    );
}

#[test]
fn test_render_dimensions_match_grid() {
    let grid = make_test_grid();
    let mut renderer = match GpuRenderer::new("Consolas", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    let (cell_w, cell_h) = renderer.cell_size();
    let expected_w = (grid.dimensions.cols as f32 * cell_w).ceil() as u32;
    let expected_h = (grid.dimensions.rows as f32 * cell_h).ceil() as u32;

    let (width, height, _pixels) = renderer.render_to_pixels(&grid).expect("render failed");
    assert_eq!(width, expected_w, "output width should match grid cols * cell_w");
    assert_eq!(height, expected_h, "output height should match grid rows * cell_h");
}

#[test]
fn test_render_cursor_hidden() {
    // When cursor is hidden, no cursor overlay should be drawn.
    let mut grid = make_test_grid();
    grid.cursor_hidden = true;

    let mut renderer = match GpuRenderer::new("Consolas", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    // Should succeed without crash.
    let result = renderer.render_to_pixels(&grid);
    assert!(result.is_ok());
}

#[test]
fn test_multiple_renders_reuse_renderer() {
    // Verify that the renderer can render multiple grids without issues.
    let grid = make_test_grid();
    let mut renderer = match GpuRenderer::new("Consolas", 14.0) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Skipping GPU test (no adapter): {}", e);
            return;
        }
    };

    for _ in 0..3 {
        let result = renderer.render_to_pixels(&grid);
        assert!(result.is_ok(), "repeated render should succeed");
    }
}
