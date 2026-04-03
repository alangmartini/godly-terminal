### Added

- **SDF shader clip rectangle support** - Added `clip_rect` field to `QuadVertex` for per-fragment clipping with smooth 1px anti-aliased edges. `UiBuilder` exposes `set_clip(rect)` / `clear_clip()` API for CSS `overflow: hidden` semantics on scrollable containers.
