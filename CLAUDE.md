# CLAUDE.md


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
Architecture docs, design specs, and testing guides stay in `docs/` — only investigation/bug tracking uses GitHub Issues.


