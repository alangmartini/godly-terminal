# Context Snapshot — godly-shell visual quality

## Task statement
Improve the visual quality of `godly-shell` so it reaches at least the minimum quality shown in `web/godly-terminal.jsx`.

## Desired outcome
- Native `godly-shell` looks materially closer to the JSX reference, especially in the main transcript/text rendering and surrounding chrome quality.
- The reference/parity path remains deterministic enough to measure with the existing screenshot diff tooling.
- Changes stay small, reversible, and limited to the native shell / terminal-surface touchpoints required for the visual fix.

## Known facts / evidence
- The user explicitly called `$ralph`, so the Ralph verification loop applies.
- Repo guidance enforces a ralplan-first gate for Ralph until `.omx/plans/prd-*.md` and `.omx/plans/test-spec-*.md` exist.
- `web/godly-terminal.jsx` is the minimum acceptable design target.
- Existing parity artifacts already exist under `docs/references/`, including:
  - `web-reference.png`
  - `current-godly-shell.png`
  - `current-godly-shell.diff.png`
  - `gaps.md`
- `docs/references/gaps.md` says the biggest remaining quality issue is transcript typography/compositing, with secondary gaps in sidebar/session spacing and some tab typography drift.
- There are already uncommitted local edits in:
  - `src-tauri/native/godly-shell/src/main.rs`
  - `src-tauri/native/godly-shell/src/terminal_renderer.rs`
  - `src-tauri/native/terminal-surface/src/atlas_shader.rs`
  - `src-tauri/native/terminal-surface/src/directwrite_rasterizer.rs`
  These appear to be an in-progress experiment for grayscale UI monospace rasterization in reference-crop mode and must be preserved.

## Constraints
- Respect all existing uncommitted changes; do not overwrite user/worktree edits.
- No new dependencies.
- Verification should stay lightweight and targeted locally.
- Because this is a visual quality task, completion requires fresh evidence from build/tests and screenshot/parity-related verification where practical.

## Unknowns / open questions
- Whether the existing grayscale-mono experiment is already sufficient once verified, or still needs tuning.
- Whether the remaining mismatch is primarily shader compositing, glyph raster output, text layout metrics, or a mix.
- Whether any sidebar/tab polish is still needed after fixing transcript quality.

## Likely codebase touchpoints
- `src-tauri/native/godly-shell/src/main.rs`
- `src-tauri/native/godly-shell/src/terminal_renderer.rs`
- `src-tauri/native/terminal-surface/src/atlas_shader.rs`
- `src-tauri/native/terminal-surface/src/directwrite_rasterizer.rs`
- Possibly reference-only UI files if final parity evidence shows layout drift:
  - `src-tauri/native/godly-shell/src/ui/reference_pane.rs`
  - `src-tauri/native/godly-shell/src/ui/sidebar.rs`
  - `src-tauri/native/godly-shell/src/ui/sidebar_layout.rs`
