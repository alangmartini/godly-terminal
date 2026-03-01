#!/usr/bin/env bash
# Detect file overlap between active worktree branches to prevent merge conflicts.
# Usage: scripts/check-file-overlap.sh [base-branch]
set -euo pipefail
BASE=${1:-master}
echo "Checking file overlap between worktree branches (base: $BASE)..."
declare -A file_branches
conflicts=0

for wt_line in $(git worktree list --porcelain | grep '^worktree ' | sed 's/^worktree //'); do
  branch=$(git -C "$wt_line" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "detached")
  [[ "$branch" == "$BASE" || "$branch" == "detached" || "$branch" == "HEAD" ]] && continue

  while IFS= read -r file; do
    if [[ -n "${file_branches[$file]:-}" ]]; then
      echo "OVERLAP: $file modified by both '${file_branches[$file]}' and '$branch'"
      conflicts=$((conflicts + 1))
    else
      file_branches[$file]="$branch"
    fi
  done < <(git diff --name-only "$BASE"..."$branch" 2>/dev/null)
done

if [[ $conflicts -eq 0 ]]; then
  echo "No file overlaps detected."
else
  echo ""
  echo "$conflicts file overlap(s) found."
  exit 1
fi
