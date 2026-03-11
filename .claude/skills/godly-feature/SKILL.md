# Feature Skill

Orchestrate full feature development: GitHub issue, parallelizable plan, multi-agent execution (Claude Code or Codex), PR, and staging build. Use this whenever the user asks to implement a feature, add new functionality, or says `/feature`.

## Usage

```
/feature <feature-name> [description]
```

## Instructions

Follow all 8 phases in order. Do not skip phases.

---

### Phase 1: Issue & Kanban

Every feature must have a GitHub Issue and a kanban task before any code is written.

#### GitHub Issue

1. Check for a remote: `git remote -v`
2. Search existing issues:
   ```bash
   gh issue list --search "<feature keywords>" --state all --limit 10
   ```
3. If a **matching open issue** exists, read it (`gh issue view N`) and add a comment noting you're starting work.
4. If **no matching issue** exists, create one:
   ```bash
   gh issue create --title "Feature Request: <concise title>" --label "enhancement" --body "$(cat <<'EOF'
   ## Goal
   <what the feature should accomplish>

   ## Scope
   <what's included and what's explicitly excluded>

   ## Acceptance Criteria
   - [ ] <criterion 1>
   - [ ] <criterion 2>

   ## Affected Areas
   <which crates, frontend modules, tests will be touched>
   EOF
   )"
   ```
5. Save the issue number for the branch name and PR.

#### Kanban Board

1. Create a task on `board_id: godly-terminal`:
   - **title**: `Add <feature-name>` (convention: `Action - <key-identifier>`)
   - **priority**: `medium`
2. Move the task to `in_progress` immediately.

### Phase 2: Codebase Analysis & Parallelizable Plan

This is the critical planning phase. The goal is to produce a plan that maximizes parallel agent execution.

#### 2a. Analyze affected areas

Use the Explore agent or direct Glob/Grep/Read to understand:

- Which **Rust crates** will be modified (protocol, daemon, vt, tauri app)
- Which **frontend modules** will be modified (components, store, services, styles)
- Which **test tiers** are needed (unit, browser, integration, daemon, crate, e2e)
- What **new files** need to be created vs which existing files get modified

#### 2b. Identify independent work units

Split the feature into work units that can run in parallel. Each work unit MUST:
- Have a **clear, non-overlapping file scope** (list every file it will touch)
- Be **independently testable** (can verify success without other units)
- Produce a **working commit** when complete

Use the crate dependency graph to identify parallel-safe boundaries:

| Parallel-safe | Why |
|--------------|-----|
| Frontend (`src/`) + any Rust crate | Different languages, no file overlap |
| `godly-mcp` + `godly-notify` | Independent crates |
| `godly-remote` + `godly-llm` | Independent crates |
| Different store domain files | `store-workspace.ts` vs `store-terminal.ts` |

| Needs sequencing | Why |
|-----------------|-----|
| `godly-protocol` then dependents | Shared types must land first |
| `daemon/server.rs` + `daemon/session.rs` | Tightly coupled |
| `commands/` + `daemon_client/` | Shared imports |

#### 2c. Design the work split

For each work unit, produce:

```
Work Unit N: <title>
  Scope: <files this unit will modify/create>
  Dependencies: <which other units must complete first, or "none">
  Engine: <suggest "claude" or "codex" — see criteria below>
  Prompt: <the exact task prompt to send to the agent>
  Verification: <command to verify this unit works>
```

**Engine suggestion criteria:**
- **Claude Code** (default): Multi-file changes, architecture decisions, complex refactoring, test writing, anything requiring deep codebase understanding
- **Codex**: Isolated single-crate changes, well-scoped mechanical tasks, tasks with a clear spec and no ambiguity

#### 2d. Present the plan

Present a summary table to the user:

```
Feature: <name> (#<issue>)

Work Units:
┌───┬─────────────────────┬────────────────┬─────────┬───────────┐
│ # │ Title               │ Scope          │ Depends │ Engine    │
├───┼─────────────────────┼────────────────┼─────────┼───────────┤
│ 1 │ Protocol types      │ protocol/      │ none    │ codex     │
│ 2 │ Daemon handler      │ daemon/        │ 1       │ claude    │
│ 3 │ Frontend UI         │ src/components │ none    │ claude    │
│ 4 │ Test suite          │ tests/         │ 1,2,3   │ claude    │
└───┴─────────────────────┴────────────────┴─────────┴───────────┘

Parallelism: Units 1+3 run simultaneously. Unit 2 waits for 1. Unit 4 waits for all.
```

### Phase 3: Agent Engine Selection

Ask the user to confirm or change the engine for each work unit using AskUserQuestion:

> For each work unit, which engine should run it?

Options per unit:
- **Claude Code** — deep codebase reasoning, multi-file edits
- **Codex** — fast, sandboxed, good for mechanical tasks

Also ask if the user wants to adjust the plan (reorder, merge, or split units).

After confirmation, proceed to spawning.

### Phase 4: Spawn Agents

Get the current workspace from godly-terminal MCP, then spawn agents in parallel.

#### 4a. Spawn terminals with worktrees

For each work unit that has no unmet dependencies:

```
# Use quick_claude for Claude Code agents
mcp__godly-terminal__quick_claude(
  workspace_id: <current>,
  prompt: <work unit prompt>,
  branch_name: "feat/<issue>-<unit-slug>"
)

# OR use codex MCP for Codex agents
# First create a terminal with worktree, then run codex in it
mcp__godly-terminal__create_terminal(
  workspace_id: <current>,
  worktree_name: "feat-<issue>-<unit-slug>"
)
# Then use mcp__codex-cli__codex with the worktree's working directory
```

