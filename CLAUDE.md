# CLAUDE.md

## Disabled Skills

- **Never invoke `superpowers:brainstorming`**. For focused, well-scoped changes, just do the work. For genuinely ambiguous large-scope work, ask one clarifying question at most, then proceed.

## Git Workflow

Always commit all staged and unstaged changes when making a commit. Do not leave uncommitted changes behind.

Never add "Generated with Claude Code" or any similar attribution message to commits, PRs, or any other output.

- Split work into atomic commits by change type/scope (`feat`, `fix`, `docs`, `test`, etc.) so each commit represents one logical unit.
- Push branches and open/update PRs frequently for each atomic unit instead of batching multiple unrelated changes behind one large PR.

### Changelog Fragments

Every `feat:` and `fix:` commit must include a changelog fragment file in `changelog/unreleased/`.

- **Naming**: `<PR-number>-<short-description>.md` (e.g., `425-fix-scroll-snap.md`). If no PR yet, use the branch name.
- **Format**: One or more [Keep a Changelog](https://keepachangelog.com/) sections (`### Added`, `### Fixed`, `### Changed`, `### Removed`, `### Tests`).
- **Content**: Bold title + dash + description + PR reference. See `changelog/TEMPLATE.md`.
- **When**: Create the fragment as part of the same commit that introduces the change.
- **Who collects**: `/bump-version` merges fragments into `CHANGELOG.md` and deletes them at release time.
- `chore:`, `docs:`, `style:`, `refactor:`, `test:` commits do NOT need fragments unless they represent user-facing changes.

### PR Policy

- **All change types (`feat:`, `fix:`, `docs:`, `test:`, `chore:`, `style:`, `refactor:`)**: Create a branch, open a PR, and merge via PR review flow.
- **No direct pushes to `master`** for routine implementation work.

## Debugging Principles

- **Never mask errors.** Don't add retry loops, fallback handlers, or auto-recovery that hides the root cause of a crash or failure. If something crashes, the priority is understanding WHY — not papering over it so the user doesn't notice.
- **Preserve crash evidence.** Logs must survive process restarts. Never truncate logs on startup. Use append mode and rotate old logs so the previous run's crash info is always available for post-mortem.

## Issue Investigation Tracking

Track all bugs and investigations as **GitHub Issues**, not local docs.

### When starting a bug investigation:
1. Search existing issues: `gh issue list --search "<keywords>" --state all --limit 10`
2. If a matching closed issue exists, read it (`gh issue view N`) — the bug may have regressed
3. Create a new issue or reopen the existing one with appropriate labels (`bug`, `performance`, `daemon`, `frontend`, `mcp`, `ux`)
4. Comment on the issue with each approach tried, including what failed and why

### During investigation:
- Add a comment for each significant attempt (what you tried, result, why it failed/succeeded)
- Include relevant code snippets, test commands, and root cause analysis in comments
- Use the issue body for the canonical summary (symptom, root cause, fix)

### When resolved:
- Reference the issue in the PR description with `fixes #N` (GitHub auto-closes on merge)
- Add a final comment with regression risk assessment and relevant test commands

### Reference docs
## Workflow Orchestration
### 1. Plan Mode Default
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity\

### 2. Subagent Strategy
- Use subagents liberally to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One task per subagent for focused execution

### 3. Self-Improvement Loop
- After ANY correction from the user: update tasks lessons.md with the pattern
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevan project

### 4. Verification Before Done
- Never mark a task complete without proving it works
- Diff behavior between main and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

### 5. Demand Elegance (Balanced)
- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant solution"
- Skip this for simple, obvious fixes -- don't over-engineer
- Challenge your own work before presenting it

### 6. Autonomous Bug Fixing
- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests -- then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

## Task Management

1. Plan First: Write plan to tasks/todo.md with checkable items
2. Verify Plan: Check in before starting implementation
3. Track Progress: Mark items complete as you go
4. Explain Changes: High-level summary at each step
5. Document Results: Add review section to tasks/todo.md
6. Capture Lessons: Update tasks/lessons.md after corrections

## Core Principles

- Simplicity First: Make every change as simple as possible. Impact minimal code.
- No Laziness: Find root causes. No temporary fixes. Senior developer standards.
- Minimal Impact: Only touch what's necessary. No side effects with new bugs.

## Stateful Ralph Loop (Visual Parity)

An autonomous loop that iteratively closes the visual gap between `godly-shell` (native winit+wgpu) and `web/godly-terminal.jsx` (the web reference).

### How it works
- **State machine** in `.ralph-state/STATE.md` — survives cancellations and usage limits.
- **Prompt** in `RALPH_STATEFUL_PROMPT.md` — same prompt fed every iteration; Claude reads the state file to know what phase it's in.
- **Goal** in `.ralph-state/GOAL.md` — the target (visual parity with the web reference).
- **Phases**: DISCOVER → PLAN_MINOR → IMPLEMENT (one minor per iteration) → VALIDATE → loop.

### How to manage it (as an overseer)
```bash
# Launch in background
cd /c/Users/User/godly-terminal
nohup bash ./ralph-stateful-loop.sh >> /c/Users/User/godly-terminal/.ralph-state/loop-output.log 2>&1 &
LOOP_PID=$!
echo "$LOOP_PID" > /c/Users/User/godly-terminal/.ralph-state/loop.pid

# Monitor (check every 5-7 min)
ps -p $(cat .ralph-state/loop.pid) -o pid= 2>/dev/null && echo "alive" || echo "dead"
head -10 .ralph-state/STATE.md                    # current major loop
grep -E "^### Minor|Status" .ralph-state/STATE.md # minor loop progress
git log --oneline -5                              # commits made
tail -30 .ralph-state/loop-output.log             # recent output

# Restart after crash/usage limit (state persists, picks up where it left off)
nohup bash ./ralph-stateful-loop.sh >> /c/Users/User/godly-terminal/.ralph-state/loop-output.log 2>&1 &
LOOP_PID=$!
echo "$LOOP_PID" > /c/Users/User/godly-terminal/.ralph-state/loop.pid
```

### Known behaviors
- The process dies between iterations due to `claude --print` exit codes or API limits. This is expected — just restart.
- Each restart picks up exactly where it left off via the state file.
- The loop uses `set -uo pipefail` (no `set -e`) so non-zero exits from Claude don't kill the bash loop.
- All work targets `godly-shell` only (NOT `godly-iced-shell`).

