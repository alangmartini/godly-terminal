# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-03-31 (Iteration 69)

## Reference Targets
- **Web reference** (`web-reference.png`): The pixel-perfect target from `web/godly-terminal.jsx`
- **opensessions** (`reference-opensessions.png`): Terminal multiplexer with colored session tabs, sidebar, status bar

## What We Match

| Element | Status | Notes |
|---------|--------|-------|
| Tab numbered circle badges | Done | Numbers inside colored circles with accent bg, matching web (iteration 25+) |
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

## Changes in Iteration 69

1. **Status bar path color fixed** — Changed from FG_MUTED (#6e7681) to new STATUS_PATH (#3b4048), matching web's `color: "#3b4048"` for directory paths.
2. **Status bar separator color fixed** — Changed from FG_MUTED (#6e7681) to BG_HOVER (#2d333b), matching web's `color: "#2d333b"` for "|" separators.
3. **Status bar diff text color fixed** — Non-colored diff tokens (e.g. "1 file changed") changed from FG_SECONDARY (#8b949e) to new STATUS_DEFAULT (#484f58), matching web's inherited `color: "#484f58"`.
4. **Active session name capped at FG_BRIGHT** — Was lerping to pure WHITE (#ffffff), now lerps to FG_BRIGHT (#e6edf3) matching web's `color: isActive ? "#e6edf3" : "#9198a1"`.
5. **New palette entries** — Added STATUS_PATH (#3b4048) and STATUS_DEFAULT (#484f58) to `colors` module for precise status bar text hierarchy.

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

### Feature Gaps (Not Styling)
1. **Sidebar process list** — Web shows running processes (amp, claude-code) at sidebar bottom. Native shows "+ New Session" button. Requires daemon process tracking.
2. **Sidebar action shortcuts** — Web shows keybinding hints ("~ cycle", "d remove") at sidebar bottom. Not yet implemented.
3. **Right panel** — Web shows contextual content panel on right. Native has layout support but panel hidden by default.
4. **Tab bar right-side indicators** — Web shows process indicators ("bun", "opensessions") on tab bar right side. Not implemented.
5. **Session lightning indicator** — Web shows small "⚡ 1" in session header. Not implemented.

### Low Impact (Polish)
6. **Context menu backdrop blur** — For future floating menus.

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
