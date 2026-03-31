# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-03-30 (Iteration 27)

## Reference Targets
- **Zed** (`reference-zed.png`): Minimal, clean dark theme with subtle separators and warm palette
- **opensessions** (`reference-opensessions.png`): Terminal multiplexer with colored session tabs, sidebar, status bar

## What We Match

| Element | Status | Notes |
|---------|--------|-------|
| Tab numbered circle badges | Done | Numbers inside colored gradient circles, matching opensessions (iteration 25) |
| Session/workspace sidebar | Done | Active accent bar, notification dots, count badge pill |
| Sidebar header with count badge | Done | "Sessions" header with pill-shaped count badge (iteration 25) |
| Soft panel separators | Done | All borders at reduced opacity, embossed grooves |
| Active tab visual treatment | Done | Top-rounded corners, inverse "ear" concave curves, accent glow, shimmer |
| Window controls | Done | Canvas-drawn icons, animated hover states |
| Status bar with shell badge | Done | Process badge, CWD, dimensions, animated pill hovers |
| Chrome compactness | Done | Tab bar 36px, status bar 28px, sidebar header 34px |
| Inactive tab shapes | Done | Tab-shaped with gradient, border, alpha 0.75 |
| Terminal padding | Done | 8px left, 6px top |
| Warm color palette | Done | Zed One Dark warm neutrals |
| Empty terminal welcome | Done | Branded header + accent underline + styled keycap shortcut cards (iteration 25) |
| Bottom separator break | Done | Active tab breaks separator with SDF inverse corner ears |
| SDF rendering | Done | Anti-aliased rounded rects, circles, borders, inner shadows |
| Gradients and depth | Done | Vertical/horizontal gradients, inner shadows, convexity gradients |
| Breathing glow animations | Done | Active elements have ambient glow with ~3.5s breathing cycle |
| Agent status badges | Done | Pill-shaped tinted badges for running/waiting/stopped (iteration 25) |
| Window frame border | Done | Multi-layer shadow + border + accent-tinted top edge |
| Clear color correction | Done | Background clear color matches One Dark palette (iteration 25) |
| Clean tab bar | Done | Removed hardcoded placeholder pills, tabs use full available width (iteration 26) |
| Zed-style sidebar header | Done | "SESSIONS" uppercase muted text like Zed section headers (iteration 26) |
| Accent glow continuity | Done | Active tab accent glow bleeds into content area for visual connection (iteration 26) |
| PROCESSES section header | Done | Uppercase muted header with count badge, matching SESSIONS style (iteration 27) |
| Welcome screen subtitle | Done | "GPU-accelerated terminal" subtitle below branded header (iteration 27) |
| Loading spinner animation | Done | Spinning arc indicator dots next to status message (iteration 27) |
| Status bar text contrast | Done | Content pills use FG_SECONDARY base instead of FG_MUTED for readability (iteration 27) |

## Remaining Gaps (Priority Order)

### High Impact
1. **Terminal content rendering** - Can only verify with active daemon; welcome screen shows when daemon is not running
2. **Proportional sidebar labels** - Session names use monospace font; references use proportional sans-serif for UI labels

### Medium Impact
3. **Tab bar could be even more compact** - Zed's tab bar is ~30-32px; ours is 36px
4. **Multi-pane terminal layout** - opensessions reference shows 2-3 panes side by side; requires daemon

### Low Impact
5. **Context menu backdrop blur** - Professional apps use backdrop blur for floating menus
6. **Font weight differentiation** - Active tab labels could use medium/semibold weight

## Active Theme Color Notes (Zed One Dark)
- Chrome/sidebar: `#1b1e24` (BG_DARK)
- Content/terminal: `#21252b` (BG_BASE)
- Elevated panels: `#1e2228` (BG_RAISED)
- Surface/hover: `#2c313a` (BG_SURFACE)
- Hover states: `#343946` (BG_HOVER)
- Text: `#abb2bf` (FG_PRIMARY)
- Subtext: `#828997` (FG_SECONDARY)
- Muted: `#5c6370` (FG_MUTED)
- Blue accent: `#61afef`
- Green: `#98c379`
- Peach/Yellow: `#e5c07b`
- Purple: `#c678dd`
- Red: `#e06c75`
- Border: `#3e4451` (warm gray)
