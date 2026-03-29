# Rendering Quality Iteration Log

Target: Match Windows Terminal / Zed quality (pixel-perfect, crisp text)

## Iteration 1 — Analysis

**Current state**: Text renders but lacks the crispness of Windows Terminal.

**Root cause identified**: The GPU glyph atlas (PR #861) converts DirectWrite's ClearType
subpixel RGB data to grayscale alpha. This loses 3x horizontal effective resolution.

**Pipeline**:
1. DirectWrite rasterizes glyphs with ClearType → RGB subpixel data (3 bytes per pixel)
2. `glyph_atlas.rs:258-274` averages RGB → single grayscale alpha channel
3. Shader samples single alpha channel, mixes fg/bg colors
4. Result: grayscale antialiasing instead of subpixel RGB

**What Windows Terminal does differently**: Keeps RGB subpixel data intact and composites
per-channel in linear color space.

**Plan for Iteration 1**: Implement LCD subpixel rendering in the GPU pipeline:
- Store RGB coverage in atlas texture (use R, G, B channels for per-subpixel coverage)
- Modify shader to blend per-channel: `out.r = mix(bg.r, fg.r, atlas.r)` etc.
- Keep DirectWrite's ClearType data intact through the pipeline

**Key files**:
- `src-tauri/native/terminal-surface/src/glyph_atlas.rs` (atlas insertion - RGB→grayscale)
- `src-tauri/native/terminal-surface/src/atlas_shader.rs` (shader - single alpha blend)
- `src-tauri/native/terminal-surface/src/atlas_vertex_builder.rs` (vertex building)
- `src-tauri/native/terminal-surface/src/directwrite_rasterizer.rs` (rasterization)

## Iteration 1 — Implementation (Subpixel RGB)

**Changes made**:

1. **glyph_atlas.rs**: Replaced `to_grayscale_alpha()` with `to_rgba_coverage()`.
   - ClearType SubpixelRgb: stores R, G, B channels as per-subpixel coverage, A = max(R,G,B)
   - Grayscale Alpha: replicates alpha to all 3 channels (R=G=B=A=alpha)
   - `pack_cell()` now writes all 4 RGBA bytes per texel instead of just R

2. **atlas_shader.rs**: Changed fragment shader from single-alpha to per-channel blending:
   - Before: `let alpha = sample.r; return mix(bg, fg, alpha);`
   - After: Per-channel `mix(bg.r, fg.r, coverage.r)` etc.

3. **atlas_shader.rs**: Fixed atlas texture format — always use `Rgba8Unorm` (linear)
   instead of matching compositor sRGB. Coverage values are linear data, not colours;
   sRGB decode would distort them (making AA too heavy).

**Result**: Text appears crisper. Subpixel RGB data from DirectWrite is now preserved
through the pipeline. No visible color fringing artifacts.

**Remaining concerns**:
- Need to check if colour fringing appears on non-dark backgrounds
- Verify the text weight matches Windows Terminal (could still be slightly different)
- Check if there's a gamma/blending mismatch (subpixel blending should ideally happen in linear space)

**Visual test results (colored git log)**:
- Commit hashes (yellow): crisp and readable
- Branch names (green/red/cyan): clear, well-defined
- Graph characters (|, \, /, *): clean rendering
- Body text (white on dark): well-hinted, good weight
- No visible colour fringing artifacts
- Quality comparable to Windows Terminal for dark backgrounds

**Status**: Iteration 1 complete. Significant improvement achieved.
The three key changes (RGB atlas storage, per-channel shader blending, linear texture format)
together deliver ClearType-quality text rendering through the GPU pipeline.

## Iteration 2 — Gamma-correct blending (sRGB 2.2)

**Problem**: Text still too thin compared to Windows Terminal after subpixel fix.
**Change**: Added sRGB→linear→sRGB conversion in shader (gamma 2.2).
**Result**: Slight improvement but still noticeably thinner than WT.

## Iteration 3 — ClearType gamma 1.8

**Problem**: sRGB gamma 2.2 is not what Windows ClearType uses.
**Discovery**: Windows ClearType uses gamma 1.8 (not 2.2) via IDWriteRenderingParams.
**Change**: Switched shader to `pow(color, 1.8)` for linearization instead of sRGB curve.
**Result**: Better weight but still slightly thin.

## Iteration 4 — Enhanced contrast 0.5

**Problem**: Windows ClearType also applies "enhanced contrast" that boosts mid-range coverage.
**Discovery**: IDWriteRenderingParams has enhanced contrast (default 0.5). Formula:
  `new_coverage = clamp(c + EC * c * (1 - c), 0, 1)`
This makes antialiased edges heavier — especially important for thin strokes on dark backgrounds.
**Change**: Added `enhance()` function in shader with EC=0.5.
**Result**: Better but still slightly lighter than WT.

## Iteration 5 — Enhanced contrast 0.75 → 1.0

**Problem**: EC=0.5 not enough, font difference (Geist Mono vs Cascadia Code) adds to perception.
**Changes tested**: EC=0.75, EC=1.0
**Result at EC=1.0**: Text weight now matches Windows Terminal quality. Side-by-side comparison
shows comparable stroke weight, crispness, and antialiasing quality.

The remaining visual difference is font design (Geist Mono is a lighter-weight font than
Cascadia Code/Consolas), not rendering pipeline quality.

**Final shader parameters**:
- Gamma: 1.8 (ClearType standard)
- Enhanced contrast: 1.0 (slightly higher than DWrite default 0.5, compensates for GPU composition)
- Atlas texture format: Rgba8Unorm (linear, not sRGB)
- Per-channel subpixel blending with full ClearType RGB data

## Iteration 6 — UI Chrome & Catppuccin Mocha Theme

**Goal**: Close the visual gap between current bare UI and the polished reference
(tmux-style terminal multiplexer with sidebar, tab bar, status bar).

**Changes made**:

1. **Color palette** (`builder.rs`): Replaced generic dark palette with Catppuccin Mocha
   colors (Base #1e1e2e, Crust #11111b, Text #cdd6f4, Blue #89b4fa, etc.).
   Added ACCENT_GREEN, ACCENT_PEACH, ACCENT_MAUVE, BORDER colors.

2. **Layout** (`layout.rs`): Widened sidebar from 48px to 200px. Increased title bar
   to 38px, status bar to 28px.

3. **Sidebar** (`sidebar.rs`): Complete rewrite — session list with numbered entries,
   "Sessions" header, active indicator (blue left bar), branch info, "+ New Session"
   button. Replaced icon-based narrow sidebar.

4. **Tab bar** (`tab_bar.rs`): Added per-tab accent colors (Blue, Green, Peach, Mauve,
   Red cycling), colored top accent bar on active tab, numbered tab indicators.

5. **Title bar** (`title_bar.rs`): Blends with tab bar using BG_RAISED. Muted title text.
   Added bottom border separator.

6. **Status bar** (`status_bar.rs`): Added CWD display, git branch field, separator dots.
   Uses Catppuccin accent colors for different info types.

7. **sRGB gamma fix** (`quad_renderer.rs`): Added `pow(color, 2.2)` in quad fragment
   shader to convert sRGB palette values to linear before output to sRGB render target.
   Without this, colors appeared much too bright (double gamma encoding).

8. **Atlas shader sRGB fix** (`atlas_shader.rs`): Removed final `pow(blended, INV_GAMMA)`
   — the atlas shader now outputs in linear space, letting the sRGB render target handle
   perceptual encoding. Previously, the shader encoded to gamma 1.8, then the sRGB
   surface re-encoded (~gamma 2.2), producing washed-out terminal backgrounds.

9. **Terminal colors** (`terminal_renderer.rs`): Updated default fg/bg to Catppuccin
   Text (#cdd6f4) and Base (#1e1e2e).

10. **Tab creation** (`main.rs`): Added tab when session is created so tab bar is
    populated on startup.

**Result**: Terminal now has a cohesive Catppuccin Mocha dark theme. Sidebar shows
session list, tab bar shows colored tab indicators, status bar is present. Terminal
background is properly dark. Text rendering remains crisp.

**Remaining work for next iteration**:
- UI text in chrome (sidebar labels, tab titles, status bar) is still invisible —
  the builder's UiTextRenderer is a no-op (UV=0, no glyph atlas). Need to wire up
  the real text renderer from `ui/text_renderer.rs`.
- Terminal area has slightly different shade from sidebar due to different gamma
  paths (atlas shader gamma 1.8 vs quad shader gamma 2.2).
- No split pane rendering yet.

## Iteration 6b — DPI-aware text layout & status bar visibility

**Goal**: Fix UI text layout issues caused by hardcoded pixel dimensions that don't
account for DPI scaling, and make the status bar visible on all screen sizes.

**Problems identified**:
1. `UiTextRenderer` was a zero-state handle; widgets used hardcoded `8.0` px/char
   for text width and `14.0` px for cell height. At 1.5x DPI, actual cell dimensions
   are 12.6×27.3 px, causing text overlap, truncation, and misalignment.
2. Status bar was invisible because the window (1200×800 logical = 1800×1200 physical)
   extended below the screen (1920×1080), pushing the bottom 34px status bar off-screen.
3. Sidebar session names were over-truncated due to aggressive branch-width reserve.

**Changes made**:

1. **UiTextRenderer** (`builder.rs`): Now stores actual `cell_width` and `cell_height`
   from font metrics. Added `text_width(s)` helper for precise text positioning.

2. **Font metrics passthrough** (`main.rs`): `UiTextRenderer` created with
   `font_metrics.scaled_for_render()` dimensions instead of hardcoded 8.0/16.0.

3. **Sidebar** (`sidebar.rs`): All text positioning uses actual cell dimensions.
   Vertical centering uses `(area_h - cell_height) / 2` instead of hardcoded 14.0.
   Session name truncation dynamically calculates available chars based on cell width
   and sidebar width, with branch names truncated to max 6 chars.

4. **Tab bar** (`tab_bar.rs`): Fixed dot/number/title spacing using actual cell widths.
   Tab title max chars calculated dynamically from TAB_WIDTH and cell_width.

5. **Status bar** (`status_bar.rs`): All text width calculations use `text_width()`.
   CWD display adapts to available space. Background changed to BG_SURFACE for
   visibility against the darker terminal area.

6. **Title bar** (`title_bar.rs`): Text centering uses actual cell_height. Background
   matches BG_DARK for consistency.

7. **Layout** (`layout.rs`): Increased dimensions for high-DPI: sidebar 200→240px,
   title bar 38→40px, tab bar 36→40px, status bar 28→40px.

8. **Window size** (`main.rs`): Reduced from 1200×800 to 1100×680 logical pixels so
   the window fits on 1920×1080 screens at 1.5× DPI without the status bar being
   pushed off-screen (physical size 1650×1020 fits within 1080px screen height).

**Result**: All UI chrome text (sidebar, tabs, title, status bar) renders with correct
spacing using actual font metrics. Status bar shows process name (green), working
directory (gray), git branch (peach), and terminal dimensions (muted). No text overlap
or clipping issues. Window fits on standard displays.

## Iteration 16 — Terminal content display + visual polish

**Goal**: Get terminal content displaying (was blank due to stale daemon) and polish UI details.

**Problems identified**:
1. Terminal area was completely blank — stale daemon process from previous session had
   closed its IPC channel, causing "channel is empty and sending half is closed" errors.
2. Status bar had hardcoded "1 file changed +21 -4 -70" text instead of real git data.
3. No user feedback when terminal has no content (just blank dark area).
4. Active tab lacked visual framing to distinguish it from inactive tabs.

**Changes made**:

1. **Status bar** (`status_bar.rs`): Added `git_diff_summary` field to replace hardcoded
   file change text. Now populated from `git diff --shortstat` at startup.

2. **Terminal placeholder** (`main.rs`): When `current_grid` is None, renders contextual
   placeholder text ("Connecting to terminal...", "Starting session...", or "Waiting for
   output...") in the terminal area using FG_MUTED color.

3. **Active tab framing** (`tab_bar.rs`): Added left/right border lines on the active tab
   for a "raised tab" effect. The accent bar thickness reduced from 3px to 2px for
   subtlety. Bottom border is cleared between the side borders to blend into terminal.

4. **Daemon restart**: Killed stale daemon, rebuilt daemon + pty-shim + godly-shell,
   verified all three processes launch correctly and terminal displays PS prompt with
   working keyboard input.

**Result**: Terminal now renders PowerShell output with full color support — yellow
commit hashes, green branch names, red error text, cyan decorations. ClearType subpixel
rendering is crisp. Status bar shows real git data. Active tab has professional visual
framing. Placeholder text provides feedback during session startup.

**Remaining gaps vs reference**:
- Split pane rendering (reference shows vertical split with text content on right)
- Status bar could show "Streaming response..." indicator for active processes
- Session working directory could be detected dynamically from PTY

## Iteration 18 — Fix sidebar/tab bar layout overlap

**Goal**: Fix the sidebar overlapping the tab bar area, causing "Godly Terminal" title
to appear at top-left instead of the tab bar spanning the full width.

**Problem identified**:
The sidebar rect started at `y = title_h` (which was 0, same as the tab bar). This
caused the sidebar to draw its BG_DARK background over the left portion of the tab bar,
hiding tabs 1-2 and replacing them with the sidebar's "Sessions" header. The reference
image clearly shows the tab bar spanning the full width above the sidebar.

**Change made**:

1. **Layout** (`layout.rs`): Changed sidebar y from `title_h` to `title_h + tab_h` so the
   sidebar starts below the tab bar. Sidebar height adjusted to `viewport_h - title_h - tab_h - status_h`.

**Result**: Tab bar now spans full width at the top with all 5 tabs visible (colored dot +
number + title for each). Sidebar sits below the tab bar with "Sessions" header, session
items, and agent panel. Terminal area visible with PowerShell prompt. Layout now matches
the reference image structure: tab bar → sidebar + terminal → status bar.

**Remaining gaps vs reference**:
- Split pane rendering (reference shows vertical split with text content on right)
- Could add more terminal content to demonstrate rendering quality
- Sidebar session items could show more detail (working directory, active process)
