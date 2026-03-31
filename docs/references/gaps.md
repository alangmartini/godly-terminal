# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-03-30 (Iteration 23)

## Reference Targets
- **Zed** (`reference-zed.png`): Minimal, clean dark theme with subtle separators and warm palette
- **opensessions** (`reference-opensessions.png`): Terminal multiplexer with colored session tabs, sidebar, status bar

## What We Match

| Element | Status | Notes |
|---------|--------|-------|
| Tab bar with colored indicators | Done | Colored circles per session, close buttons |
| Session/workspace sidebar | Done | Active accent bar, notification dots, count badges |
| Sidebar header with controls | Done | "Sessions" header with count |
| Soft panel separators | Done | All borders at reduced opacity, embossed grooves |
| Active tab visual treatment | Done | Top-rounded corners, inverse "ear" concave curves, accent glow, shimmer |
| Window controls | Done | Canvas-drawn icons, animated hover states |
| Status bar with shell badge | Done | Process badge, CWD, dimensions, animated pill hovers |
| Chrome compactness | Done | Tab bar 36px, status bar 28px, sidebar header 34px (iteration 23) |
| Inactive tab shapes | Done | Tab-shaped with gradient, border, alpha 0.75 (iteration 23) |
| Terminal padding | Done | 8px left, 6px top (iteration 23) |
| Bottom separator break | Done | Active tab breaks separator with SDF inverse corner ears |
| SDF rendering | Done | Anti-aliased rounded rects, circles, borders, inner shadows |
| Gradients and depth | Done | Vertical/horizontal gradients, inner shadows, convexity gradients |
| Breathing glow animations | Done | Active elements have ambient glow with ~3.5s breathing cycle |

## Remaining Gaps (Priority Order)

### High Impact
1. **Terminal content rendering** - Can only verify with active daemon; "Starting session..." placeholder shows when daemon is not running
2. **Proportional sidebar labels** - Session names use monospace font; references use proportional sans-serif for UI labels, giving a tighter, more polished feel

### Medium Impact
3. **Tab bar could be even more compact** - Zed's tab bar is ~30-32px; ours is 36px. Further reduction possible if text rendering supports smaller sizes.
4. **Multi-pane terminal layout** - opensessions reference shows 2-3 panes side by side; requires daemon for real content

### Low Impact
5. **Context menu backdrop blur** - Professional apps use backdrop blur for floating menus; custom GPU pipeline doesn't support this natively
6. **Font weight differentiation** - Active tab labels could use medium/semibold weight vs normal for inactive (requires multi-weight text renderer)
7. **Warm color option** - References use warmer brown/gray tones; our theme is blue/purple. A "Zed One Dark" warm palette option would match closer.

## Theme Color Notes (ZedOneDark-like)
- Terminal/content: `#282c33` (darkest, BG_BASE)
- Sidebar/chrome: `#2f343e` (medium, BG_DARK)
- Borders (full): `#464b57` (visible but rendered at reduced opacity)
- Accent: `#74ade8` (blue)
