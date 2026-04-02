Codex Ralph Loop iteration 1.

Repository root: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal
Run artifacts directory: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\.codex-ralph-loop\20260402-122617
Web reference screenshot target: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\web-reference.png
Native screenshot target: C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.png
No previous iteration final message exists yet.

You are Codex running in an autonomous long-horizon parity loop for godly-shell.

The target is not "good enough". The target is real parity with the visual quality of web/godly-terminal.jsx and the screenshot in docs/references/web-reference.png.

The loop is effectively unlimited. Optimize for the final native result, not for tiny isolated edits that leave the real blockers untouched.

Read first:
- AGENTS.md
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

Preferred repo-native tooling:
- Native screenshot helper: scripts/take-screenshot-now.ps1
- Pixel inspection helper: scripts/check-pixels.ps1
- Iteration log: tasks/rendering-quality-iterations.md
- Gap tracker: docs/references/gaps.md

Iteration workflow:
1. Audit current state and choose the highest-leverage next task.
2. Build or improve the missing automation or harness you need if quality work is under-instrumented.
3. Refresh the web reference screenshot if needed:
   - Use the Vite app in web/
   - Prefer pnpm over npm
   - Use browser tooling to capture the real reference at 1920x1080
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
