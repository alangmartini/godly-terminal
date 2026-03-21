### Added
- **Worktree Mode for workspaces** — when enabled via sidebar context menu, new terminals automatically create isolated git worktrees in `.godly-worktrees/` and open in those directories. Closing a worktree terminal prompts to remove or keep the worktree on disk. Worktree paths persist across app restarts and `.godly-worktrees/` is auto-added to `.gitignore` on first use.
