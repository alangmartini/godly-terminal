# Migration Phases

## Phase 0: Fork & Setup

**Goal:** godly-vt crate exists in workspace, compiles, all existing tests pass.

### Tasks

- [ ] **0.1** Clone vt100-rust source files into `src-tauri/godly-vt/src/`
- [ ] **0.2** Copy vte 0.15.0 source into `src-tauri/godly-vt/src/state_machine/`
- [ ] **0.3** Remove vte as external dependency; wire internal module references
- [ ] **0.4** Create `Cargo.toml` with workspace member registration
- [ ] **0.5** Rename crate references: `vt100` → `godly_vt`, update all `use` paths
- [ ] **0.6** Add MIT license file with attribution to both doy/vt100-rust and alacritty/vte
- [ ] **0.7** Verify `cargo check -p godly-vt` passes
- [ ] **0.8** Port vt100's test suite (copy tests/, update imports)
- [ ] **0.9** Verify `cargo test -p godly-vt` passes
- [ ] **0.10** Verify existing workspace tests still pass (`cargo test -p godly-protocol`, `-p godly-daemon`, `-p godly-terminal`)

### Definition of Done
- `godly-vt` compiles as workspace member
- All ported tests pass
- All existing workspace tests pass
- No external dependency on `vte` or `vt100` crates

### Estimated Effort: 1 day

---

## Phase 1: Testable Terminal State

**Goal:** godly-vt is wired into the daemon as the terminal state backend. Grid can be tested without a running app. MCP reads from the grid.

### Tasks

- [ ] **1.1** Add `godly_vt::Parser` to daemon's `Session` struct
- [ ] **1.2** Feed PTY output to both ring buffer and godly-vt parser in the reader thread
- [ ] **1.3** Add `Request::ReadGrid` / `Response::Grid` to protocol for grid queries
- [ ] **1.4** Implement grid query handler in daemon server
- [ ] **1.5** Update MCP `read_terminal` to read from godly-vt grid instead of raw output_history
- [ ] **1.6** Write VT compliance test suite (SGR, cursor movement, erase, scroll, alternate screen)
- [ ] **1.7** Write daemon→grid integration tests with mock pipes
- [ ] **1.8** Write MCP `read_terminal` automated tests against grid
- [ ] **1.9** Verify all existing tests still pass
- [ ] **1.10** Update scrollback save to optionally use grid serialization

### Tests to Write
```
tests/vt_compliance.rs:
  - test_sgr_colors (16, 256, RGB foreground/background)
  - test_sgr_attributes (bold, dim, italic, underline, inverse)
  - test_cursor_movement (CUP, CUU, CUD, CUF, CUB, CNL, CPL, CHA, VPA)
  - test_erase_operations (ED 0/1/2, EL 0/1/2, ECH)
  - test_scroll_regions (DECSTBM + scroll up/down)
  - test_alternate_screen (DECSET 1049)
  - test_line_wrapping (autowrap at column boundary)
  - test_wide_characters (CJK, emoji)
  - test_tab_stops (default 8-column tabs)
  - test_osc_title (OSC 0, 1, 2)
  - test_bracketed_paste (DECSET 2004)

tests/grid_tests.rs:
  - test_scrollback_captures_rows
  - test_scrollback_eviction_at_capacity
  - test_resize_preserves_content
  - test_clear_screen

tests/bridge_integration.rs:
  - test_daemon_output_updates_grid
  - test_grid_content_after_command_output
  - test_mcp_read_terminal_from_grid
```

### Definition of Done
- Can assert on terminal grid state in `cargo test` — no app or window needed
- MCP `read_terminal` returns clean text from grid (0 ANSI stripping)
- Daemon↔grid integration tested with mock pipes
- VT compliance suite covers the 20 most common escape sequences

### Estimated Effort: 3-4 days

---

## Phase 2: SIMD Performance

**Goal:** godly-vt parses bulk output significantly faster than vanilla vte.

### Tasks