#### 4b. Rename terminals for visibility

```
mcp__godly-terminal__rename_terminal(terminal_id, "Agent N: <unit-title>")
```

#### 4c. Create kanban sub-tasks

For each work unit, create a sub-task on the kanban board linking to the parent task.

### Phase 5: Monitor & Coordinate

#### 5a. Monitor progress

Periodically check each agent terminal:

```
mcp__godly-terminal__read_terminal(terminal_id, { mode: "tail", lines: 30, strip_ansi: true })
```

Look for:
- Completion signals (agent went idle, committed changes)
- Error signals (test failures, compilation errors)
- Agent asking questions (needs intervention)

#### 5b. Handle dependencies

When a dependency unit completes:
1. Verify its work (read the terminal output for test results)
2. Spawn the next dependent unit's agent

#### 5c. Handle failures

If an agent fails:
- Read the full error output
- Decide: retry with adjusted prompt, fix manually, or ask the user
- Do NOT let a failing agent block others indefinitely

### Phase 6: Merge & Verify

After all agents complete:

#### 6a. Merge worktree branches

Switch to the feature branch and merge each worktree branch:

```bash
git checkout feat/<issue>-<short-description>
# Merge each unit's branch
git merge wt-feat-<issue>-<unit-slug> --no-ff -m "merge: <unit-title>"
```

Resolve conflicts if needed. If conflicts are complex, read both versions and choose the correct resolution.

#### 6b. Run verification

Run the local verification checks from CLAUDE.md:

1. `cd src-tauri && cargo check -p <modified-crates>`
2. `cd src-tauri && cargo nextest run -p <modified-crates>`
3. `pnpm test` (if TypeScript was touched)
4. `pnpm test:browser` (if Canvas2D/layout was touched)
5. `pnpm build:daemon && pnpm test:integration` (if daemon protocol was touched)

Fix any failures. This may require additional edits to resolve integration issues between work units.

#### 6c. Clean up worktrees

```bash
git worktree remove <path>
git branch -d <branch>
```

### Phase 7: Changelog, Commit & PR

#### Changelog fragment

Create `changelog/unreleased/<PR-number>-<short-desc>.md`:

```markdown
### Added
- **<Feature name>** - <description> ([#<PR>](https://github.com/<org>/<repo>/pull/<PR>))
```

#### Commit

Make atomic commits per logical change. Use `feat:` prefix:

```bash
git add -A
git commit -m "feat: <concise description of the feature>"
```

#### Push & PR

```bash
git push -u origin feat/<issue>-<short-description>
gh pr create --title "feat: <feature title>" --body "$(cat <<'EOF'
## Summary
<1-3 bullet points describing what was added>

Fixes #<issue-number>

## Test Plan
- [ ] Unit tests pass (`pnpm test`)
- [ ] Cargo tests pass (`cargo nextest run -p <crates>`)
- [ ] <additional verification steps>

## Work Units
<list the work units and which engine (Claude/Codex) implemented each>
EOF
)"
```

#### Update tracking

1. Comment on the GitHub issue summarizing what shipped
2. Move the kanban task to `done`

### Phase 8: Build Staging

Ask the user if they want to build Godly Staging to test the feature:

> Feature is ready. Want to build Godly Staging to test it?

If yes, run in background:

```bash
powershell.exe -ExecutionPolicy Bypass -File scripts/staging-build.ps1
```

When the build completes, install silently:

```bash
powershell.exe -Command "Start-Process -FilePath 'installations/staging/Godly Terminal (Staging)_*_x64-setup.exe' -ArgumentList '/S' -Wait"
```

Then notify the user: "Staging is installed. Open 'Godly Terminal (Staging)' from the Start Menu to test."

Optionally offer to run `/manual-testing <feature>` on the staging build.

---

## Quick Reference: Files by Area

| Area | Frontend | Backend |
|------|----------|---------|
| Terminal I/O | `TerminalPane.ts`, `terminal-service.ts` | `commands/terminal.rs`, `daemon/src/server.rs` |
| Workspaces | `WorkspaceSidebar.ts`, `workspace-service.ts` | `commands/workspace.rs` |
| Tabs | `TabBar.ts` | N/A (frontend only) |
| State | `state/store.ts` | `state/app_state.rs`, `state/models.rs` |
| Persistence | N/A | `persistence/layout.rs`, `persistence/scrollback.rs` |
| Keyboard | `state/keybinding-store.ts` | N/A |
| Settings | `components/SettingsDialog.ts` | N/A |
| Events | `listen()` from `@tauri-apps/api/event` | `app_handle.emit()` |

## Engine Comparison

| Dimension | Claude Code | Codex |
|-----------|------------|-------|
| Best for | Complex multi-file, architecture decisions, test writing | Isolated mechanical tasks, clear spec |
| Speed | Slower (deeper reasoning) | Faster (focused execution) |
| Codebase awareness | Full project context via CLAUDE.md | Working directory context |
| Spawning | `quick_claude` (godly-terminal MCP) | `mcp__codex-cli__codex` or terminal + CLI |
| Sandbox | No sandbox (full access) | Configurable sandbox (`workspace-write`) |
