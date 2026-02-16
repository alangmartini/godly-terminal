# Slow Worktree Terminal Creation

## Symptom
Creating a new terminal with worktree mode enabled has a noticeable delay (2-10+ seconds depending on network conditions).

## Root Cause
`create_worktree()` in `src-tauri/src/worktree.rs` called `pull_latest()` which ran `git pull origin <branch>` before creating the worktree. This performed:

1. A full network fetch from the remote
2. A merge into the local default branch

The merge step was completely unnecessary since the worktree creates a new branch — it only needs up-to-date remote tracking refs, not a merged local branch.

Additionally, `commands/terminal.rs` made two sequential subprocess calls (`is_git_repo` + `get_repo_root`) where one sufficed.

## Fix
1. **Replaced `git pull` with `git fetch origin <branch> --no-tags`** — eliminates the merge step and `--no-tags` skips tag negotiation overhead.
2. **Branch worktree from `origin/<branch>` start point** — `git worktree add <path> -b <name> origin/<branch>` creates the worktree directly from the remote tracking ref, no local merge needed.
3. **Combined `is_git_repo` + `get_repo_root`** into a single `get_repo_root()` call (it already fails on non-git dirs).

## Files Changed
- `src-tauri/src/worktree.rs` — replaced `pull_latest()` with `fetch_latest()`, updated `create_worktree()` to use start point
- `src-tauri/src/commands/terminal.rs` — removed redundant `is_git_repo()` call

## Regression Risk
Low. The worktree still branches from the latest remote state. The only behavioral difference is that the local default branch is no longer merged as a side effect (which was unnecessary).
