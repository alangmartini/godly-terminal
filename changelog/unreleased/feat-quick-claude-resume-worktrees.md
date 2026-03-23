### Added
- **Quick Claude session resumption** — Claude Code sessions launched via Quick Claude now track session ID, CWD, and workspace, enabling resume on worktree context loss (feat/quick-claude-resume-worktrees)
- **Automatic session cleanup** — Stale Quick Claude session records are cleaned up on app startup (feat/quick-claude-resume-worktrees)

### Changed
- **Quick Claude preset launch** — Pre-assigns Claude session ID before launching to enable session tracking and resumption (feat/quick-claude-resume-worktrees)
- **Worktree session context** — Session records now include workspace ID and CWD, allowing Claude to resume in the original workspace even after worktree cleanup (feat/quick-claude-resume-worktrees)
