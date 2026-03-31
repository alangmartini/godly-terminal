# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-03-30 (Iteration 22)

## Reference Targets
- **Zed** (`reference-zed.png`): Minimal, clean dark theme with subtle separators and warm palette
- **opensessions** (`reference-opensessions.png`): Terminal multiplexer with colored session tabs, sidebar, status bar

## What We Match

| Element | Status | Notes |
|---------|--------|-------|
| Tab bar with colored indicators | Done | Colored circles per session, close buttons |
| Session/workspace sidebar | Done | Active accent bar, notification dots, count badges |
| Sidebar header with controls | Done | "WORKSPACES" header with gear + add buttons |
| Soft panel separators | Done | All borders reduced to ~45-55% opacity (iteration 22) |
| Active tab visual treatment | Done | Top-rounded corners, accent tint, subtle shadow |
| Window controls | Done | Canvas-drawn icons, red close hover (iteration 22) |
| Status bar with shell badge | Done | Process badge, CWD, dimensions display |

## Remaining Gaps (Priority Order)

### High Impact
1. **Terminal content rendering** - Can only verify with active daemon; "Starting session..." placeholder shows when daemon is not running
2. **Active tab content connection** - Zed's active tab seamlessly blends into editor; our separator (even at 45%) still creates a slight edge. A "gap" in the separator under the active tab would be ideal but requires custom rendering.

### Medium Impact
3. **Sidebar text readability** - Branch names and folder paths at 10pt are very small. Reference apps use 11-12pt for secondary text.
4. **Tab bar compactness** - Zed's tab bar is slightly more compact (~32px); ours is 36px.
5. **Status bar visibility** - Status bar can be hard to see due to similar bg color to sidebar. Could benefit from slightly different shade.

### Low Impact
6. **Context menu backdrop blur** - Professional apps use backdrop blur for floating menus; Iced doesn't support this natively.
7. **Smooth hover transitions** - Reference apps have animated hover transitions; Iced uses instant state changes.
8. **Font weight differentiation** - Active tab labels could use medium weight vs normal for inactive.

## Theme Color Notes (ZedOneDark)
- Terminal/content: `#282c33` (darkest)
- Sidebar/chrome: `#2f343e` (medium)
- Borders (full): `#464b57` (visible but now rendered at reduced opacity)
- Accent: `#74ade8` (blue)