- [ ] **2.1** Add `print_str(&str)` and `put_slice(&[u8])` to Perform trait
- [ ] **2.2** Modify `ground_dispatch()` to batch printable runs via `print_str()`
- [ ] **2.3** Implement `scan_for_control()` SSE2 version
- [ ] **2.4** Implement `scan_for_control()` AVX2 version with runtime dispatch
- [ ] **2.5** Implement `is_all_ascii()` SIMD check
- [ ] **2.6** Add ASCII fast path: skip UTF-8 decode when `is_all_ascii()` is true
- [ ] **2.7** Integrate `simdutf8` for non-ASCII validation
- [ ] **2.8** Update Screen's text handler to accept string slices efficiently
- [ ] **2.9** Implement CSI fast-path for common sequences (SGR, CUP, ED, EL)
- [ ] **2.10** Create benchmark suite (criterion): ASCII, Unicode, CSI-heavy, mixed
- [ ] **2.11** Benchmark against baseline (pre-SIMD) and document results
- [ ] **2.12** Add SIMD scanner correctness tests

### Benchmark Scenarios
```
benches/throughput.rs:
  - ascii_1mb: 1MB of pure ASCII text
  - unicode_1mb: 1MB of mixed UTF-8 (CJK, emoji, Latin)
  - csi_heavy: 10K SGR color changes interspersed with text
  - mixed_realistic: simulated `cargo build` output (colors + text + cursor movement)
  - large_cat: 100MB file dump simulation
```

### Target Numbers
- ASCII: > 100 MB/s (baseline vte ~40 MB/s)
- CSI-heavy: > 50 MB/s
- At least 3x improvement over baseline in all benchmarks

### Definition of Done
- SIMD scanner passes all correctness tests
- Benchmark suite shows measurable improvement with numbers documented
- All existing tests still pass

### Estimated Effort: 1-2 weeks

---

## Phase 3: Image Protocol Support

**Goal:** godly-vt handles inline images from the three major protocols.

### Tasks

#### 3.1 Infrastructure
- [ ] **3.1.1** Add `CellContent::ImageFragment` to cell enum
- [ ] **3.1.2** Implement `ImageStore` with quota, LRU eviction, content-hash dedup
- [ ] **3.1.3** Implement `assign_image_to_cells()` — map pixel regions to cell grid
- [ ] **3.1.4** Handle cell clearing/scrolling for image fragments

#### 3.2 APC Parser Extension (required for Kitty)
- [ ] **3.2.1** Add `ApcString` state to state machine
- [ ] **3.2.2** Route `ESC _` (0x5F) to ApcString instead of SosPmApcString
- [ ] **3.2.3** Implement `advance_apc_string()` handler
- [ ] **3.2.4** Add `apc_start()` / `apc_put()` / `apc_put_slice()` / `apc_end()` to Perform
- [ ] **3.2.5** Test APC parsing with Kitty graphics sequences

#### 3.3 Kitty Graphics Protocol
- [ ] **3.3.1** Parse key=value control data from APC payload
- [ ] **3.3.2** Implement `a=t` (transmit): base64 decode, optional zlib decompress
- [ ] **3.3.3** Implement `a=T` (transmit+display): decode and place in grid
- [ ] **3.3.4** Implement `a=p` (place): reference existing upload by ID
- [ ] **3.3.5** Implement `a=d` (delete): remove placements and/or image data
- [ ] **3.3.6** Handle chunked transfer (`m=0`/`m=1`)
- [ ] **3.3.7** Support `f=100` (PNG format, terminal decodes)
- [ ] **3.3.8** Support `f=32` (raw RGBA)
- [ ] **3.3.9** Test: upload, display, delete lifecycle
- [ ] **3.3.10** Test: image survives scrollback

#### 3.4 iTerm2 Inline Images
- [ ] **3.4.1** Parse `OSC 1337 ; File=` parameters in `osc_dispatch()`
- [ ] **3.4.2** Base64 decode payload
- [ ] **3.4.3** Decode image format (via `image` crate)
- [ ] **3.4.4** Place decoded image in grid cells
- [ ] **3.4.5** Test: inline image display

