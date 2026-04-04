# Stateful Ralph Loop - Autonomous Improvement Engine

You are running inside a Ralph Loop. The same prompt is fed to you every iteration.
Your memory between iterations is the **state file** at `.ralph-state/STATE.md`.

**Read `.ralph-state/STATE.md` FIRST before doing anything else.**
**Then read `.ralph-state/GOAL.md`** — it contains the user's goal, reference source, and key context.

If the state file has no active Major Loop (only History or empty), you are in the DISCOVER phase.
Otherwise, parse the current Major Loop status to determine your phase.

---

## Project Context

You are working on `godly-shell` — a native terminal app built with **raw winit + wgpu** (no CSS, no Iced, no web tech). All UI rendering is manual Rust code:
- **Quads/rectangles**: `ui/quad_renderer.rs` + `ui/builder.rs`
- **Text**: `terminal-surface` crate with DirectWrite rasterization
- **Layout**: Taffy (flexbox-like) in `ui/layout.rs`, `ui/sidebar_layout.rs`
- **Colors/styling**: constants in `ui/builder.rs`
- **Components**: `ui/sidebar.rs`, `ui/tab_bar.rs`, `ui/status_bar.rs`

The **web reference** is `web/godly-terminal.jsx` — a React component with inline CSS styles that defines what the native UI should look like. Read its CSS values (colors, padding, border-radius, font-sizes, etc.) and translate them into the equivalent Rust/wgpu rendering code.

Key docs to check: `docs/references/gaps.md`, `tasks/rendering-quality-iterations.md`, `CLAUDE.md`, `AGENTS.md`.

---

## State Machine

### Phase 1: DISCOVER (no active Major Loop)

1. Read CLAUDE.md, `.ralph-state/GOAL.md`, and `docs/references/gaps.md`.
2. Read `web/godly-terminal.jsx` to understand the target styling.
3. Build and run godly-shell. Take a screenshot of current state.
4. Read the reference images from GOAL.md (use Read tool on the image paths).
5. Compare current native rendering against the web reference. Identify the **single highest-leverage visual gap** to close next.
6. Write the state file with the new Major Loop:

```markdown
# Ralph State

## Major Loop #N
- **Description**: [Specific visual gap to close]
- **Status**: PLANNING
- **Rationale**: [Why this is the highest-leverage gap right now]
- **Web Reference**: [Relevant CSS/style values from godly-terminal.jsx]
- **Started**: [ISO timestamp]

## Minor Loops
<!-- Not yet planned -->

## History
[preserve existing history]
```

7. **Stop here.** Exit. Next iteration will plan the minor loops.

---

### Phase 2: PLAN_MINOR (Major Loop status is PLANNING)

1. Re-read the state file and GOAL.md.
2. Read the relevant source files (both web reference JSX and native Rust code).
3. Break the Major Loop into **numbered minor loops** (ordered by execution dependency). Each should be a self-contained, implementable unit.
4. Update the state file:

```markdown
## Minor Loops

### Minor 1: [Title]
- **Description**: [Specific implementation task — include target values from JSX]
- **Status**: PENDING
- **Files**: [Expected files to touch]

### Minor 2: [Title]
- ...
```

5. Update the Major Loop status to `IN_PROGRESS`.
6. **Stop here.** Exit.

---

### Phase 3: IMPLEMENT (Major Loop is IN_PROGRESS, PENDING/IN_PROGRESS minor loops exist)

1. Re-read the state file. Find the **first** minor loop with status `PENDING` or `IN_PROGRESS`.
2. If `IN_PROGRESS`, it was interrupted — read its notes to pick up where it left off.
3. Implement the minor loop:
   - Read the relevant native Rust files AND the web JSX reference for target values.
   - Make the code changes to bring native rendering closer to web reference.
   - Run `cargo check -p godly-shell` to verify compilation.
   - Run `cargo nextest run -p godly-shell` if there are relevant tests.
4. Update the state file:
   - Set status to `COMPLETE` (or keep `IN_PROGRESS` with `**Remaining**` notes if unfinished).
   - Add `**Notes**` describing what was done.
5. **Stop here.** Exit. Next iteration handles the next minor loop or validation.

**IMPORTANT**: Only work on ONE minor loop per iteration.

---

### Phase 4: VALIDATE (Major Loop is IN_PROGRESS, ALL minor loops are COMPLETE)

1. Build: `cd src-tauri && cargo check -p godly-shell`
2. Test: `cd src-tauri && cargo nextest run -p godly-shell` (if tests exist)
3. Run godly-shell and take a screenshot. Compare against reference.
4. If it builds and tests pass:
   - Commit all changes following CLAUDE.md git workflow.
   - Update Major Loop status to `COMPLETE` with summary.
   - Move to `## History` section. Clear `## Minor Loops`.
5. If validation fails:
   - Add fix minor loops and set Major Loop back to `IN_PROGRESS`.
6. **Stop here.** Exit. Next iteration starts a fresh DISCOVER.

---

## State File Rules

- The state file is your ONLY memory. Write everything important there.
- Always preserve the `## History` section.
- Be specific in notes — next iteration has no memory of what you did.
- Number major loops sequentially across the full History.
- Use ISO timestamps.

## Decision Rules

- **DISCOVER**: No active Major Loop (state missing, empty, or all in History).
- **PLAN_MINOR**: Major Loop status is `PLANNING`.
- **IMPLEMENT**: Major Loop is `IN_PROGRESS` AND `PENDING`/`IN_PROGRESS` minor loops exist.
- **VALIDATE**: Major Loop is `IN_PROGRESS` AND ALL minor loops are `COMPLETE`.

## Quality Standards

- Each minor loop must produce code that compiles (`cargo check -p godly-shell`).
- Each major loop must produce a working, tested improvement.
- Never commit broken code.
- Follow existing patterns. Read how similar things are already done before adding new code.
- When translating CSS to Rust: extract exact values (colors as hex/rgb, sizes in px, etc.) from the JSX.

## What NOT to do

- Don't skip reading the state file.
- Don't work on multiple minor loops in one iteration.
- Don't forget to update the state file before exiting.
- Don't make changes to `iced-shell` — we're working on `godly-shell` only.
- Don't ignore previous iteration's notes about failures or remaining work.
- Don't guess at styling values — read them from `web/godly-terminal.jsx`.
