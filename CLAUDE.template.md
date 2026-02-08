# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Install dependencies
# TODO: Add your install command (e.g., npm install, pip install -r requirements.txt, cargo build)

# Development mode
# TODO: Add your dev command (e.g., npm run dev, cargo run)

# Production build
# TODO: Add your build command (e.g., npm run build, cargo build --release)

# Type checking (if applicable)
# TODO: Add your type check command (e.g., npx tsc --noEmit, mypy .)

# Run tests
# TODO: Add your test command (e.g., npm test, cargo test, pytest)
```

## Git Workflow

Always commit all staged and unstaged changes when making a commit. Do not leave uncommitted changes behind.

Never add "Generated with Claude Code" or any similar attribution message to commits, PRs, or any other output.

## Bug Fix Workflow

When the user pastes a bug report or describes a bug:

1. **Write a test first** that reproduces the bug. The test must fail, confirming the bug exists.
2. **Run the test** to verify it actually fails as expected (red phase).
3. **Fix the bug** by modifying the source code.
4. **Run the test again** and loop until all tests pass (green phase).
5. Continue with the standard verification requirements below (full build + all tests).

Do NOT skip the reproduction step. The test must fail before you start fixing.

### Test Quality Standards

- Tests must be **specific** enough that passing them means the bug is actually fixed, not just that something changed.
- Each test should assert the **exact expected behavior**, not just "no error."
- Regression tests should include the original bug trigger as a comment (e.g., `// Bug: connection dropped when payload exceeded 1MB`).

## Feature Development Workflow

When adding a new feature:

1. **Implement the feature** in the source code.
2. **Write tests** covering the feature's key behaviors.
3. **Run the tests** and loop until all pass.
4. Continue with the standard verification requirements below (full build + all tests).

Do NOT consider a feature complete without accompanying tests.

## Parallel Agent Workflow

When multiple Claude instances work simultaneously (e.g., in git worktrees):

### Task Claiming

Before starting work, create a file `current_tasks/<branch-name>.md` describing the task scope and files likely to be modified. Check existing files in `current_tasks/` first to avoid overlap. Remove the file when the PR is merged.

### Branch Naming

Use descriptive branch names: `wt-<scope>` (e.g., `wt-fix-auth`, `wt-feat-search`). Avoid generic names.

### Staying in Sync

Pull and rebase from the main branch before opening a PR. If another agent's PR merges first, rebase on top of it before pushing.

### Scope Boundaries

Each agent should own a clearly scoped task. Avoid modifying the same files as another active agent. If overlap is unavoidable, coordinate via smaller, more frequent commits and PRs.

### Task Scoping

Each agent should receive a single, well-defined task when launched. Good tasks have clear boundaries:
- "Implement search in the settings page" (one feature, known files)
- "Write tests for the auth module" (one module, test-only changes)
- "Refactor database connection pooling" (one concern, contained scope)

Avoid giving one agent a broad task like "improve the codebase" — it will collide with other agents. The narrower the task, the fewer merge conflicts.

### Self-Orientation

Each agent starts fresh with no prior context. Support orientation with:
- This CLAUDE.md file (kept up to date)
- Progress notes in `current_tasks/` files
- Clear commit messages that explain *why*, not just *what*

## Output Hygiene

Rules to keep context windows clean during long agent sessions:

- **Run targeted tests first**: When working on a specific module, run just that module's tests before the full suite.
- **Summarize failures**: When tests fail, identify the root cause and state it concisely rather than pasting full stack traces.
- **Avoid verbose flags**: Don't use `--verbose`, `--nocapture`, or similar flags unless actively debugging a specific test.
- **Incremental verification**: Check compilation before running tests. Check one module before all modules.

### Clean Test Output

Tests should produce minimal, parseable output — not walls of text that pollute the context window.

- **Minimal on success**: A passing test suite should print a summary line (e.g., `45 passed, 0 failed`), not per-test details.
- **Structured on failure**: Failed tests should print a single-line error identifier (e.g., `FAIL: test_name — expected X, got Y`) followed by the relevant assertion, not the entire backtrace.
- **Log to files, not stdout**: When debugging requires verbose output, redirect to a file and read it selectively rather than flooding the terminal.
- **Grep-friendly format**: Error messages should be self-contained on one line so they can be found with `grep FAIL` or `grep ERROR`.
- **Pre-compute summaries**: When running large test suites, use summary/terse reporters to get aggregate pass/fail counts without per-test noise.

## Verification Requirements

**IMPORTANT**: After making any code changes, always verify the project builds and tests pass before considering work complete. Loop until all checks pass:

1. **Run all tests**:
   ```bash
   # TODO: Add your full test command(s)
   ```

2. **Verify production build**:
   ```bash
   # TODO: Add your build command
   ```

3. If any step fails, fix the issues and repeat until everything passes.

This catches:
- Compilation / type errors
- Test failures
- Configuration errors

## Architecture Overview

<!-- TODO: Replace this section with your project's architecture -->

Describe your project's high-level architecture here. Include:

### Stack
- **Frontend**: (e.g., React, Vue, vanilla JS)
- **Backend**: (e.g., Rust, Node.js, Python)
- **Build**: (e.g., Vite, Webpack, Cargo)

### Project Structure

```
# TODO: Add your project's directory structure
# Example:
# src/
#   components/   ← UI components
#   services/     ← Business logic / API calls
#   state/        ← State management
# tests/          ← Test suites
```

### Key Data Flows

<!-- TODO: Describe how data moves through your system -->
<!-- Example: User input → Service layer → API → Database → Response → UI update -->

## Key Patterns

<!-- TODO: Add your project's common development patterns -->

### Adding a new [component/endpoint/module]

<!-- Example:
1. Create the file in `src/components/`
2. Register it in `src/routes.ts`
3. Add tests in `tests/`
-->

<!-- ============================================================
TEMPLATE NOTES (delete this section when using):

This template encodes practices from real-world parallel Claude agent
workflows. Key principles:

1. ENVIRONMENT > CAPABILITY: Agent success depends more on test quality,
   output hygiene, and task coordination than on model capability.

2. TESTS ARE THE VERIFIER: When agents work autonomously, tests are the
   only reliable way to know if work is correct. Invest in test quality.

3. CONTEXT IS FINITE: Every line of noisy output wastes tokens the agent
   could use for reasoning. Keep test output minimal and structured.

4. FILE-BASED COORDINATION: Simple file locks (current_tasks/) beat
   complex orchestration. Git's merge mechanics handle conflicts.

5. INCREMENTAL VERIFICATION: Check compilation before tests, one module
   before all modules, targeted tests before full suites.

6. SELF-ORIENTATION: Each agent starts fresh. Good documentation,
   progress files, and clear commit history help agents orient fast.

References:
- https://www.anthropic.com/engineering/building-c-compiler
============================================================ -->
