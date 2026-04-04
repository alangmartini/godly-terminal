# User Goal

Make godly-shell (pure winit + wgpu native renderer at `src-tauri/native/godly-shell/`) achieve visual parity with the web reference at `web/godly-terminal.jsx`.

This is hard because godly-shell uses raw winit without CSS — all layout, styling, text rendering, and compositing is done manually in Rust with wgpu shaders and DirectWrite. The web version has flexbox, CSS colors, border-radius, box-shadow, etc. for free.

## Reference Source
- **Web reference code**: `web/godly-terminal.jsx` — the JSX + inline styles define what the UI should look like
- **Web reference screenshot**: Run `cd web && pnpm dev` then screenshot at localhost:5199

## Reference Images
Claude: use your Read tool to view these images for visual comparison.
- `C:/Users/User/godly-terminal/.ralph-state/reference.png` (the TARGET - what we want to look like)
- `C:/Users/User/godly-terminal/.ralph-state/current.png` (the CURRENT state - what we have now)

## Key Context
- The native renderer is `godly-shell` (NOT `godly-iced-shell`)
- All rendering is manual: quads via `ui/quad_renderer.rs`, text via `terminal-surface` crate with DirectWrite
- Layout uses Taffy (flexbox-like) in `ui/layout.rs` / `ui/sidebar_layout.rs`
- Colors and styling constants live in `ui/builder.rs`
- Read `docs/references/gaps.md` and `tasks/rendering-quality-iterations.md` for known gaps
