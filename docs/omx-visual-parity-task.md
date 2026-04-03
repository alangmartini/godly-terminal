# OMX Visual Parity Task

Use this task contract when driving `godly-shell` toward visual parity with the web reference through `omx exec`, `omx ralph`, or `omx team`.

## Goal

Achieve real visual parity between the native `godly-shell` crop scene and the web reference in `docs/references/web-reference.png`.

The target is not "close enough". The target is parity across:

- transcript typography and compositing
- sidebar/session stack spacing and hierarchy
- tab-strip typography and badge placement
- retained layout discipline instead of handwritten rectangle drift
- measurable screenshot-diff evidence from the repo-native harness

## Read First

- `AGENTS.md`
- `docs/references/gaps.md`
- `tasks/rendering-quality-iterations.md`
- `docs/superpowers/plans/2026-03-29-drop-iced-migration.md`
- `docs/superpowers/plans/2026-03-27-directwrite-cleartype-rendering.md`
- `scripts/measure-godly-shell-parity.ps1`
- `scripts/capture-web-reference.ps1`
- `scripts/check-pixels.ps1`

## Hard Gates

Do not declare completion while any of these remain materially unresolved:

1. Windows presentation must stay physically sharp.
2. Chrome text must use a real typography and measurement path.
3. Chrome text compositing must remain background-aware where technically valid.
4. Inner shell layout must continue moving toward retained layout, not deeper handwritten coordinate math.
5. Parity claims must be backed by the screenshot harness, not vibes.

## Required Workflow

1. Audit `docs/references/gaps.md` and pick the highest-leverage unresolved gap.
2. Run the parity harness before editing when the current visual state is unclear:
   `powershell -ExecutionPolicy Bypass -File scripts/measure-godly-shell-parity.ps1`
3. If reference/native/diff screenshots are attached to the run, invoke `$visual-verdict` on them before the next edit and turn that verdict into a concrete next step.
4. Prefer architectural fixes over tiny color nudges when the bigger blocker is still open.
5. Keep `docs/references/gaps.md` and `tasks/rendering-quality-iterations.md` honest and specific after meaningful progress.
6. Run lightweight verification only: targeted `cargo check`, targeted tests, and the parity scripts.

## Preferred Tooling

- Web reference refresh: `scripts/capture-web-reference.ps1`
- Native capture + diff: `scripts/measure-godly-shell-parity.ps1`
- Window capture helper: `scripts/take-screenshot-now.ps1`
- Pixel metrics: `scripts/check-pixels.ps1`

## Team Mode Staffing

When using `omx team`, prefer three lanes:

- Lane 1: transcript typography and compositing
- Lane 2: sidebar or retained-layout migration
- Lane 3: parity measurement, evidence, and gap-log updates

Keep one lane responsible for regression evidence so the team does not optimize blindly.

## Completion Contract

- Do not claim parity while `docs/references/gaps.md` still lists material critical or major gaps.
- Do not hide uncertainty in the docs or final report.
- Leave the repo buildable.
