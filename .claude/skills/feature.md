# Feature Skill

Develop a new feature end-to-end using TDD (Red-Green-Refactor), with full issue tracking, documentation, and changelog management. Use this skill whenever the user asks to implement a feature, add new functionality, or says `/feature`. It covers the complete lifecycle: GitHub issue, kanban board, branch creation, TDD implementation, documentation, changelog entry, and PR.

## Usage

```
/feature <feature-name> [description]
```

## Instructions

Follow all 8 phases in order. Do not skip phases — each one produces artifacts the next phase depends on.

---

### Phase 1: Issue Tracking

Every feature must have a GitHub issue before any code is written. This creates a searchable history and links all related work.

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
   <which parts of the codebase will be touched>
   EOF
   )"
   ```
5. Save the issue number — you'll need it for the branch name and PR.

### Phase 2: Kanban Board

Create a task on the kanban board and move it to in-progress:

1. Create a task:
   - **board_id**: `godly-terminal`
   - **title**: `Add <feature-name>` (follow the naming convention: `Action - <key-identifier>`)
   - **priority**: `medium` (default for features)
2. Move the task to `in_progress` immediately.
3. If you discover sub-tasks during implementation, create them on the same board.

### Phase 3: Branch Creation

Create a feature branch using the issue number:

```bash
git checkout -b feat/<issue-number>-<short-description>
```

Example: `feat/234-split-terminal-zoom`

### Phase 4: Test Tier Selection

Before writing any code, decide which test tiers are needed. Use this decision tree:

| Feature involves... | Test tier | Why |
|---------------------|-----------|-----|
| Store logic, keyboard shortcuts, event routing | **Unit** (`npm test`) | Pure logic, no DOM needed |
| Canvas2D rendering, layout, pointer events | **Browser** (`npm run test:browser`) | Needs real Chromium |
| Daemon protocol, session lifecycle, IPC | **Integration** (`npm run test:integration`) | Needs real daemon |
| Full user workflow, persistence across restart | **E2E** (`npm run test:e2e`) | Needs full app |
| New daemon command, concurrency, ring buffers | **Daemon** (`cargo nextest run -p godly-daemon`) | Needs isolated daemon |
| New VT sequences, parser changes | **Crate** (`cargo nextest run -p godly-vt`) | Rust unit tests |

Most features need at least **unit tests** + one higher-tier test. If the feature spans frontend and backend, use multiple tiers.

### Phase 5: TDD — Red-Green-Refactor

This is the core implementation loop. Follow it strictly.

#### Red Phase — Write Failing Tests First

Write tests that describe the feature's expected behavior. Run them to confirm they **fail** — the feature doesn't exist yet, so they must fail. If they pass, the tests aren't testing the right thing.

Guidelines:
- Test the **behavior**, not the implementation. Assert on what the user sees or what the system does, not internal data structures.
- Cover the key scenarios: happy path, edge cases, error conditions.
- Each test should have a clear name that describes the expected behavior (e.g., `"should zoom in when Ctrl+= is pressed"`).
- Post a progress comment on the GitHub issue: "Tests written, all failing as expected (Red phase complete)."

#### Green Phase — Make Tests Pass

Implement the feature with the **simplest code that makes all tests pass**. Don't optimize, don't refactor, don't add anything beyond what's needed to go green.

Follow the architecture patterns in CLAUDE.md:
- **New Tauri command**: handler in `src-tauri/src/commands/` → register in `lib.rs` → TypeScript wrapper in `src/services/`
- **New daemon command**: protocol variant in `protocol/src/messages.rs` → handler in `daemon/src/server.rs` → client method in `src/daemon_client/client.rs` → Tauri command wrapper
- **New UI component**: class in `src/components/` → state in `src/state/store.ts` → styles in `src/styles/main.css`
- **New keyboard shortcut**: add to `DEFAULT_SHORTCUTS` in `src/state/keybinding-store.ts` → add to Settings dialog categories

Run tests after each significant change. The goal is to see them go from red to green one by one.

#### Refactor Phase — Clean Up While Green

Now improve the code without changing behavior:
- Remove duplication
- Improve naming
- Simplify logic
- Extract functions if genuinely needed (not prematurely)

Run tests after **each refactor step**. If any test turns red, undo the last change and try a different approach.

### Phase 6: Verification

Run the local verification checks required by CLAUDE.md:

1. **Cargo check** (if Rust was touched):
   ```bash
   cd src-tauri && cargo check -p <crate-you-modified>
   ```

2. **Rust tests** (changed crates only):
   ```bash
   cd src-tauri && cargo nextest run -p <crate-you-modified>
   ```
   Or use the smart runner: `npm run test:smart`

3. **Frontend unit tests** (if TypeScript was touched):
   ```bash
   npm test
   ```

4. **Browser tests** (if Canvas2D, layout, or pointer events were touched):
   ```bash
   npm run test:browser
   ```

5. **Integration tests** (if daemon protocol or session lifecycle was touched):
   ```bash
   npm run build:daemon && npm run test:integration
   ```

All checks must pass before proceeding. Fix any failures and re-run.

### Phase 7: Documentation & Changelog

#### Documentation

1. **Create or update docs** in `docs/` if the feature is architecturally significant, introduces new concepts, or has a non-obvious usage pattern. Not every feature needs a doc — use judgment.

2. **Update README.md** if the feature:
   - Changes how users install, build, or run the project
   - Adds a new user-facing command, shortcut, or capability
   - Modifies the architecture overview
   - Adds a new build/test command

3. **Update CLAUDE.md** if the feature:
   - Introduces a new architectural pattern other developers should follow
   - Adds new test commands or verification steps
   - Changes the daemon command chain or IPC protocol

#### Changelog

Add an entry to `CHANGELOG.md` under `## [Unreleased]`:

1. Read the current changelog to match the style.
2. Place the entry in the correct **Keep a Changelog** category:
   - **Added** — new features (this is almost always the right one for `/feature`)
   - **Changed** — if you modified existing behavior
   - **Fixed** — if the feature also fixes a known bug
3. Format: `- **<Feature name>** — <concise description> (#<issue-number>)`
4. Bold the entry if it's a significant user-facing addition.

Example:
```markdown
## [Unreleased]

### Added
- **Terminal zoom** — keyboard (Ctrl+=/−) and Ctrl+scroll zoom in/out (#300)
```

The version number gets filled in later by the `/release` or `/bump-version` skill when a release happens.

### Phase 8: Commit & PR

1. **Commit** with conventional format. Use atomic commits — one commit per logical change:
   ```
   feat: <concise description of the feature>
   ```

2. **Push and open a PR** referencing the tracking issue:
   ```bash
   git push -u origin feat/<issue-number>-<short-description>
   gh pr create --title "feat: <feature title>" --body "$(cat <<'EOF'
   ## Summary
   <1-3 bullet points describing what was added>

   Fixes #<issue-number>

   ## Test Plan
   - [ ] Unit tests pass (`npm test`)
   - [ ] Cargo tests pass (`cargo nextest run -p <crate>`)
   - [ ] <any additional verification steps>
   EOF
   )"
   ```
   Use `Fixes #N` if the PR fully delivers the feature, or `Refs #N` for incremental PRs.

3. **Update the GitHub issue** with a final comment summarizing what shipped.

4. **Move the kanban task** to `done`.

5. **Ask the user** if they'd like you to run user-like testing via `/manual-testing <feature>` on Godly Staging.

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
