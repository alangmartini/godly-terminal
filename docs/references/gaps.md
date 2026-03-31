# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-03-31 (Iteration 41)

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
| Chrome compaction | Done | Tab bar 33px, status bar 25px, sidebar header 30px, settings row 28px (iteration 28) |
| Sidebar-tab visual continuity | Done | Session accent dots match tab bar color cycle (blue, green, peach, mauve, red) (iteration 28) |
| Green "New Session" icon | Done | Plus icon uses accent green for visual pop (iteration 28) |
| Status bar content inner shadow | Done | Recessed shadow on content section for depth (iteration 28) |
| Embossed tab separators | Done | Groove pairs between tabs matching sidebar depth language (iteration 29) |
| Inactive tab baseline shadows | Done | Drop shadows below inactive tabs for physical depth (iteration 29) |
| Keycap badge drop shadows | Done | Physical "raised key" shadows on welcome shortcut badges (iteration 29) |
| Welcome container inner shadow | Done | Recessed inner shadow on shortcut card container (iteration 29) |
| Sidebar hover glow | Done | Accent-tinted glow shadow on hovered session items (iteration 29) |
| Sidebar item spacing | Done | Compact items 38px, two-line 52px for better vertical rhythm (iteration 30) |
| Active session prominence | Done | White-bright name, wider indicator bar, stronger blue ambient (iteration 30) |
| Metadata text readability | Done | Branch and description text visible at rest via luminance boost (iteration 30) |
| New Session CTA styling | Done | Green accent-tinted border, label, and hover glow (iteration 30) |
| Inactive tab readability | Done | Brighter title text, higher rest-state alpha/brightness (iteration 30) |
| Active tab contrast | Done | Brighter top gradient (1.18×) for clear active/inactive distinction (iteration 30) |
| Window accent top bar | Done | 2px accent-colored top edge with glow spill, stronger focused alpha (iteration 31) |
| Sidebar section dividers | Done | Groove separators between sessions, new-session button, and processes (iteration 31) |
| Circular new-tab button | Done | Proper circular icon button with rest-state border (iteration 31) |
| Git diff summary pill | Done | Styled pill in status bar for diff summary display (iteration 31) |
| Welcome vertical balance | Done | Content positioned at 33% from top for better visual balance (iteration 31) |
| Font weight hierarchy | Done | Bold text for active tabs, active sessions, branding, and welcome heading (iteration 32) |
| Proportional UI font | Done | Segoe UI Variable/Segoe UI for sidebar labels, tab titles, status bar, welcome screen (iteration 33) |
| Sidebar-tab accent color continuity | Done | Active workspace indicator, border, glow uses session's own accent color from rotating palette (iteration 34) |
| Terminal branding icon | Done | Canvas-drawn terminal monitor + prompt icon next to "Godly Terminal" text in title bar (iteration 34) |
| Sidebar metadata readability | Done | Branch/description text base luminance boosted (0.25→0.40 blend toward FG_SECONDARY) (iteration 34) |
| Shell type pill badges | Done | "pwsh"/"bash" pills right-aligned on sidebar sessions for shell identification (iteration 35) |
| Session working directories | Done | CWD shown as second-line text with "›" chevron prefix on sessions without descriptions (iteration 35) |
| Filled "New Session" CTA | Done | Green-tinted filled button (15% green wash) at rest, reads as primary action (iteration 35) |
| Tab-session name consistency | Done | Tab titles match sidebar session names (plane, opensessions, quiver, etc.) (iteration 35) |
| Settings keyboard shortcut | Done | "Ctrl+," hint right-aligned on Settings row for discoverability (iteration 35) |
| Status bar connection status | Done | "Ready" label with breathing green dot in mode pill (iteration 35) |
| Welcome screen hero icon | Done | Large SDF terminal icon with accent halo glow above title (iteration 36) |
| Content area edge vignettes | Done | Top/left/bottom gradient shadows for cinematic framing depth (iteration 36) |
| Welcome card container depth | Done | Drop shadow below card container for floating effect (iteration 36) |
| Version indicator | Done | Muted version string below welcome shortcut cards (iteration 36) |
| SDF chevron icon rendering | Done | Terminal icon chevron uses SDF rotated pills for clean scaling (iteration 36) |
| SDF folder icon | Done | Folder outline icon before CWD in status bar for visual identification (iteration 37) |
| SDF git branch icon | Done | Forked-line branch icon replacing dot before git branch in status bar (iteration 37) |
| Improved gear cog icon | Done | Gear icon now has 6 SDF teeth + center dot instead of plain ring (iteration 37) |
| Sidebar session timestamps | Done | Relative time labels ("5m", "2h", "1d", "3d") right-aligned on second line (iteration 37) |
| Breadcrumb/path bar | Done | Thin bar between tab bar and content showing CWD as segmented path with chevrons (iteration 37) |
| Content area radial spotlight | Done | Soft accent-tinted glow behind welcome screen for visual depth (iteration 37) |
| Session terminal icons | Done | Mini terminal prompt icons before session names, replacing plain colored dots (iteration 38) |
| Sidebar scrollbar track | Done | Thin decorative scrollbar rail on session list right edge (iteration 38) |
| Breadcrumb last-segment pill | Done | Subtle rounded background highlight on current directory in breadcrumb (iteration 38) |
| Breadcrumb left depth shadow | Done | Inner shadow at breadcrumb left edge for sidebar-cast depth (iteration 38) |
| Section disclosure triangles | Done | Small ▾ triangles before "SESSIONS" and "PROCESSES" section headers (iteration 39) |
| Sidebar version indicator | Done | Muted version string at bottom of sidebar below Settings row (iteration 39) |
| Tab close button glow | Done | Red accent glow shadow behind close button on hover (iteration 39) |
| Status bar encoding/LF labels | Done | "UTF-8" and "LF" muted text labels in status bar for professional completeness (iteration 39) |
| Flat tab badges | Done | Solid accent fill circles instead of 3D gradient badges (iteration 40) |
| Reduced glow breathing | Done | Breathing range narrowed to ±8% (was ±15%), all glow alphas halved across UI (iteration 40) |
| Sidebar accent dots | Done | 7px flat accent dots replacing 11px terminal icons for clean readability (iteration 40) |
| Modern sidebar right edge | Done | Gradient shadow + hairline border replacing embossed groove (iteration 40) |
| Flat unread badges | Done | Unread count badges use solid fill matching tab badge style (iteration 40) |
| Sidebar folder icons | Done | Codicon folder icon before workspace folder paths for visual identification (iteration 41) |
| Branded empty state card | Done | Terminal icon, heading, shortcut keycaps, CTA button in professional welcome card (iteration 41) |
| Status bar encoding/LF labels | Done | "UTF-8" and "LF" labels right-aligned matching IDE conventions (iteration 41) |
| Sidebar depth shadow | Done | Rightward drop shadow on sidebar content for panel depth separation (iteration 41) |
| Status bar git branch field | Done | Git branch icon + name field added (renders when branch is available) (iteration 41) |

## Remaining Gaps (Priority Order)

### High Impact
1. **Terminal content rendering** - Can only verify with active daemon; welcome screen shows when daemon is not running

### Medium Impact
2. **Multi-pane terminal layout** - opensessions reference shows 2-3 panes side by side; requires daemon

### Low Impact
3. **Context menu backdrop blur** - Professional apps use backdrop blur for floating menus

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
