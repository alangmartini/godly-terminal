# PRD — godly-shell visual quality uplift

## Problem
`godly-shell` still looks materially cheaper than the reference embedded in `web/godly-terminal.jsx`, especially in the deterministic reference-mode screenshot used for native parity work. The user explicitly wants the native shell quality lifted to at least the reference minimum.

## Goal
Ship a focused visual-quality pass that improves the most obvious remaining low-quality traits in `godly-shell`, prioritizing the parity-scene transcript/chrome work that most strongly affects perceived polish.

## Non-goals
- Replacing the overall product direction.
- Adding new dependencies.
- Full redesign of unrelated Tauri/web shells.

## User stories
1. As a user, I want the native shell transcript to look crisp, intentional, and browser-grade so the main content no longer reads as cheap or placeholder-quality.
2. As a user, I want surrounding chrome spacing/typography to feel coherent with the reference so the overall shell looks deliberate rather than stitched together.
3. As a maintainer, I want screenshot-harness evidence for the pass so quality claims are measurable.

## Acceptance criteria
- The parity scene is visually closer to the `web/godly-terminal.jsx` reference in the most obvious remaining problem areas.
- `scripts/measure-godly-shell-parity.ps1` shows a non-regressing, preferably improved result after the change.
- Targeted `cargo check -p godly-shell` passes.
- Relevant targeted tests for touched layout/visual modules pass.
- Gap log / iteration log reflect the current state honestly.
