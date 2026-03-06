Merge all open PRs into master, resolve conflicts, consolidate changelogs, and bump the version.

Accepts an optional argument: `major`, `minor` (default), or `patch`.

## Overview

This command handles the full release merge workflow: fetch all open PRs, merge them one-by-one into master (resolving conflicts locally), consolidate changelog fragments, and finish with `/bump-version`. It parallelizes reconnaissance but merges sequentially since each merge changes the base.

## Phase 1: Reconnaissance (parallel)

Gather intel on all open PRs before touching anything.

1. Switch to master and pull latest: `git checkout master && git pull origin master`
2. Run `gh pr list --state open --json number,title,headRefName,mergeable,statusCheckRollup,additions,deletions,files,labels` to get all open PRs with metadata.
3. If no open PRs, report "No open PRs to merge" and skip to Phase 5.
4. **For each PR in parallel** (use subagents or batch `gh` calls):
   a. Check CI status — any PR with failing checks gets flagged. Ask the user whether to skip it or abort entirely.
   b. Run `gh pr diff <number>` to capture the full diff.
   c. Identify changed files: group by crate/directory to predict conflict zones.
   d. Check if the PR has changelog fragments in `changelog/unreleased/` (look at the diff for files matching `changelog/unreleased/*.md`).
   e. Read the PR description for context on what it does (needed for conflict resolution).
5. Print a summary table:

```
PR   | Title                        | Files | Conflicts? | CI  | Changelog?
#101 | feat: add search bar         | 12    | likely     | ✓   | ✓
#102 | fix: scroll snap             | 3     | unlikely   | ✓   | ✓
#103 | refactor: extract handlers   | 8     | likely     | ✓   | ✗
```

6. Ask the user to confirm the merge set. They can exclude specific PRs.

## Phase 2: Plan merge order

Order matters — merge the simplest/most isolated PRs first to build a stable base, then tackle the ones with more overlap.

1. **Sort PRs by conflict risk** (lowest first):
   - PRs touching unique files (no overlap with other PRs) go first
   - PRs touching shared files go later
   - Within the same risk tier, merge oldest first (lower PR number)
2. **Identify conflict clusters** — groups of PRs that touch the same files. Log these so you know where to focus during resolution.
3. Print the planned merge order and conflict predictions. No need to ask for confirmation — just inform.

## Phase 3: Sequential merge with conflict resolution

This is the core — merge each PR locally into master, resolving conflicts as they arise.

For each PR in the planned order:

### 3a. Attempt the merge

```bash
git fetch origin pull/<number>/head:pr-<number>
git merge pr-<number> --no-ff -m "Merge pull request #<number> from <branch>: <title>"
```

### 3b. If merge succeeds cleanly

Great — move on to validation (3d).

### 3c. If merge conflicts

This is the hard part. Resolve conflicts by keeping **both functionalities**:

1. Run `git diff --name-only --diff-filter=U` to list conflicted files.
2. For each conflicted file:
   a. Read the file with conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`).
   b. Understand what **both sides** are doing by reading the PR descriptions and diffs from Phase 1.
   c. Resolve by **integrating both changes**, not by picking one side:
      - If both sides add different items to a list/array/enum → keep all items
      - If both sides modify the same function → merge the logic, combining both behaviors
      - If one side renames/moves code that the other side modifies → apply the modification to the renamed/moved version
      - If both sides add imports → keep all imports (deduplicate)
      - For `Cargo.toml` dependency sections → keep all added dependencies
      - For `lib.rs` or `mod.rs` handler registrations → keep all registrations
      - For changelog fragments → these are separate files, rarely conflict, but if they do, keep both entries
   d. After editing, `git add <file>`.
3. After all conflicts resolved: `git commit --no-edit` (uses the merge commit message).

**If a conflict is too complex to resolve confidently:**
- Do NOT guess. Stage what you can, then ask the user to review the remaining conflicts.
- Show them the conflict with context from both PRs so they can decide.

### 3d. Quick validation after each merge

Run a fast check to catch integration issues early — don't wait until the end:

```bash
cd src-tauri && cargo check -p <affected-crates>
```

Only check crates whose files were touched by this PR (from Phase 1 data). If the check fails:
1. Read the error.
2. If it's a straightforward integration issue (missing import, type mismatch from concurrent changes), fix it.
3. Amend the merge commit: `git add . && git commit --amend --no-edit`
4. If the fix isn't obvious, ask the user.

### 3e. Close the PR on GitHub

After a successful local merge and push:

```bash
# The PR will auto-close when its commits are in master after push.
# But explicitly mark it to keep things clean:
gh pr comment <number> --body "Merged locally as part of batch-pr release workflow."
```

### 3f. Repeat for next PR

Continue through the merge order. Each subsequent merge builds on top of the previous ones, which is why order matters.

## Phase 4: Push and verify

After all PRs are merged locally:

1. Run TypeScript check if any `.ts`/`.js` files changed:
   ```bash
   npm test
   ```
2. Run Rust check for all modified crates:
   ```bash
   cd src-tauri && cargo check -p <all-affected-crates>
   ```
3. If checks pass, push to master:
   ```bash
   git push origin master
   ```
   The push will auto-close all merged PRs on GitHub (their commits are now in master).
4. If checks fail, fix the issues, amend the last merge commit, and retry.

## Phase 5: Consolidate changelog

After all merges are on master:

1. Read all `.md` files from `changelog/unreleased/` (skip `.gitkeep` and `TEMPLATE.md`).
2. If PRs that were just merged added new fragments, they'll now be in the working tree.
3. Also check: some PRs may reference `(#PR)` placeholder in their fragments — replace with the actual PR number.
4. The changelog fragments are already in the right format. `/bump-version` will handle merging them into `CHANGELOG.md`.
5. If any merged PR was missing a changelog fragment, create one now:
   - Name: `<PR-number>-<short-description>.md`
   - Content: appropriate section (`### Added` for feat, `### Fixed` for fix, etc.) with a one-liner from the PR title/description.

## Phase 6: Bump version

Run `/bump-version <patch|minor|major>` (using the argument passed to this command, defaulting to `minor`).

This will:
- Bump version across all crates and config files
- Collect and merge changelog fragments into `CHANGELOG.md`
- Delete fragment files
- Build verification
- Commit, tag, and optionally push

## Error handling

- **CI failures**: Never merge a PR with failing CI. Report it and ask the user.
- **Merge conflicts you can't resolve**: Show the conflict with both sides' context and ask.
- **Cargo check fails after merge**: Try to fix obvious integration issues. If not obvious, ask.
- **A PR was force-pushed during the process**: Re-fetch and retry that PR's merge.

## Important rules

- Always verify the PR author before merging — only merge PRs by Alan (alangmartini). Flag any others for manual review.
- Never force-push to master.
- The merge commits preserve full PR history (use `--no-ff`).
- Keep changelog entries concise — one line per logical change, reference the PR number.
- Per PR policy, the version bump commit goes directly to master (no PR needed for `chore:` changes).
