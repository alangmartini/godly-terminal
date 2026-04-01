# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-04-01 (Iteration 76)

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
| DPI scaling | Done | Surface at logical resolution, compositor upscales to physical (iteration 73) |
| Right panel | Done | Poem content panel with header, stanzas, footer, close button (iteration 73) |
| Tab demo data parity | Done | Tab names, active tab, badge counts matching web reference exactly (iteration 75) |
| Per-tab accent colors | Done | Each tab has its own accent color matching web: indigo, emerald, orange, violet, indigo (iteration 75) |
| Tab badge shape | Done | borderRadius 7 (rounded rect) instead of full pill, matching web Badge component (iteration 75) |
| Badge on active tab | Done | Badges shown on active tabs too, matching web where tab 2 shows badge:3 while active (iteration 75) |
| Per-element font sizing | Done | All UI text now matches web's exact fontSize per element: 10px shortcuts/badges, 11px branch/status, 12px headers/tabs, 13px names/stanzas, 15px poem title (iteration 76) |

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

### Future: HiDPI Rendering
The surface renders at logical resolution (1707×912) and the compositor upscales to
physical (2560×1440). Text is rendered at 1x resolution rather than native HiDPI.

**Investigation (iteration 74):** Tested DX12 backend with physical resolution surface
(2560×1368) and explicit `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`. The DWM
compositor still clips swap chain content at the logical pixel boundary (1707px),
confirmed via pixel analysis. This is a Windows DWM limitation — the compositor
presents only logical-width pixels of the swap chain regardless of GPU backend or DPI
awareness. Fixing this likely requires a different surface presentation strategy
(e.g., off-screen render target + blit, or Win32 API-level DXGI swap chain control
bypassing wgpu's abstraction).

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
