# MCP Testing Procedure

When asked to test the godly-terminal MCP, use the MCP tools directly from Claude Code. The MCP binary (`godly-mcp`) exposes 15 tools via JSON-RPC over stdio, proxied through the Tauri app via named pipe IPC.

## Test Sequence

Run these tests in order. Each phase builds on the previous one. Clean up all test artifacts (terminals, workspaces, worktrees) when done.

**Phase 1 — Read-only queries (no side effects):**
1. `get_current_terminal` → expect `{id, name, process_name, workspace_id}`
2. `list_terminals` → expect array of terminal objects
3. `list_workspaces` → expect array of workspace objects
4. `get_notification_status` (no params) → expect `{enabled, source: "global"}`

**Phase 2 — Notifications:**
5. `notify` with `message` → expect `{success: true}`, verify chime plays
6. `set_notification_enabled` with `terminal_id` + `enabled: false` → expect success
7. `get_notification_status` with `terminal_id` → expect `{enabled: false, source: "terminal"}`
8. `set_notification_enabled` with `terminal_id` + `enabled: true` → re-enable
9. Repeat steps 6-8 for `workspace_id` instead of `terminal_id`

**Phase 3 — Terminal CRUD:**
10. `create_terminal` (basic, just `workspace_id`) → expect `{id, success: true}`
11. `create_terminal` with `cwd` param → expect success
12. `create_terminal` with `command` param → expect success, then `read_terminal` to verify command output appears
13. `create_terminal` with `worktree: true` → expect `{id, worktree_path, worktree_branch}`
14. `create_terminal` with `worktree_name` → expect custom branch name in response
15. `rename_terminal` → rename a test terminal, verify via `list_terminals`
16. `focus_terminal` → expect success (visual confirmation needed — see gaps)
17. `write_to_terminal` → send `echo "MARKER"`, then `read_terminal` to verify
18. `read_terminal` with `mode: "tail"` → expect terminal content
19. `read_terminal` with `mode: "head"` → expect terminal content
20. `read_terminal` with `mode: "full"` → expect terminal content
21. `read_terminal` with `filename` param → expect file written to disk
22. `close_terminal` → close all test terminals

**Phase 4 — Workspace operations:**
23. `create_workspace` with `name` + `folder_path` → expect `{id, success: true}`
24. `switch_workspace` to new workspace → expect success
25. `move_terminal_to_workspace` → move a terminal, verify via `list_terminals`
26. `switch_workspace` back to original → expect success
27. Clean up: close test terminals, remove worktrees via git CLI

**Phase 5 — Error handling:**
28. `write_to_terminal` with invalid ID → expect error (daemon validates: "Session not found")
29. `read_terminal` with invalid ID → expect error (daemon validates: "Session not found")
30. `close_terminal` with invalid ID → **BUG: returns `{success: true}` silently**
31. `switch_workspace` with invalid ID → **BUG: returns `{success: true}` silently**
32. `rename_terminal` with invalid ID → **BUG: returns `{success: true}` silently**
33. `focus_terminal` with invalid ID → **BUG: returns `{success: true}` silently**
34. `move_terminal_to_workspace` with invalid ID → **BUG: returns `{success: true}` silently**

Note: Operations routed through the daemon (`write_to_terminal`, `read_terminal`) properly validate IDs.
Operations handled by Tauri app state (`close`, `switch`, `rename`, `focus`, `move`) silently succeed with invalid IDs — they need validation added.

## Cleanup Checklist

After testing, ensure:
- [ ] All test terminals are closed
- [ ] All test worktrees are removed (`git worktree remove` + `git branch -d`)
- [ ] Test workspace still exists (no `delete_workspace` tool — manual cleanup needed)

## Known Gaps (cannot test via MCP alone)

| Gap | Description | Suggested MCP Tool |
|-----|-------------|-------------------|
| No `delete_workspace` | Can create but not delete workspaces; leaves orphans | `delete_workspace` |
| No `delete_worktree` | Worktrees from `create_terminal` need manual git cleanup | `delete_worktree` or auto-cleanup on `close_terminal` |
| No `get_active_workspace` | Cannot verify `switch_workspace` actually changed the UI | `get_active_workspace` |
| No `get_active_terminal` | Cannot verify `focus_terminal` actually switched the tab | `get_active_terminal` |
| No plain-text `read_terminal` | Output contains raw ANSI escapes, hard to parse programmatically | Add `strip_ansi: true` param to `read_terminal` |
| No `get_terminal_cwd` | Cannot verify `cwd` param on `create_terminal` worked | `get_terminal_cwd` or include cwd in terminal info |
| No `resize_terminal` via MCP | The daemon supports resize but MCP doesn't expose it | `resize_terminal` |
| Silent success on invalid IDs | `close`, `switch_workspace`, `rename`, `focus`, `move` return `{success: true}` for nonexistent IDs | Add ID validation in Tauri MCP handler before dispatching |
| No error case testing docs | Error format inconsistent between daemon-routed and Tauri-routed tools | Standardize error responses across all tools |
