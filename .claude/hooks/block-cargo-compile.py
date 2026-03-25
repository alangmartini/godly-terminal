"""PreToolUse hook: block cargo compilation commands.

Agents must not compile locally (cold worktree = full rebuild, 10-20 agents = machine death).
Only cargo fmt is allowed. Everything else goes through CI/CD.
"""
import json
import re
import sys

BLOCKED_SUBCOMMANDS = r"cargo\s+(build|check|test|nextest|bench|clippy|run|install|add|tauri)\b"

data = json.load(sys.stdin)
cmd = data.get("tool_input", {}).get("command", "")

# Strip heredocs, single-quoted strings, and double-quoted strings so that
# commit messages or echo statements mentioning "cargo build" don't trigger.
cleaned = re.sub(r"<<'?(\w+)'?\n.*?\n\1", "", cmd, flags=re.DOTALL)
cleaned = re.sub(r"'[^']*'", "", cleaned)
cleaned = re.sub(r'"(?:[^"\\]|\\.)*"', "", cleaned)

if re.search(BLOCKED_SUBCOMMANDS, cleaned):
    print(
        json.dumps(
            {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": (
                        "BLOCKED: cargo compilation commands are delegated to CI/CD. "
                        "Only 'cargo fmt' is allowed locally. "
                        "Edit code and push your branch - CI validates."
                    ),
                }
            }
        )
    )
