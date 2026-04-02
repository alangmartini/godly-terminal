# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-04-02 (Iteration 85)

## Reference Targets
- **Web reference** (`web-reference.png`): The pixel-perfect target from `web/godly-terminal.jsx`
- **opensessions** (`reference-opensessions.png`): Terminal multiplexer with colored session tabs, sidebar, status bar

## What We Match

| Element | Status | Notes |
|---------|--------|-------|
| Tab numbered circle badges | Done | 18×18px circles with proportional font numbers, accent bg at 13% (iteration 74) |
| Tab flat shapes | Done | Flat rects with 2px colored bottom indicators, no rounded tops (iteration 64) |
| Tab active/inactive colors | Done | Active: #e6edf3, Inactive: #555d6b, matching web exactly (iteration 62) |
| Tab gap spacing | Done | 6px gaps between tabs matching web reference (iteration 63) |
| Tab bar background | Done | Flat #0f1117 fill, no gradient (iteration 64) |
| Tab bar bottom border | Done | Solid 1px #1a1d25 matching web `borderBottom` (iteration 67) |
| Tab bar top stripe | Done | Removed — web has no accent stripe at top (iteration 67) |
| Tab close buttons | Done | Faintly visible at rest, animated hover with red glow (iteration 58) |
| Sidebar flat background | Done | Flat #0b0d12, no gradient or shadow effects (iteration 67) |
| Sidebar right border | Done | Solid 1px #1a1d25 matching web `borderRight` (iteration 67) |
| Sidebar branding section | Done | Flat background matching sidebar tone (iteration 67) |
| Session items | Done | Rounded 6px corners, indigo active border, two-line layout (iteration 65-66) |
| Session header | Done | "Sessions {count}" inline mixed-case (iteration 66) |
| Active session "::" indicator | Done | Right-aligned on active session (iteration 66) |
| Session name font weight | Done | Bold (600) for all session names (iteration 66) |
| Status bar | Done | 26px, #0c0e14, solid 1px top border (iteration 67) |
| Status bar git diff | Done | Colorized +N -M format with green/red (iteration 67) |
| Status bar borders | Done | All separators solid 1px #1a1d25 (iteration 67) |
| Content area background | Done | #0e1017 (BG_BASE), distinct from sidebar (iteration 62) |
| Color palette | Done | All hex values match web reference exactly |
| SDF rendering | Done | Anti-aliased rounded rects, circles, borders |
| Proportional UI font | Done | Segoe UI for chrome labels, tab titles, status bar (iteration 33) |
| Window controls | Done | Canvas-drawn minimize/maximize/close with hover animations |
| Breathing glow animations | Done | ~3.5s ambient cycle on active elements |
| Tab active background | Done | #161920 (BG_TAB_ACTIVE), distinct from session active #171b24 (iteration 68) |
| Sidebar scrollbar | Done | 6px width, #2d333b thumb, 3px radius, hidden when no overflow (iteration 68) |
| Status bar text colors | Done | Path #3b4048, separators #2d333b, diff text #484f58 matching web exactly (iteration 69) |
| Session active name | Done | FG_BRIGHT (#e6edf3) not WHITE, matching web exactly (iteration 69) |
| Sidebar header opacity | Done | Full FG_MUTED (#6e7681), removed 0.65 alpha (iteration 69) |
| Branch text color | Done | STATUS_DEFAULT (#484f58) base, matching web exactly (iteration 69) |
| "::" indicator color | Done | STATUS_DEFAULT (#484f58), matching web exactly (iteration 69) |
| Sidebar action shortcuts | Done | Bottom bar with "~ cycle", "⊘ go", etc. matching web layout (iteration 70) |
| Session header lightning | Done | "⚡ 1" indicator after session count, color #484f58 (iteration 71) |
| Process panel layout | Done | Directory path header, two-line items with descriptions, dismiss ×, borderRadius 3 badges (iteration 71) |
| Process panel separators | Done | Solid 1px #13161d between items, solid top border (iteration 71) |
| Demo data parity | Done | 3 sessions matching web (plane, opensessions active, quiver) (iteration 71) |
| New Session button removed | Done | Removed from sidebar to match web reference which has no such button (iteration 72) |
| Tab bar process indicators | Done | Right-side "bun" + "opensessions" labels with colored dots before window controls (iteration 72) |
| DPI scaling | Done | Surface sizing now follows the real physical Win32 client size on Windows instead of leaving the shell stuck to a logical-sized presentation region (iteration 82) |
| Right panel | Done | Poem content panel with header, stanzas, footer, close button (iteration 73) |
| Tab demo data parity | Done | Tab names, active tab, badge counts matching web reference exactly (iteration 75) |
| Per-tab accent colors | Done | Each tab has its own accent color matching web: indigo, emerald, orange, violet, indigo (iteration 75) |
| Tab badge shape | Done | borderRadius 7 (rounded rect) instead of full pill, matching web Badge component (iteration 75) |
| Badge on active tab | Done | Badges shown on active tabs too, matching web where tab 2 shows badge:3 while active (iteration 75) |
| Per-element font sizing | Done | All UI text now matches web's exact fontSize per element: 10px shortcuts/badges, 11px branch/status, 12px headers/tabs, 13px names/stanzas, 15px poem title (iteration 76) |
| Tab badge count font | Done | Badge count text ("3", "12") now renders at 9px proportional bold (fontWeight 700), matching web's Badge component fontSize: 9, was incorrectly using 14px monospace (iteration 77) |
| Session number font | Done | Session ID numbers (1, 2, 3) now render at 12px proportional font matching web's fontSize: 12, fontWeight: 500, was incorrectly using 14px monospace (iteration 77) |
| Inactive name color | Done | Session names and agent names now use exact #9198a1 (FG_INACTIVE), was #8b949e (FG_SECONDARY) (iteration 78) |
| Top-level shell region layout | Done | `ShellLayout` now comes from a retained `taffy` tree instead of hand-solved top-level rectangles (iteration 79) |
| Tab bar retained layout sections | Done | Sidebar brand, tab strip, process-indicator block, controls gap, and window controls now come from a retained `taffy` tree instead of reserve/origin math (iteration 81) |
| DirectWrite UI text measurement | Done | UI text widths and glyph positions now come from a cached DirectWrite layout path instead of `ui_char_advances` / average-advance hacks (iteration 80) |
| Serif italic poem text | Done | Right-panel stanzas now use a real serif italic font face instead of synthetic skew (iteration 80) |
| Physical-pixel presentation | Done | `godly-shell` now sizes the wgpu surface from the real Win32 client rect on Windows instead of the stale logical-sized `winit` dimensions that left a black, unused lower/right region at 150% DPI (iteration 82) |
| Native physical-pixel capture | Done | `scripts/take-screenshot-now.ps1` now opts into DPI awareness and captures the physical desktop size instead of logical size (iteration 79) |
| Deterministic native window capture | Done | `scripts/take-screenshot-now.ps1 -WindowOnly` now captures the godly-shell window directly via `PrintWindow`, avoiding desktop-focus noise (iteration 80) |
| Screenshot diff harness | Done | `scripts/check-pixels.ps1` now emits MAE/RMSE, changed-pixel counts, and a diff image instead of probing a few fixed coordinates (iteration 79) |
| Normalized 1920×1080 parity capture | Done | `capture-web-reference.ps1`, `take-screenshot-now.ps1 -ClientOnly -ClientWidth 1920 -ClientHeight 1080`, and `measure-godly-shell-parity.ps1` now produce deterministic same-size web/native captures and run the diff in one path (iteration 81) |
| Reliable web reference refresh | Done | `capture-web-reference.ps1` now uses `browser-use screenshot` directly after viewport/token verification instead of the brittle Python base64 write-out path that intermittently failed to materialize the raw file (iteration 82) |
| Deterministic client capture default | Done | `take-screenshot-now.ps1 -ClientOnly` now defaults to `PrintWindow` unless explicitly overridden, so the parity harness no longer depends on desktop-focus screen capture (iteration 82) |
| Cropped web-reference scene mode | Done | `godly-shell` now has a dedicated `--web-reference-crop` / `GODLY_SHELL_REFERENCE_MODE=web_reference_crop` path that renders the visible `web-reference.png` composition instead of a live daemon-backed shell scene (iteration 83) |
| Off-frame chrome removed from parity scene | Done | The reference-mode tab bar hides branding, indicators, window controls, and the new-tab button; the sidebar hides its footer panels; the right panel and status bar are disabled so the native scene matches the cropped web target (iteration 83) |
| Deterministic native parity launch | Done | `measure-godly-shell-parity.ps1` now launches the built `godly-native.exe` in reference mode when no PID is provided, so screenshot diff runs no longer depend on a manually staged window (iteration 83) |
| Reference transcript surface | Done | The main pane now renders a deterministic fixed transcript matching the visible web-reference content instead of a PowerShell prompt or welcome screen (iteration 83) |
| Reference-mode CSS-pixel scaling | Done | The cropped parity scene now lays out chrome/text at fixed CSS-pixel sizes and compensates at rasterization time instead of inflating the whole scene by the Windows DPI scale factor (iteration 84) |
| Crop tab-strip intrinsic sizing | Done | The cropped parity scene now uses content-width tabs with inline badges, 2px left padding, and full-height 36px geometry instead of stretching tab slots across the strip (iteration 84) |
| Sidebar session-stack CSS layout | Done | The cropped sidebar session stack now comes from a shared web-CSS layout helper (`ui/sidebar_layout.rs`) reused by both render and hit-testing, closing the old branch-row height mismatch and switching to full-height active borders / 20px secondary-row indent (iteration 85) |
| Streaming status bar | Done | Status bar shows "~ Streaming response..." matching web reference demo state (iteration 78) |
| Agent demo data parity | Done | Agent 2 name "anu" → "amp", descriptions matched to web, claude-code has no description (iteration 78) |

## Changes in Iteration 85

1. **Sidebar session-stack geometry now comes from one shared web-CSS helper** — Added [`ui/sidebar_layout.rs`] and routed [`ui/sidebar.rs`] through it for both draw and hit-testing. The crop-mode header/list/session stack now uses the web reference constants directly (`12/14/4` header padding, `4/6` list inset, `7/8` item padding, `2px` row gap, `20px` secondary-row indent) instead of duplicating slightly different coordinate math in multiple places.
2. **The branch-row height bug is closed** — Before this pass, sidebar hit-testing/layout treated branch-bearing rows as two-line `46px` items, but the render path collapsed those same rows to the compact height whenever `description` was empty. That mismatch was compressing the visible stack and drifting the screenshot. The shared helper now keeps the row-height decision consistent for both paths.
3. **Active session border and row placement are closer to the browser capture** — Active cards now use a full-height 3px indigo left border like the web CSS instead of the old shortened floating bar. Session rows also now start from the list inset and use the web’s secondary-row x offset rather than aligning branch text to the name column.
4. **Transcript compositing experiment was honest but not decisive** — Added grayscale-AA helpers for UI-monospace transcript text and tried that path in [`ui/reference_pane.rs`]. The measurable movement was tiny, so the remaining transcript mismatch is still unresolved rather than solved.
5. **Measured baseline after the sidebar/layout pass** — Fresh same-size runs now report `Changed: 2,073,596 px (99.9998%)`, `Pixels over 10: 230,188 px (11.1009%)`, `MAE: 0.049211`, `RMSE: 0.108136`, `Max channel diff: 255`. That improves the over-10 error from iteration 84 (`11.5366%`) but still leaves parity materially unfinished.

## Changes in Iteration 84

1. **Reference-mode layout no longer multiplies the whole crop by Windows DPI** — [`main.rs`] now routes the cropped reference scene through a `ui_scale()` of `1.0` while keeping the wgpu surface in physical pixels. `UiTextRenderer`/`TextCommand` gained a `raster_scale` path, and [`terminal_renderer.rs`] applies it when choosing glyph sizes. That keeps the crop physically sharp without inflating every tab, sidebar row, and transcript line by the monitor scale factor.
2. **The crop tab strip now uses the web layout model instead of the full-shell stretch model** — [`ui/tab_bar_layout.rs`] gained a content-sized tab mode, and [`ui/tab_bar.rs`] now computes intrinsic widths for the crop, removes the vertical inset, uses the web’s 2px left strip padding, and places unread badges inline after the title instead of pinning them to the far right of a stretched slot.
3. **Measured baseline improved materially after the scale/layout fix** — Fresh same-size runs now report `Changed: 2,073,344 px (99.9877%)`, `Pixels over 10: 239,223 px (11.5366%)`, `MAE: 0.047128`, `RMSE: 0.101986`, `Max channel diff: 249`. That cuts the over-10 error roughly in half from the iteration-83 baseline (`22.0102%`), so the remaining diff is no longer dominated by the crop being globally oversized.
4. **What the new diff says** — The dominant remaining error is now concentrated in transcript glyph weight/line rhythm and the sidebar/session stack spacing, with smaller residual drift in tab-label/badge typography. The crop is substantially closer, but not yet at parity.

## Changes in Iteration 83

1. **The native diff now measures the same scene as `web-reference.png`** — Added a dedicated reference scene mode in [`main.rs`], backed by a deterministic transcript renderer in [`ui/reference_pane.rs`]. This bypasses the live daemon/PTy path and removes the old “PowerShell prompt vs. README transcript” mismatch that was dominating the previous comparison.
2. **Retained layout now supports the cropped screenshot composition** — [`ui/layout.rs`] can hide the status bar, and [`ui/tab_bar_layout.rs`] / [`ui/tab_bar.rs`] can hide branding, optional right-side sections, and window controls. The native parity scene now matches the cropped web screenshot instead of the full shell viewport.
3. **Sidebar/footer chrome is no longer polluting the crop** — [`ui/sidebar.rs`] gained a footer-visibility toggle so the parity scene can match the screenshot’s empty lower sidebar instead of rendering the process list and action strip that are outside the captured crop.
4. **One-command native launch for parity checks** — [`scripts/measure-godly-shell-parity.ps1`] now launches the built `godly-native.exe` in `--web-reference-crop` mode automatically when no PID is provided, then captures and diff-checks that scene.
5. **Measured baseline after the reference-scene pass** — Fresh same-size runs now report `Changed: 2,073,598 px (99.9999%)`, `Pixels over 10: 456,403 px (22.0102%)`, `MAE: 0.061262`, `RMSE: 0.139698`, `Max channel diff: 249`. This is materially better than the old live-shell comparison, but the remaining visible mismatch is now mostly real typography/spacing drift.
6. **Critical finding corrected** — `docs/references/web-reference.png` is a cropped top-left capture of the web prototype, not the full prototype viewport. The native parity target therefore must hide the right panel, lower sidebar footer, and bottom status chrome for the screenshot mode even though those elements still exist in the full prototype.

## Changes in Iteration 82

1. **Physical-pixel presentation blocker closed** — [`main.rs`] now sizes the wgpu surface from the real Win32 client rect via `GetClientRect` instead of trusting the stale logical-sized `winit` dimensions under 150% DPI. The prior behavior painted only the top-left logical region inside a `1920×1080` client, leaving a large black lower/right area in captures and on screen.
2. **Resize path now uses the same physical client query** — The Windows resize handler no longer configures the surface directly from `WindowEvent::Resized`. It now re-queries the HWND client rect and uses that physical size, keeping live resize behavior aligned with the corrected startup path.
3. **Client/window capture is deterministic by default** — [`scripts/take-screenshot-now.ps1`] now auto-enables `PrintWindow` for `-ClientOnly` / `-WindowOnly` captures unless explicitly overridden. The loop’s default native artifact path is now window-targeted instead of depending on which desktop surface happened to be visible.
4. **Web reference refresh path is no longer brittle** — [`scripts/capture-web-reference.ps1`] still verifies the expected DOM tokens and viewport, but it now uses the `browser-use screenshot` subcommand to write the PNG directly. This removes the failing Python base64 file-write path that was breaking `measure-godly-shell-parity.ps1`.
5. **Measured baseline after the physical-surface fix** — Fresh `1920×1080` parity runs now diff at `Changed: 2,071,780 px (99.9122%)`, `Pixels over 10: 1,132,168 px (54.5992%)`, `MAE: 0.062596`, `RMSE: 0.123352`, `Max channel diff: 253`. The absolute parity gap is still large, but the black unused client region is gone and the error budget now reflects real layout/content differences rather than a broken presentation path.
6. **Verification** — `cargo check -p godly-shell`, `cargo build -p godly-shell`, `cargo test -p godly-shell ui::layout -- --nocapture`, `powershell -ExecutionPolicy Bypass -File scripts/capture-web-reference.ps1`, and `powershell -ExecutionPolicy Bypass -File scripts/measure-godly-shell-parity.ps1 -ProcessId <pid>` all passed with the updated scripts/code.

## Changes in Iteration 81

1. **Retained tab-bar layout landed** — Added [`ui/tab_bar_layout.rs`] to move the tab bar’s main horizontal structure onto a persistent `taffy` tree. The sidebar brand block, tab strip, new-tab slot, process-indicator reserve, controls gap, and window controls no longer depend on a single handwritten reserve/origin formula in [`ui/tab_bar.rs`].
2. **Tab-bar hit testing now uses the retained geometry** — [`ui/tab_bar.rs`] now asks the layout engine for tab, button, and new-tab rectangles during both render and hover/click handling. This removes a second copy of the bar geometry and makes the `+` button produce `UiAction::NewTab` instead of being paint-only.
3. **Deterministic 1920×1080 web capture added** — [`scripts/capture-web-reference.ps1`] now starts the local Vite reference when needed and uses the `browser-use` session API plus the underlying page viewport to save an exact `1920×1080` web screenshot to [`docs/references/web-reference.png`].
4. **Deterministic 1920×1080 native client capture added** — [`scripts/take-screenshot-now.ps1`] now supports `-ClientOnly -ClientWidth <w> -ClientHeight <h>`, restores/resizes the shell window to an exact client area, captures it with `PrintWindow`, and crops to the client rect so shadows and invisible frame math no longer pollute the parity image.
5. **One-command parity measurement path added** — [`scripts/measure-godly-shell-parity.ps1`] now refreshes both captures and runs [`scripts/check-pixels.ps1`] against them, which gives the loop a reproducible same-size visual baseline instead of the previous `2560×1440` vs `2582×1390` mismatch.
6. **Measured baseline after normalization** — Fresh `1920×1080` captures currently diff at `Changed: 2,073,600 px (100%)`, `Pixels over 10: 2,071,289 px (99.8886%)`, `MAE: 0.124377`, `RMSE: 0.132828`, `Max channel diff: 241`. The harness is now honest and comparable; parity itself is still far away.
7. **Verification** — `cargo check -p godly-shell`, `cargo test -p godly-shell ui::tab_bar_layout -- --nocapture`, `powershell -ExecutionPolicy Bypass -File scripts/capture-web-reference.ps1`, `powershell -ExecutionPolicy Bypass -File scripts/take-screenshot-now.ps1 -ClientOnly -ClientWidth 1920 -ClientHeight 1080`, and `powershell -ExecutionPolicy Bypass -File scripts/measure-godly-shell-parity.ps1 -ProcessId <pid>` all passed.

## Changes in Iteration 80

1. **Real DirectWrite UI text layout path** — Added [`ui/text_layout.rs`] to cache DirectWrite measurements for sans and serif UI runs. `UiTextRenderer` now queries that engine for widths and per-glyph x offsets instead of relying on `ui_char_advances`, average-advance estimates, or manual truncation math.
2. **Chrome text rendering now respects font roles and italics** — `TextCommand` now carries a real font role (`terminal`, `ui sans`, `ui serif`) plus italic state. `TerminalRenderer` selects the matching rasterizer, so the poem panel now renders through a real serif italic face instead of the old synthetic skew.
3. **Background-aware text compositing landed** — `TextCommand` also now carries a compositing mode. Flat opaque chrome surfaces use ClearType-style subpixel blending against their actual background color, while mixed surfaces (welcome hero, breadcrumb gradient) explicitly opt into grayscale AA.
4. **DirectWrite advance hacks removed from the frame build** — `main.rs` no longer builds the old `ui_char_advances` table, and the builder helpers now attach real glyph positions directly to text commands. This materially closes the faux-layout blocker called out in the previous iteration.
5. **Screenshot helper gained deterministic window capture** — `scripts/take-screenshot-now.ps1` now supports `-WindowOnly`, using `PrintWindow` to capture the godly-shell window directly when full-desktop capture is polluted by whichever app Windows keeps in the foreground.
6. **Verification** — `cargo check -p godly-shell`, `cargo test -p godly-shell ui::layout -- --nocapture`, and `cargo test -p godly-shell ui::builder -- --nocapture` all passed. A fresh native window capture was saved to `docs/references/current-godly-shell.png` using the new `-WindowOnly` mode.

## Changes in Iteration 79

1. **Top-level shell layout moved onto retained `taffy` nodes** — Replaced the hand-written `ShellLayout::compute()` region solver with a persistent `ShellLayoutEngine` backed by a `taffy::TaffyTree`. The shell now computes tab bar/body/sidebar/center/right panel/status bar through flex layout and only derives `terminal_content` as a post-layout inset. This closes the highest-leverage rectangle-math blocker at the outer shell layer without rewriting every widget in one pass.
2. **App render/input path now routes through the retained layout engine** — `main.rs` holds a `RefCell<ShellLayoutEngine>` and all region queries (`terminal_size`, render, hover, resize-handle hit testing, selection routing) now pull from the same retained layout tree instead of rebuilding top-level geometry ad hoc.
3. **Layout verification added** — `cargo test -p godly-shell ui::layout -- --nocapture` now exercises the retained layout engine for visible, hidden, and scaled configurations so layout regressions are caught before screenshot comparison.
4. **Screenshot-diff harness is now measurable** — `scripts/check-pixels.ps1` was rewritten to compute full-image metrics (`changed_pixels`, `pixels_over_10`, `max_channel_diff`, `MAE`, `RMSE`) and emit `current-godly-shell.diff.png`. This replaces the previous hard-coded spot sampling script.
5. **Native screenshot capture is now DPI-aware** — `scripts/take-screenshot-now.ps1` now calls `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` before reading screen bounds, so captured screenshots are in physical pixels (`2560x1440` here) rather than logical pixels (`1707x960`).
6. **Measured baseline after the harness landed** — Fresh captures at `2560x1440` currently diff at `Changed: 3,686,400 px (100%)`, `Pixels over 10: 3,684,752 px (99.9553%)`, `MAE: 0.350239`, `RMSE: 0.500536`, `Max channel diff: 245`. The harness is working, and it confirms parity is still far away.

## Changes in Iteration 78

1. **FG_INACTIVE color constant** — Added `#9198a1` ([0.569, 0.596, 0.631]) as `FG_INACTIVE` to the color palette. Web uses this for inactive session names, agent names, and poem stanzas. Previously these elements used `FG_SECONDARY` (#8b949e) which was slightly too dark.
2. **Inactive session name color fixed** — Session names when not active now lerp from `FG_INACTIVE` (#9198a1) instead of `FG_SECONDARY` (#8b949e), matching web's exact `color: isActive ? "#e6edf3" : "#9198a1"`.
3. **Agent name color fixed** — Agent/process names in sidebar now use `FG_INACTIVE` base color matching web's `color: "#9198a1"`.
4. **Synthetic italic for poem stanzas** — Added `skew` field to `TextCommand` and `text_ui_italic_scaled()` method to `UiBuilder`. The terminal renderer shifts top vertices of glyph quads rightward by `skew * height` to simulate italic. Poem stanzas now render with ~12° slant approximating web's `fontStyle: italic`.
5. **Agent demo data matched** — Agent 2 name changed from "anu" to "amp", agent descriptions updated to match web reference exactly ("Verify and clean README documentation", "Verify README against codebase"), claude-code agent has empty description matching web.
6. **Streaming status bar** — Set `streaming = true` in demo status bar initialization to match web reference which shows "~ Streaming response... Esc to cancel" in the status bar.

## Changes in Iteration 77

1. **Tab badge count font fixed** — Badge count text on tabs (e.g., "3", "12") was rendering using `ui.text()` (monospace font at 14px) instead of `ui.text_ui_bold_scaled()` (proportional font at 9px). Changed to match web's Badge component: `fontSize: 9, fontWeight: 700, color: "#fff"`. Text width measurement also updated to use `text_width_ui_scaled` with PX9 scale for correct badge width calculation. Vertical centering updated to use scaled glyph height.
2. **PX9 font scale constant** — Added `PX9 = 9.0 / 14.0` (0.643) to `font_scale` module for the tab badge count text size.
3. **Session number font fixed** — Session ID numbers in sidebar (1, 2, 3) were rendering using `ui.text()` (monospace at 14px) but web uses `fontSize: 12, fontWeight: 500, color: "#555d6b"`. Changed to `ui.text_ui_scaled()` with PX12 scale, matching the proportional font and correct size.

## Changes in Iteration 76

1. **Per-element font size scaling** — Added `scale` field to `TextCommand` and `text_ui_scaled()`/`text_ui_bold_scaled()` methods to `UiBuilder`. The renderer now multiplies glyph quad dimensions and advance widths by the scale factor, enabling different text sizes without re-rasterizing glyphs. Scale constants defined in `font_scale` module: PX10 (0.714), PX11 (0.786), PX12 (0.857), PX13 (0.929), PX15 (1.071).
2. **Sidebar font sizes matched** — Sessions header: 12px, lightning indicator: 10px, session names: 13px bold, "::" indicator: 11px, branch text: 11px, description text: 11px.
3. **Process panel font sizes matched** — Directory path header: 10px, status icons: 11px, agent names: 12px bold, status badges: 10px, dismiss ×: 13px, task descriptions: 11px.
4. **Sidebar shortcuts bar: 10px** — Shortcut labels ("~ cycle", "⊘ go", etc.) now render at 10px matching web's `fontSize: 10`. Both measurement and rendering use scaled widths for correct wrap layout.
5. **Tab bar font sizes matched** — Tab circle badge numbers: 10px, tab titles: 12px (bold when active), right-side process indicators ("bun", "opensessions"): 11px bold.
6. **Status bar: 11px** — All status bar text (streaming indicator, process name, path, branch, diff stats, separators) renders at 11px matching web's `fontSize: 11`.
7. **Right panel font sizes matched** — Header title: 12px, poem title: 15px bold, poem stanzas: 13px (with 1.7 line-height on 13px base), footer: 12px, bottom status bar: 11px.
8. **`text_width_ui_scaled()` helper** — Added to `UiTextRenderer` for correct layout calculations at scaled sizes. Used throughout for centering, truncation, and positioning.

## Changes in Iteration 75

1. **Per-tab accent color support** — Added optional `accent` field to `TabInfo` struct. Each tab can now override the index-based color rotation with a specific accent color. Demo tabs use exact web reference colors: #6366f1 (indigo), #10b981 (emerald), #f97316 (orange), #8b5cf6 (violet), #6366f1 (indigo).
2. **Demo tab data matched to web** — Tab names changed from (plane, opensessions, quiver, godly-terminal, notes) to (opensessions, opensessions, work, opensessions, opensessions). Active tab moved from index 0 to index 1 (2nd tab). Badge counts: tab 2 has 3, tab 4 has 12, matching web exactly.
3. **Tab badge shape fixed** — Changed from full pill (`badge_r = badge_h / 2.0`) to `borderRadius: 7` (`badge_r = s(7.0)`), matching web's Badge component. Also updated badge dimensions: height 16px, padding 5px, minWidth 16px.
4. **Badge visible on active tabs** — Removed the `active_t < 0.5` guard that hid badges on active tabs. Web reference shows badges on all tabs regardless of active state (tab 2 is active and has badge:3).
5. **New color constants** — Added `ACCENT_EMERALD` (#10b981) and `ACCENT_ORANGE` (#f97316) to match web tab colors that differed from existing palette entries.

## Changes in Iteration 74

1. **Tab number circle sizing** — Changed from `ch * 0.9` (~16.4px) to `s(18.0)` matching web reference's exact `width: 18, height: 18` circle dimensions.
2. **Tab number proportional font** — Switched from monospace font (`ui.text`) to proportional UI font (`ui.text_ui`) for the number inside each tab's circle badge, matching web reference's proportional rendering. Numbers now center better in the circle.
3. **DWM clipping investigation** — Tested DX12 backend + physical resolution surface with explicit Per-Monitor DPI Awareness v2. Confirmed that the Windows DWM compositor clips swap chains at logical pixel boundaries regardless of GPU backend (DX12 or Vulkan) or DPI awareness settings. Surface remains at logical resolution.

## Changes in Iteration 73

1. **DPI scaling fix** — wgpu surface now configured at logical resolution instead of physical resolution. On Windows with 150% DPI scaling, the compositor clips the swap chain at the logical pixel boundary (1707px), making content rendered at physical coordinates >1707 invisible. Fixed by computing `logical_w = physical_w / scale_factor` and configuring the surface at that size. Layout now uses `scale_factor=1.0` since all coordinates are in logical pixels. The compositor handles upscaling to physical resolution.
2. **Right panel enabled** — The right panel with "The Gardener of Broken Things" poem is now visible. Previously hidden behind `visible: false` due to the DPI clipping issue. Now renders correctly with header, poem stanzas, footer, close button, and bottom status bar matching web reference.
3. **Resize handler updated** — `WindowEvent::Resized` now converts physical size to logical before reconfiguring the surface, matching the init_gpu approach.

## Changes in Iteration 72

1. **Removed "+ New Session" button** — Sidebar no longer renders the green "+ New Session" CTA button, which did not exist in the web reference. Removed ~90 lines of button rendering, animation, hover state, and hit-testing code.
2. **Tab bar right-side process indicators** — Added "bun" (orange dot) and "opensessions" (green dot) labels on the right side of the tab bar before window controls, matching web's `display: "flex", gap: 10, paddingRight: 14, fontSize: 11, color: "#555d6b", fontWeight: 600`.
3. **Tab width calculation updated** — Reserved 150px for right-side indicators to prevent tab overlap.

## Changes in Iteration 71

1. **Session header lightning indicator** — Added "⚡ 1" text after "Sessions N" header, matching web's `color: "#484f58"` (STATUS_DEFAULT) and inline positioning.
2. **Process panel header** — Changed from "Processes N" to truncated directory path ("…ments/work/opensessions"), matching web's `padding: "8px 10px 4px"`, `fontSize: 10`, `color: "#484f58"`.
3. **Process item descriptions** — Added second line with task text below each agent name, matching web's `fontSize: 11`, `color: "#484f58"`, `paddingLeft: 20`.
4. **Process status badges** — Changed from pill shape (full radius) to `borderRadius: 3` matching web. Opacity changed from 0.12 to 0.094 (web's "18" hex).
5. **Process icons** — Changed from plain dots to text symbols (ⓘ running, ⚠ stopped, ● waiting) matching web reference.
6. **Agent name weight** — Changed to bold (`text_ui_bold`) matching web's `fontWeight: 600`.
7. **Process dismiss button** — Added × button right-aligned on each process item, matching web's `color: "#3b4048"`.
8. **Process separators** — Changed from faded lines to solid 1px `#13161d`, matching web's `borderBottom`.
9. **Process panel top border** — Changed from faded divider to solid 1px `#1a1d25`, matching web's `borderTop`.
10. **Demo data** — Matched to web reference: 3 sessions (plane, opensessions active, quiver), removing 4th "godly-terminal" session.

## Changes in Iteration 70

1. **Sidebar action shortcuts bar** — Added wrapping shortcut labels ("~ cycle", "⊘ go", "d remove", "u restore", "x kill", "t theme") at the very bottom of the sidebar, matching web reference's `borderTop: "1px solid #1a1d25"`, `padding: "6px 10px"`, `color: "#3b4048"`, and `gap: "4px 10px"` flex-wrap layout.
2. **Removed Settings row** — Replaced bottom Settings gear + "Ctrl+," row with the action shortcuts bar to match web reference, which shows shortcuts rather than settings in this position.
3. **Agent panel positioning updated** — Bottom agent/process panel now accounts for shortcuts bar height instead of old settings row height.

## Changes in Iteration 69

1. **Status bar path color fixed** — Changed from FG_MUTED (#6e7681) to new STATUS_PATH (#3b4048), matching web's `color: "#3b4048"` for directory paths.
2. **Status bar separator color fixed** — Changed from FG_MUTED (#6e7681) to BG_HOVER (#2d333b), matching web's `color: "#2d333b"` for "|" separators.
3. **Status bar diff text color fixed** — Non-colored diff tokens (e.g. "1 file changed") changed from FG_SECONDARY (#8b949e) to new STATUS_DEFAULT (#484f58), matching web's inherited `color: "#484f58"`.
4. **Active session name capped at FG_BRIGHT** — Was lerping to pure WHITE (#ffffff), now lerps to FG_BRIGHT (#e6edf3) matching web's `color: isActive ? "#e6edf3" : "#9198a1"`.
5. **New palette entries** — Added STATUS_PATH (#3b4048) and STATUS_DEFAULT (#484f58) to `colors` module for precise status bar text hierarchy.
6. **Sidebar header opacity fixed** — "Sessions N" text was at 0.65 alpha (effectively ~#4b515a), now at full FG_MUTED (#6e7681) matching web's `color: "#6e7681"`.
7. **Branch text corrected** — Changed from approximated `FG_MUTED*0.7` (~#4d535a) to exact STATUS_DEFAULT (#484f58) matching web's `color: "#484f58"`.
8. **"::" indicator corrected** — Changed from `FG_MUTED*0.7` at 0.65 alpha (~#363a41) to STATUS_DEFAULT (#484f58) matching web.

## Changes in Iteration 68

1. **Active tab background corrected** — Tab active background changed from BG_ACTIVE (#171b24) to new BG_TAB_ACTIVE (#161920), matching web's exact `backgroundColor: '#161920'` for active tabs. Session active background remains #171b24 as web uses different colors for tabs vs sessions.
2. **Sidebar scrollbar matches web CSS** — Scrollbar width changed from 2px to 6px, thumb color from FG_MUTED at 0.08 alpha to solid #2d333b (BG_HOVER), hover color #3b4048, border-radius 3px. Track made transparent. Scrollbar now hidden when all items fit (matching web CSS overflow behavior).

## Changes in Iteration 67

1. **Border opacities fixed** — All major separators (sidebar right, tab bar bottom, status bar top) changed from ultra-low opacity (0.12-0.50) to solid 1px #1a1d25, matching web reference's `borderRight/borderBottom/borderTop: "1px solid #1a1d25"`.
2. **Removed top accent stripe** — Tab bar no longer has a 2px colored gradient stripe at the top with glow spill. Web reference has no such element.
3. **Removed sidebar shadows** — Inward shadow gradient and SDF inner shadow removed from sidebar. Web uses clean flat background.
4. **Flattened sidebar gradient** — Sidebar background is now solid #0b0d12 instead of 4% top-to-bottom gradient.
5. **Flattened branding section** — Tab bar sidebar branding section uses solid #0b0d12 instead of gradient.
6. **Git diff summary parsing** — `git diff --shortstat` output now parsed into "+N -M" format for proper green/red colorization in status bar.

## Remaining Gaps (Priority Order)

### Critical: Transcript typography/compositing still diverges from the browser capture
After the sidebar/session-stack correction, the main remaining mismatch is the transcript text itself:
glyph weight, anti-aliasing contrast, and vertical rhythm still drift from the browser capture,
especially across the long body lines and headings. The latest grayscale-AA experiment moved the
metrics only marginally, so the blocker is still open.

**Next measurable step:** compare the crop transcript’s mono rendering/compositing path against the
browser screenshot more directly (glyph weight, baseline, and AA mode), then retune the reference
pane using the measured result rather than assuming the current mono path is already equivalent.

### Major: Sidebar/session stack spacing is still visibly off
The shared session-stack helper removed the old row-height mismatch and got the active card much
closer, but the sidebar header and row rhythm are still a little tighter than the browser capture.

**Next layout step:** keep using the shared session-stack helper as the single source of truth and
retune the remaining sidebar constants from screenshot evidence instead of reintroducing per-callsite
manual offsets.

### Major: Some tab-strip typography drift remains
The crop tab model is now structurally correct, but the label/badge text still reads slightly
lighter and less precisely placed than the browser capture.

**Next text step:** retune tab chrome typography after the transcript path is corrected, using
the updated parity baseline rather than the old stretched-tab geometry.

### Major: Inner chrome/content layout is still mostly manual
The top-level shell and tab bar already use retained layout, and the sidebar session stack now has
a shared CSS-layout helper, but the transcript flow is still largely hand-positioned and the
sidebar helper is not yet a full retained flex tree. That is materially better than iteration 82,
but it is still short of the parity gates.

**Next architectural step:** move the cropped transcript flow onto a retained flex/layout layer and
decide whether the sidebar helper should graduate into a fuller retained node tree once the
remaining transcript blocker is under control.

### Medium: Measurable visual gaps remain after the scale/layout fix
The latest same-size run still has `11.1009%` of pixels differing by more than 10 RGB
levels. That is a meaningful improvement over iteration 84, but it still confirms parity is
materially unfinished.

### Low Impact (Polish)
1. **Context menu backdrop blur** — For future floating menus.

## Active Theme Color Notes (GitHub Dark)
- Chrome/sidebar: `#0b0d12` (BG_DARK)
- Content/terminal: `#0e1017` (BG_BASE)
- Elevated panels: `#0f1117` (BG_RAISED)
- Surface/hover: `#1a1d25` (BG_SURFACE)
- Hover states: `#2d333b` (BG_HOVER)
- Active selection: `#171b24` (BG_ACTIVE)
- Status bar: `#0c0e14` (BG_STATUS)
- Text bright: `#e6edf3` (FG_BRIGHT)
- Text primary: `#c9d1d9` (FG_PRIMARY)
- Text secondary: `#8b949e` (FG_SECONDARY)
- Text muted: `#6e7681` (FG_MUTED)
- Text dim: `#555d6b` (FG_DIM)
- Border: `#1a1d25` (BORDER)
- Indigo accent: `#6366f1`
- Green accent: `#22c55e`
- Amber accent: `#f59e0b`
- Violet accent: `#8b5cf6`
- Red accent: `#ef4444`
