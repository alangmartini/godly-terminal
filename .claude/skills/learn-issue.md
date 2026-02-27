# Learn Issue

Analyze the current conversation to detect bugs, UX friction, or missing features encountered during this session in **Godly Terminal itself**, then create GitHub issues for approved findings.

## Usage

```
/learn-issue
```

## Instructions

Run through all 5 phases below, in order.

### Phase 1: Scan the Conversation

Review the full conversation history. Identify things that went wrong or could be improved **in Godly Terminal itself** — not in the user's code, external tools, or Claude's behavior.

For each finding, categorize it:

| Category | Label | Examples |
|----------|-------|---------|
| Bug | `bug` | Crash, wrong behavior, data loss, error that shouldn't happen |
| UX friction | `ux` | Confusing workflow, missing feedback, slow operation, poor error message |
| Missing feature | `enhancement` | Capability that would have helped but doesn't exist yet |

Collect each finding as:
- **Title**: Concise issue title (imperative, e.g., "Fix worktree parameter in quick_claude MCP tool")
- **Category**: `bug`, `ux`, or `enhancement`
- **Symptom**: What happened or what was missing
- **Context**: Relevant conversation details (error messages, workarounds used, etc.)

Skip anything that:
- Is about Claude Code itself (not Godly Terminal)
- Is about the user's project code
- Was already filed as an issue during this session
- Is a deliberate design decision the user confirmed

### Phase 2: Deduplicate Against Existing Issues

For each finding from Phase 1:

1. Search existing GitHub issues:
   ```bash
   gh issue list --search "<keywords from title>" --state all --limit 5
   ```
2. If a **matching open issue** exists → mark as `SKIP (open #N)`
3. If a **matching closed issue** exists → mark as `POSSIBLE REGRESSION (closed #N)` — the bug may have come back
4. If **no match** → mark as `NEW`

### Phase 3: Present Findings

Show the user a summary table:

```
Findings from this session:
┌───┬──────────────────────────────────┬─────────────┬─────────────────────┐
│ # │ Title                            │ Category    │ Status              │
├───┼──────────────────────────────────┼─────────────┼─────────────────────┤
│ 1 │ Fix worktree param in quick_...  │ bug         │ NEW                 │
│ 2 │ Add session export command       │ enhancement │ SKIP (open #234)    │
│ 3 │ Improve error msg for pipe...    │ ux          │ POSSIBLE REGRESSION │
└───┴──────────────────────────────────┴─────────────┴─────────────────────┘
```

Ask the user which findings to file (by number). Default: all `NEW` items. `SKIP` items are excluded by default. `POSSIBLE REGRESSION` items will reopen the closed issue instead of creating a new one.

### Phase 4: File Issues

For each approved finding, create a GitHub issue:

**For NEW issues:**
```bash
gh issue create \
  --title "<title>" \
  --label "<category>" \
  --body "$(cat <<'EOF'
## Symptom
<what happened or what's missing>

## Expected Behavior
<what should happen instead>

## Steps to Reproduce
<how to trigger this — be specific>

## Context
<relevant details from the conversation: error messages, workarounds, environment>

---
*Detected by `/learn-issue` from conversation context.*
EOF
)"
```

**For POSSIBLE REGRESSION issues:**
```bash
gh issue reopen <N>
gh issue comment <N> --body "$(cat <<'EOF'
## Possible Regression

This issue may have regressed. Encountered again during a session on $(date +%Y-%m-%d).

**Symptom observed:** <description>
**Context:** <what was happening when it occurred>

---
*Detected by `/learn-issue` from conversation context.*
EOF
)"
```

Apply additional labels where relevant: `daemon`, `frontend`, `mcp`, `performance`.

### Phase 5: Summary

Output a summary of what was filed:

```
Issues Created/Updated:
  - #456 Fix worktree param in quick_claude MCP tool (bug) — NEW
  - #234 Improve error message for pipe connection (ux) — REOPENED

Skipped:
  - "Add session export command" — already tracked in #234
```

Include clickable issue URLs so the user can review them.
