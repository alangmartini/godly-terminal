# Rendering Quality Gaps: Current vs Reference

Last updated: 2026-03-31 (Iteration 62)

## Reference Targets
- **Zed** (`reference-zed.png`): Minimal, clean dark theme with subtle separators and warm palette
- **opensessions** (`reference-opensessions.png`): Terminal multiplexer with colored session tabs, sidebar, status bar

## What We Match

| Element | Status | Notes |
|---------|--------|-------|
| Tab numbered circle badges | Done | Numbers inside colored gradient circles, matching opensessions (iteration 25) |
| Session/workspace sidebar | Done | Active accent bar, notification dots, count badge pill |
| Sidebar header with count badge | Done | "Sessions" header with pill-shaped count badge (iteration 25) |
| Soft panel separators | Done | All borders at reduced opacity, uniform single-hairline separators (iteration 60) |
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
| SDF breadcrumb chevrons | Done | Path separators use SDF vector chevron icon instead of text `›` for crisp scaling (iteration 42) |
| Sidebar CWD folder icons | Done | Working directory lines show SDF folder icon instead of text chevron prefix (iteration 42) |
| Status bar cursor position | Done | "Ln X, Col Y" indicator matching VS Code/Zed IDE convention (iteration 42) |
| Empty state CTA button | Done | "Create terminal" accent-tinted pill button below shortcut cards and version (iteration 42) |
| SDF chevron-right icon | Done | New `icon_chevron_right` builder method for breadcrumb/path separators (iteration 42) |
| Window top accent stripe | Done | 2px dynamic accent line at window top edge, color follows active tab accent (iteration 43) |
| Accent glow spill | Done | Breathing glow gradient below top accent stripe for depth (iteration 43) |
| Status bar bottom accent edge | Done | Subtle accent-tinted bottom border for visual window frame bookending (iteration 43) |
| Sidebar resting borders removed | Done | Session items show borders only on hover/active, not at rest — matches Zed (iteration 44) |
| Section grooves → thin lines | Done | All sidebar section dividers use single hairlines instead of embossed groove pairs (iteration 44) |
| Status bar separator softened | Done | Top separator uses single thin line instead of groove + bevel (iteration 44) |
| Reduced inner shadows | Done | Sidebar and status bar inner shadows halved for lighter feel (iteration 44) |
| Scrollbar near-invisible | Done | Track and thumb at minimal opacity at rest, fade in on hover (iteration 44) |
| Tab bar border softening | Done | Bottom separator at 50% opacity, sidebar section uses thin line (iteration 44) |
| Surface effect reduction | Done | Tab bar bevel and glass sheen reduced for calmer aesthetic (iteration 44) |
| Status bar metadata dividers | Done | Vertical pipe separators between UTF-8, LF, Ln/Col, dimensions — matches VS Code/Zed (iteration 45) |
| Dynamic bottom accent stripe | Done | Bottom window edge accent follows active tab color (was hardcoded blue), matches top stripe (iteration 45) |
| Breadcrumb text readability | Done | Path segment text contrast boosted; last segment brighter; chevrons more visible (iteration 45) |
| Breadcrumb gradient background | Done | Subtle top-down gradient (darker near tab bar → lighter near content) for smooth transition (iteration 45) |
| White badge text | Done | Tab number badges and unread count badges use white text for maximum contrast on any accent color (iteration 46) |
| Hero icon prominence | Done | Welcome terminal icon uses 70% accent blend (was 45%) at 75% opacity (was 55%), stronger halo glow (iteration 46) |
| Welcome title brightness | Done | Title text uses FG_PRIMARY base (was FG_SECONDARY) at 88% opacity (was 70%) for hero heading prominence (iteration 46) |
| Sidebar active glow strength | Done | Active session ambient glow 0.08 (was 0.05), name brightens to full white on active (iteration 46) |
| Inactive tab border clarity | Done | Rest-state border alpha 0.28 (was 0.18) for visible tab shapes without hover (iteration 46) |
| Breadcrumb panel definition | Done | Gradient top darkened to 88% (was 93%), bottom separator 0.35 (was 0.25), folder icon uses FG_SECONDARY (iteration 46) |
| Status bar metadata readability | Done | Text alpha 0.65 (was 0.55), divider alpha 0.35 (was 0.25), diff badge uses FG_SECONDARY (iteration 46) |
| Agent badge definition | Done | Status badge background 0.18 (was 0.12), stroke 0.35 (was 0.25), hover background full opacity (iteration 46) |
| Close button red glow | Done | Hover glow alpha 0.15 (was 0.10) for stronger destructive action telegraph (iteration 46) |
| Welcome shortcut 2×2 grid | Done | Shortcuts in 2-column grid layout for compact professional appearance (iteration 47) |
| Full-width CTA button | Done | "Create terminal" button spans full card width with gradient fill for visual weight (iteration 47) |
| CTA button gradient | Done | Subtle top-lighter gradient on accent CTA for physical button depth (iteration 47) |
| Shortcut-CTA divider | Done | Thin hairline separator between shortcut grid and CTA for visual section clarity (iteration 47) |
| Keycap gradient styling | Done | Key badges use top-lighter gradient background for raised keycap appearance (iteration 47) |
| Tab bar codicon plus | Done | New tab "+" button uses codicon icon with hover border instead of text (iteration 47) |
| Compact sidebar sessions | Done | Single-line entries for sessions without descriptions; CWD removed (shown in breadcrumb/status bar) (iteration 48) |
| 2×2 shortcut grid | Done | Welcome screen shortcuts in 2-column grid for compact professional layout (iteration 48) |
| Hero icon pill background | Done | Rounded pill with accent-tinted fill replaces raw halo glow for refined icon presentation (iteration 48) |
| Proportional font width calc | Done | Tab titles, sidebar names, status bar CWD use UI font advance (~25% more visible text) (iteration 49) |
| Tab hover lift effect | Done | Inactive tabs shift up 1.5px on hover for physical "raise" feel (iteration 49) |
| Smooth close button fade | Done | Close button uses smooth alpha (max of active_t, hover_t) instead of binary threshold (iteration 49) |
| Tab bar visual restraint | Done | Removed glass sheen band, bevel highlight, and shimmer from accent bar for clean flat aesthetic (iteration 50) |
| Flat sidebar surface | Done | Removed convexity gradient overlay for consistent flat surface matching Zed (iteration 50) |
| Flat agent panel | Done | PROCESSES panel uses inline items (no card container, shadow, inner shadow) matching sessions section style (iteration 50) |
| Compact agent items | Done | Single-line 36px agents, no task description or orbit animation — clean status dot + name + badge (iteration 50) |
| Element definition polish | Done | Stronger borders, shadows, gradients across card, icon, tabs, sidebar, status bar (iteration 51) |
| Hero icon legibility | Done | Monitor/caret strokes 2.0px, stand 1.6px at 65% opacity for confident rendering at 48px (iteration 51) |
| CTA button elevation | Done | Shadow offset 3px, blur 8px, alpha 0.32 for convincing floating button (iteration 51) |
| Active tab warmth | Done | 8% accent tint (was 5%), border alpha 0.85× (was 0.7×) for stronger tab-content connection (iteration 51) |
| Tab hover accent preview | Done | Faint accent-colored line at top of inactive tabs on hover, previews active state color (iteration 52) |
| Sidebar compact timestamps | Done | Timestamps on first line for compact items, second line only for items with descriptions (iteration 52) |
| Secondary text readability pass | Done | Breadcrumb, subtitle, shortcut descriptions, metadata, timestamps all boosted for consistent readability (iteration 53) |
| CTA button prominence | Done | Stronger accent fill (30%, was 18%), brighter label text, inner top highlight, deeper shadow for commanding primary action (iteration 54) |
| Sidebar active warmth | Done | Active session background 12% accent-tinted, stronger glow (0.10, was 0.08), warmer border (0.40×, was 0.35×) (iteration 54) |
| Status bar pill shadows | Done | Subtle drop shadows under all interactive pills (mode, CWD, git, dims, hints) for physical depth (iteration 54) |
| Tab depth consistency | Done | Inactive tabs get subtle shadows (0.08 alpha) for smooth depth transitions across tab states (iteration 55) |
| Tab icon/label alignment | Done | Process icon glyph matched to label size (both 13px) for vertical balance (iteration 55) |
| Inactive tab close visibility | Done | Close buttons at 25% text opacity on inactive tabs for discoverability (iteration 55) |
| Title bar gradient strength | Done | Gradient offset 0.012→0.020 for perceptible depth matching tab bar language (iteration 55) |
| Inactive tab rest borders | Done | Faint BORDER_VARIANT × 0.15 borders on inactive tabs for shape definition at rest (iteration 55) |
| Sidebar background flatness | Done | 4% bottom darkening (was 10%), matching Zed's flat sidebar surfaces (iteration 56) |
| Softened structural borders | Done | All panel borders reduced to 0.12 alpha — color difference provides primary separation (iteration 56) |
| Breadcrumb compactness | Done | 20px height (was 22px), subtle gradient (92/98%), softer separator (0.20 alpha) (iteration 56) |
| Window frame accent symmetry | Done | 2px bottom accent stripe matching top stripe for cohesive framing (iteration 56) |
| Bottom stripe edge fades | Done | Three-part gradient (fade-in → solid → fade-out) + focus dimming, matching top stripe treatment (iteration 57) |
| Content area spotlight strength | Done | Radial accent glow behind welcome content 1.2% → 2.0% for warmer ambient lighting (iteration 57) |
| Breadcrumb accent integration | Done | Last path segment pill uses 8% accent tint in background + 15% accent in border for color continuity (iteration 57) |
| Mode pill accent border | Done | Status bar mode pill border blends 15% active accent color for consistent accent language (iteration 57) |
| Top stripe edge fades | Done | Three-part gradient (fade-in → solid → fade-out) matching bottom stripe for symmetrical window framing (iteration 58) |
| Close button rest visibility | Done | Inactive tab close buttons at 0.18 alpha rest state for discoverability without hover (iteration 58) |
| Sidebar dot brightness | Done | Inactive session accent dots 0.62 alpha (was 0.50) for better visibility at rest (iteration 58) |
| Content area accent frame | Done | Subtle accent-tinted top-edge glow on content area connecting tab bar accent to content (iteration 58) |
| Empty status bar state | Done | Shows "Ready" dot + label and "Godly Terminal" branding when no terminal is focused (iteration 59) |
| Title bar icon accent tint | Done | Terminal icon blends 35% accent color for brand identity instead of plain muted (iteration 59) |
| Sidebar footer document icons | Done | Codicon file icons on "Project CLAUDE.md" and "User CLAUDE.md" buttons (iteration 59) |
| Active workspace accent bar glow | Done | Rightward accent-colored shadow (0.35 alpha, 6px blur) on active workspace indicator bar (iteration 59) |
| Full-width CTA button | Done | "Create terminal" button spans full card container width with 6px rounded corners (iteration 61) |
| Shortcut-CTA separator | Done | Thin hairline between shortcut grid and version/CTA section for visual clarity (iteration 61) |
| Sidebar session list fade | Done | Gradient fade at bottom of session list for smooth visual clipping (iteration 61) |
| Text contrast hierarchy | Done | Added FG_BRIGHT (#e6edf3) and FG_DIM (#555d6b) color levels matching web reference (iteration 62) |
| Active tab text brightness | Done | Active tab uses FG_BRIGHT (#e6edf3, was FG_PRIMARY #c9d1d9) for stronger active/inactive contrast (iteration 62) |
| Inactive tab dimming | Done | Inactive tabs use FG_DIM (#555d6b, was FG_MUTED #6e7681) for web-accurate contrast (iteration 62) |
| Sidebar session name brightness | Done | Session names use FG_PRIMARY at rest (was FG_SECONDARY), FG_BRIGHT on hover, WHITE on active (iteration 62) |
| Sidebar item border radius | Done | Session items use 6px radius (was 4px) matching web reference border-radius (iteration 62) |

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
