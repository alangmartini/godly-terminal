# Ralph State

## Major Loop #56
- **Description**: Switch dark-background text blending from ClearType gamma 1.8 to sRGB gamma 2.2 for browser-matching compositing
- **Status**: IN_PROGRESS
- **Rationale**: The dark-background text path in atlas_shader.rs linearizes fg/bg colors with gamma 1.8 (ClearType standard), but browsers use sRGB gamma 2.2. This causes the native output to be `pow(blended, 1/1.8)` vs browser's `pow(blended, 1/2.2)`. Since 1/1.8 > 1/2.2, mid-coverage pixels are brighter in native, producing slightly thicker-looking text on dark backgrounds. Switching the dark-bg path to sRGB gamma would match browser text compositing exactly.
- **Web Reference**: Browsers use sRGB gamma (2.2) for text linearization and compositing
- **Native Current**: Dark-bg path at atlas_shader.rs:98-109 uses `pow(color, 1.8)` linearization and `pow(blended, 2.2/1.8)` output conversion
- **Started**: 2026-04-04T15:15:00Z

## Minor Loops

### Minor 1: Change dark-bg linearization and output to sRGB gamma 2.2
- **Description**: In atlas_shader.rs WGSL, modify the dark-bg branch (lum < 0.5) to: (1) linearize fg/bg with `pow(color, 2.2)` instead of `pow(color, GAMMA)` i.e. 1.8, (2) output `blended` directly instead of `pow(blended, 2.2/1.8)` — the sRGB render target auto-applies pow(x, 1/2.2), so outputting linear-sRGB blended values produces correct sRGB output. Leave the light-bg ClearType path unchanged (it still uses gamma 1.8 which is correct for ClearType subpixel rendering).
- **Status**: COMPLETE
- **Files**: `src-tauri/native/terminal-surface/src/atlas_shader.rs`
- **Notes**: Restructured the opaque-bg path: moved luminance check before linearization. Dark-bg branch now uses `pow(color, 2.2)` for linearization and outputs `blended` directly (linear sRGB, render target applies 1/2.2). Light-bg branch still uses gamma 1.8 ClearType path. `cargo check -p godly-shell` passes.

### Minor 2: Build verification and test
- **Description**: Run `cargo check -p godly-shell` and `cargo nextest run -p godly-shell` to verify compilation and test pass.
- **Status**: PENDING
- **Files**: N/A (verification only)

## History

### Major Loop #55: Use grayscale AA on dark opaque backgrounds matching browser behavior
- **Status**: COMPLETE
- **Completed**: 2026-04-04T15:00:00Z
- **Summary**: Added luminance-based grayscale AA in atlas_shader.rs for dark backgrounds (lum < 0.5). On dark backgrounds, ClearType per-channel coverage is collapsed to a single grayscale value, eliminating subpixel color fringing and matching browser behavior. ClearType remains for light backgrounds. Build clean, 19 tests pass. Committed as 980963c.

### Major Loop #54: Reduce ENHANCED_CONTRAST from 0.5 to 0.0
- **Status**: COMPLETE
- **Completed**: 2026-04-04T14:30:00Z
- **Summary**: Removed enhanced contrast boost entirely (0.5 → 0.0) in atlas_shader.rs. The enhance() function is now identity, producing text weight matching browser rendering (no shader-side coverage boost). Build clean, 19 tests pass. Committed as beb84c3.

### Major Loop #53: Reduce ENHANCED_CONTRAST from 1.0 to 0.5
- **Status**: COMPLETE
- **Completed**: 2026-04-04T14:10:00Z
- **Summary**: Reduced atlas shader ENHANCED_CONTRAST from 1.0 to 0.5 (DirectWrite's default) in atlas_shader.rs. The enhance() formula now applies half the previous boost to glyph coverage, producing text weight closer to browser rendering. Updated comment to explain DirectWrite default rationale. Build clean, 19 tests pass. Committed as 952050c.

### Major Loop #52: Switch DirectWrite rendering mode to NATURAL
- **Status**: COMPLETE
- **Completed**: 2026-04-04T13:35:00Z
- **Summary**: Changed DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC to DWRITE_RENDERING_MODE_NATURAL in directwrite_rasterizer.rs for sharper text matching browser rasterization. Committed as 9f346a3.

### Major Loop #51: Fix right panel width
- **Status**: COMPLETE (NO-OP)
- **Completed**: 2026-04-04T13:25:00Z
- **Summary**: No mismatch — both web and native use 380px.

### Major Loop #50: Comprehensive CSS value audit
- **Status**: COMPLETE (NO-OP)
- **Completed**: 2026-04-04T13:15:00Z
- **Summary**: All CSS values match. Remaining gap is inherent platform typography.

### Major Loop #49: Add half-leading offsets to paragraph and bullet text
- **Status**: COMPLETE
- **Completed**: 2026-04-04T12:50:00Z
- **Summary**: Added CSS half-leading offsets. Committed as d0a69b7.

### Major Loop #48-#1: Earlier improvements
- **Status**: ALL COMPLETE
