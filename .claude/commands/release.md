Merge all open PRs, update the changelog, and bump the version.

Accepts an optional argument: `major`, `minor` (default), or `patch`.

## Steps

### 1. Merge all open PRs

1. Run `gh pr list --state open` to see all open PRs.
2. Run `gh pr status` to check CI status for each.
3. If any PR has failing checks, **stop and report** — do not merge PRs with failing CI.
4. Merge each PR in order (oldest first) using `gh pr merge <number> --merge --delete-branch`.
5. If the current branch conflicts during a merge (local changes), stash first with `git stash`, then retry.
6. After all PRs are merged, switch to master and pull: `git checkout master && git pull`.

### 2. Update the changelog

1. Read the current `CHANGELOG.md`.
2. Gather the full list of merged PRs since the last release tag using `gh pr list --state merged` and `git log`.
3. Under a new `## [X.Y.Z] - YYYY-MM-DD` section (using today's date and the new version), organize changes into **Keep a Changelog** categories:
   - **Added** — new features, capabilities, tools
   - **Fixed** — bug fixes
   - **Changed** — behavioral changes, refactors, optimizations
   - **Removed** — removed features or deprecated code (if any)
4. Each entry should be a single concise line referencing the PR number(s).
5. Group related PRs into a single entry when they form a cohesive feature (e.g., multiple scrollback PRs become one entry).
6. Bold the most significant entries.
7. Keep the `## [Unreleased]` section empty above the new release.

### 3. Bump the version

1. Read the current version from `src-tauri/tauri.conf.json` (canonical source).
2. Bump the requested component:
   - `major`: 0.5.0 -> 1.0.0
   - `minor`: 0.5.0 -> 0.6.0 (default)
   - `patch`: 0.5.0 -> 0.5.1
3. Update the version string in all three files:
   - `src-tauri/tauri.conf.json` (`.version` field)
   - `package.json` (`.version` field)
   - `src-tauri/Cargo.toml` (`[package]` section `version` field only — do NOT touch `[workspace.dependencies]`)

### 4. Commit and tag

1. Stage the changed files: `CHANGELOG.md`, `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`.
2. Commit: `chore: bump version to X.Y.Z`
3. Create annotated tag: `git tag -a vX.Y.Z -m "Release X.Y.Z"`
4. Push commit and tag: `git push origin master && git push origin vX.Y.Z`
5. Report: old version, new version, number of PRs merged, and tag name.

## Important

- Never merge a PR with failing CI checks.
- If no open PRs exist, skip step 1 and just do changelog + bump for already-merged work.
- The changelog should cover ALL changes since the previous release tag, not just the PRs merged in this run.
- Per PR policy, `chore:` commits go directly to master — no PR needed for the version bump.
