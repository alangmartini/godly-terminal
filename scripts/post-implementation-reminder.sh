#!/usr/bin/env bash
# Post-implementation skill reminder hook for Claude Code.
# Only prints reminders when there are uncommitted code changes (not just docs/config).

# Check for uncommitted code changes (staged + unstaged + untracked)
code_changes=$(git diff --name-only HEAD 2>/dev/null; git diff --name-only --cached 2>/dev/null; git ls-files --others --exclude-standard 2>/dev/null)

# Filter to actual code files (not docs, config, etc.)
code_files=$(echo "$code_changes" | grep -E '\.(rs|ts|js|tsx|jsx|css|html)$' | sort -u)

if [ -z "$code_files" ]; then
  exit 0  # No code changes, stay silent
fi

# Count changed files
file_count=$(echo "$code_files" | wc -l | tr -d ' ')

# Check what types of files changed
has_rust=$(echo "$code_files" | grep -c '\.rs$')
has_ts=$(echo "$code_files" | grep -c '\.\(ts\|js\|tsx\|jsx\)$')
has_css=$(echo "$code_files" | grep -c '\.css$')

echo ""
echo "=== $file_count code file(s) changed ==="
echo ""
echo "Consider running:"

# Always suggest testing
if [ "$has_rust" -gt 0 ] && [ "$has_ts" -gt 0 ]; then
  echo "  npm run test:smart          — run affected Rust + TS tests"
elif [ "$has_rust" -gt 0 ]; then
  echo "  npm run test:smart          — run affected Rust tests"
elif [ "$has_ts" -gt 0 ]; then
  echo "  npm test                    — run frontend tests"
fi

# Always suggest manual testing for UI changes
if [ "$has_ts" -gt 0 ] || [ "$has_css" -gt 0 ]; then
  echo "  /manual-testing <feature>   — test the feature via MCP + visual review"
fi

# Always suggest commit
echo "  /commit                     — commit with conventional format"

# Suggest learning from the session
echo "  /learn                      — capture any lessons from this session"
echo ""
