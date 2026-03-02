### Fixed
- **MCP worktree workspace resolution** — `create_terminal` and `quick_claude` now use the user-supplied `workspace_id` to resolve the folder path for CWD and worktree operations, instead of silently overriding it with the Agent workspace UUID. Fixes misleading "Workspace not found" errors when using worktree parameters (refs #511)

### Changed
- **MCP tool descriptions** — Fixed inaccurate `create_terminal` description that claimed CWD defaults to home directory (it actually defaults to the workspace's folder path). Clarified `workspace_id` behavior in both `create_terminal` and `quick_claude` tool descriptions