#### 3.5 Sixel
- [ ] **3.5.1** Accumulate DCS sixel data via `hook()`/`put()`/`unhook()`
- [ ] **3.5.2** Decode via `icy_sixel` crate
- [ ] **3.5.3** Place decoded image in grid cells
- [ ] **3.5.4** Test: sixel image display
- [ ] **3.5.5** Test: multi-color overprinting

### Definition of Done
- All three protocols produce correct image cell references in the grid
- Images survive scrollback and clearing
- ImageStore enforces quota with LRU eviction
- Content deduplication prevents re-storing identical images
- Unit tests verify image placement for all protocols

### Estimated Effort: 2-3 weeks

---

## Phase 4: Frontend Renderer Migration

**Goal:** Replace xterm.js with a renderer that reads from godly-vt grid.

### Tasks

- [ ] **4.1** Design grid snapshot IPC format (binary or JSON)
- [ ] **4.2** Add Tauri commands: `get_grid_snapshot`, `get_grid_diff`
- [ ] **4.3** Build Canvas2D renderer that paints from grid snapshots (simplest approach)
- [ ] **4.4** OR: Embed wgpu surface in Tauri window for native GPU rendering
- [ ] **4.5** Implement text selection from grid cells
- [ ] **4.6** Implement URL detection (replace web-links addon)
- [ ] **4.7** Implement image rendering in the new renderer
- [ ] **4.8** Handle cursor display (blinking, shape)
- [ ] **4.9** Handle font rendering (glyph caching, ligatures)
- [ ] **4.10** Verify rendering matches xterm.js for common workflows
- [ ] **4.11** Performance test: render latency, FPS with heavy output

### Decision Point
Before starting Phase 4, decide between:
- **Canvas2D**: faster to build, uses existing webview, less performant
- **wgpu native**: harder to build, bypasses webview, maximum performance
- **Hybrid**: xterm.js for text, native overlay for images

### Definition of Done
- Terminal output renders correctly without xterm.js
- Text selection and copy work
- Images display inline
- Input latency < 16ms (60fps)

### Estimated Effort: 3-6 weeks (largest phase)

---

## Phase 5: Remove xterm.js Dependency

**Goal:** Clean removal of all xterm.js code and npm dependencies.

### Tasks

- [ ] **5.1** Remove xterm.js imports from TerminalPane.ts
- [ ] **5.2** Remove `@xterm/*` packages from package.json
- [ ] **5.3** Remove xterm.css import from main.ts
- [ ] **5.4** Update scrollback save/load to use godly-vt native format
- [ ] **5.5** Migrate any remaining addon functionality (fit, search)
- [ ] **5.6** Full regression test
- [ ] **5.7** Update CLAUDE.md architecture docs

### Definition of Done
- Zero xterm.js code in the project
- All functionality preserved
- Docs updated

### Estimated Effort: 1-2 days

---

## Summary Timeline

| Phase | What | Effort | Cumulative |
|-------|------|--------|------------|
| 0 | Fork & Setup | 1 day | 1 day |
| 1 | Testable Terminal State | 3-4 days | ~1 week |
| 2 | SIMD Performance | 1-2 weeks | ~3 weeks |
| 3 | Image Protocols | 2-3 weeks | ~6 weeks |
| 4 | Frontend Renderer | 3-6 weeks | ~10 weeks |
| 5 | Remove xterm.js | 1-2 days | ~10 weeks |

**Phases 0-1 unlock testability** — the primary objective.
**Phase 2 unlocks performance** — measurable competitive advantage.
**Phase 3 unlocks differentiation** — features no Tauri terminal has.
**Phases 4-5 complete the migration** — full ownership of the rendering stack.

Each phase is independently valuable. You can stop after Phase 1 and still have solved all 3 testability gaps.
