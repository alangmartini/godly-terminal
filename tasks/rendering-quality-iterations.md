# UI Chrome Quality Iterations

## Iteration 1: Remove fake hover effects

**Problem**: Sidebar session hover states had three ugly effects:
1. `fill_shadow` — colored glow halo around hovered items (cheap-looking)
2. `fill_rounded_gradient` — top-to-bottom gradient on hover background (fake 3D)
3. `stroke_rounded` — thin border stroke adding to overdesigned look

Text also had visible background color boxes (ClearType compositing artifact)
because `item_bg` used `BG_HOVER` while the actual drawn fill was a gradient.

**Fix** (sidebar.rs):
- Replaced all three effects with a single `fill_rounded` using `BG_SURFACE` at `hover_t` opacity
- Updated `item_bg` to use `BG_SURFACE` (matching the actual drawn background) so ClearType compositing has no color fringing
- Removed unused `lerp` import

**Fix** (tab_bar.rs):
- Close button: replaced glow + gradient + stroke with flat `fill_rounded`
- New-tab button: replaced gradient + stroke with flat `fill_rounded`
- Window buttons (min/max/close): replaced gradient + stroke with flat `fill_rounded`
- Removed unused `lerp` import

**Result**: All hover states are now clean flat fills matching the web reference's simple `backgroundColor` transitions. No shadows, no gradients, no borders on hover.

## Iteration 2: Agent panel layout parity + cleanup

**Problem**: Agent panel had several layout mismatches vs reference:
1. Status badge placed next to agent name instead of right-aligned
2. Descriptions truncated with ellipsis instead of word-wrapped
3. Fixed 48px item height regardless of description length
4. Description indentation was icon-relative (~35px) instead of web's fixed 20px
5. `session_accent` colors leaked into hover effects (removed but variable was dead)
6. Agent hover used `BG_HOVER` instead of `BG_SURFACE` (inconsistent with session fix)

**Fix** (sidebar.rs):
- Status badge now right-aligned before × dismiss button (web: `marginLeft: "auto"`)
- Description text now word-wraps using `wrap_ui_text_lines()` helper
- Agent item heights are dynamic per item based on wrapped description line count
- Description uses `paddingLeft: 20px` from item edge (matching web CSS)
- Agent hover fills use `BG_SURFACE` (consistent with session hover fix)
- Removed dead `SESSION_ACCENTS` constant and `session_accent` variable
- Added `wrap_ui_text()` (line count) and `wrap_ui_text_lines()` (actual lines) helpers
- Fixed separator check to use `ai + 1 < len` instead of `std::ptr::eq`

**Result**: Agent panel matches web reference density and layout.
