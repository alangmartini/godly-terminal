OMX Visual Parity Loop iteration 1.

Repository root: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal
Run artifacts directory: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\.omx-visual-parity-loop\20260402-105332
Web reference screenshot target: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\web-reference.png
Native screenshot target: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.png
Diff screenshot target: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.diff.png
No previous iteration final message exists yet.

You are running an autonomous long-horizon parity loop for godly-shell using oh-my-codex.

The target is not "good enough". The target is real visual parity with the web reference screenshot in docs/references/web-reference.png.

Use the shared task contract:
- docs/omx-visual-parity-task.md

Shared task contract:
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


Attached screenshots:
- C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\web-reference.png
- C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.png
- C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.diff.png

If any screenshots are attached, you must run $visual-verdict before the next edit and treat the verdict as the next action gate.
Use the screenshot diff as a secondary aid only.

Primary harness commands:
- powershell -ExecutionPolicy Bypass -File scripts/measure-godly-shell-parity.ps1
- powershell -ExecutionPolicy Bypass -File scripts/capture-web-reference.ps1
- powershell -ExecutionPolicy Bypass -File scripts/check-pixels.ps1

The loop is effectively unlimited. Optimize for the final native result, not for tiny isolated edits that leave the real blockers untouched.

Read first:
- AGENTS.md
- docs/omx-visual-parity-task.md
- docs/references/gaps.md
- tasks/rendering-quality-iterations.md
- docs/superpowers/plans/2026-03-29-drop-iced-migration.md
- docs/superpowers/plans/2026-03-27-directwrite-cleartype-rendering.md
- src-tauri/native/godly-shell/src/main.rs
- src-tauri/native/godly-shell/src/ui/layout.rs
- src-tauri/native/godly-shell/src/ui/builder.rs
- src-tauri/native/godly-shell/src/terminal_renderer.rs

Non-negotiable parity gates:
1. Windows presentation must be physical-pixel sharp, or you must implement and verify an objectively equivalent path.
2. UI chrome text must use a real typography/layout path. Do not treat synthetic italic, hand-tuned advance hacks, or pseudo-layout as finished.
3. Chrome text compositing must be background-aware so labels can achieve terminal-grade sharpness where technically valid.
4. Shell chrome layout must move toward a retained flex/layout layer (taffy or equivalent) instead of accumulating more manual rectangle math.
5. The repo must have a screenshot-diff or measurable visual-parity harness. Do not rely purely on vibes.

You must not emit RALPH_DONE while any of the above remain materially unresolved.

Current strategy priority:
1. Close the highest-leverage architectural blocker first.
2. Then close measurable visual gaps.
3. Only after those are solid should you spend iterations on micro-polish.

Important guidance:
- Do not artificially limit yourself to one tiny visual tweak if a deeper blocker spans multiple files.
- If a needed helper script or harness is missing, build it as part of the iteration.
- If docs/references/gaps.md understates architectural gaps, correct it.
- Use parallel agents if your environment supports them and it shortens the critical path, but do not delegate immediate blockers blindly.

Iteration workflow:
1. Audit current state and choose the highest-leverage next task.
2. Build or improve the missing automation or harness you need if quality work is under-instrumented.
3. Refresh the web reference screenshot if needed.
4. Build and run godly-shell, capture the native screenshot, and compare against the web reference.
5. Implement the code changes.
6. Run lightweight verification only. Prefer targeted cargo check, targeted tests, or script verification.
7. Rebuild and re-capture screenshots after the fix.
8. Update docs/references/gaps.md and tasks/rendering-quality-iterations.md with precise technical findings and remaining gaps.
9. Commit only if the result is meaningfully better and working. Follow AGENTS.md exactly:
   - commit all staged and unstaged changes in that commit
   - never leave partially related local changes behind
   - if the commit is feat: or fix:, include a changelog fragment in changelog/unreleased/
10. Leave the repo buildable before ending the iteration.

Hard rules:
- Do not settle for close enough.
- Do not spend the whole iteration on tiny color nudges while a higher-leverage parity blocker remains open.
- Do not revert unrelated user changes.
- Do not hide uncertainty in docs. Be explicit about what is still wrong.
- Do not claim parity just because the current screenshot looks better than the previous one.

Done criteria:
- The native shell is visually at parity with the web reference across text sharpness, layout discipline, spacing, chrome hierarchy, and presentation quality.
- docs/references/gaps.md no longer lists any material parity gap.
- The parity harness or checking path is good enough that future regressions are detectable.

At the end of your response:
- Output RALPH_DONE only if the done criteria are genuinely met.
- Otherwise end with a line starting exactly with: RALPH_CONTINUE:
  The value after the colon must be the next highest-leverage task.
