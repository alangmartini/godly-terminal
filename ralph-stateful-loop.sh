#!/usr/bin/env bash
# Stateful Ralph Loop Launcher
#
# Usage:
#   ./ralph-stateful-loop.sh [OPTIONS]
#   ./ralph-stateful-loop.sh [OPTIONS] "goal text with image paths"
#
# Examples:
#   ./ralph-stateful-loop.sh                          # Auto-discover improvements
#   ./ralph-stateful-loop.sh --reset "Make the UI snappier"
#   ./ralph-stateful-loop.sh "current.png reference.png Make our app (current.png) match (reference.png)"
#
# Image paths in the goal are detected automatically and passed to Claude
# so it can view them. Supports: .png .jpg .jpeg .gif .bmp .webp .svg
#
# State persists in .ralph-state/STATE.md across iterations AND across
# cancellations - just re-run this script to resume.

set -uo pipefail
# NOTE: intentionally no `set -e` — the main loop must survive non-zero
# exits from `claude --print` (e.g. context overflow, API errors).
cd "$(dirname "$0")"

MAX_MAJOR=0  # 0 = unlimited
RESET=false
GOAL=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --max-major)
      MAX_MAJOR="$2"
      shift 2
      ;;
    --reset)
      RESET=true
      shift
      ;;
    --help|-h)
      echo "Usage: ./ralph-stateful-loop.sh [OPTIONS] [GOAL]"
      echo ""
      echo "Options:"
      echo "  --max-major N   Stop after N major loops complete (0 = unlimited)"
      echo "  --reset         Clear state and start fresh"
      echo ""
      echo "Goal:"
      echo "  Optional free-form text describing what to improve."
      echo "  Image paths (.png, .jpg, etc.) in the goal are auto-detected"
      echo "  and passed to Claude for visual comparison."
      echo ""
      echo "Examples:"
      echo "  ./ralph-stateful-loop.sh"
      echo "  ./ralph-stateful-loop.sh \"Fix the sidebar layout\""
      echo "  ./ralph-stateful-loop.sh \"current.png ref.png Match (current.png) to (ref.png)\""
      echo ""
      echo "State is stored in .ralph-state/STATE.md"
      echo "Cancel anytime - re-run to resume from where you left off."
      exit 0
      ;;
    --*)
      echo "Unknown option: $1"
      exit 1
      ;;
    *)
      # Anything that's not a flag is the goal
      GOAL="$1"
      shift
      ;;
  esac
done

if $RESET; then
  echo "Clearing state..."
  rm -f .ralph-state/STATE.md .ralph-state/GOAL.md
  echo "State cleared. Starting fresh."
fi

# Ensure state directory exists
mkdir -p .ralph-state

# If a goal was provided, save it so it persists across re-runs
if [[ -n "$GOAL" ]]; then
  # Extract image paths from the goal (anything ending in common image extensions)
  IMAGES=()
  for word in $GOAL; do
    if [[ "$word" =~ \.(png|jpg|jpeg|gif|bmp|webp|svg)$ ]]; then
      # Resolve to absolute path if file exists, otherwise keep as-is
      if [[ -f "$word" ]]; then
        IMAGES+=("$(realpath "$word")")
      elif [[ -f "$(pwd)/$word" ]]; then
        IMAGES+=("$(realpath "$(pwd)/$word")")
      else
        echo "WARNING: Image path '$word' not found. Including path as-is."
        IMAGES+=("$word")
      fi
    fi
  done

  # Write the goal file
  {
    echo "# User Goal"
    echo ""
    echo "$GOAL"
    if [[ ${#IMAGES[@]} -gt 0 ]]; then
      echo ""
      echo "## Reference Images"
      echo "Claude: use your Read tool to view these images for visual comparison."
      for img in "${IMAGES[@]}"; do
        echo "- \`$img\`"
      done
    fi
  } > .ralph-state/GOAL.md
  echo "Goal saved to .ralph-state/GOAL.md"
  if [[ ${#IMAGES[@]} -gt 0 ]]; then
    echo "Detected ${#IMAGES[@]} image(s): ${IMAGES[*]}"
  fi
elif [[ ! -f .ralph-state/GOAL.md ]]; then
  # No goal provided and no saved goal - that's fine, DISCOVER will auto-find
  echo "No goal specified. Will auto-discover improvements."
fi

# Check if state exists (resuming vs fresh start)
if [[ -f .ralph-state/STATE.md ]]; then
  echo "=== Resuming Stateful Ralph Loop ==="
  echo "Found existing state in .ralph-state/STATE.md"
  if [[ -f .ralph-state/GOAL.md ]]; then
    echo "Goal: $(sed -n '3p' .ralph-state/GOAL.md)"
  fi
  # Show current phase
  if grep -q "Status.*PLANNING" .ralph-state/STATE.md 2>/dev/null; then
    echo "Current phase: PLAN_MINOR (breaking down major loop)"
  elif grep -q "Status.*IN_PROGRESS" .ralph-state/STATE.md 2>/dev/null; then
    if grep -q "Status.*PENDING\|Status.*IN_PROGRESS" .ralph-state/STATE.md 2>/dev/null; then
      echo "Current phase: IMPLEMENT (working on minor loops)"
    else
      echo "Current phase: VALIDATE (all minors complete)"
    fi
  else
    echo "Current phase: DISCOVER (finding next improvement)"
  fi
else
  echo "=== Starting Fresh Stateful Ralph Loop ==="
  echo "No existing state found. Will start with DISCOVER phase."
fi

echo ""
echo "Press Ctrl+C to cancel at any time."
echo "Re-run this script to resume from where you left off."
echo ""

# Build the prompt from the template
PROMPT=$(cat RALPH_STATEFUL_PROMPT.md)

# Count completed major loops if max is set
check_major_count() {
  if [[ $MAX_MAJOR -gt 0 ]] && [[ -f .ralph-state/STATE.md ]]; then
    local count
    count=$(grep -c "^## Major Loop.*COMPLETE" .ralph-state/STATE.md 2>/dev/null || echo "0")
    # Count history entries as completed major loops
    local history_count
    history_count=$(grep -c "^### Major Loop #" .ralph-state/STATE.md 2>/dev/null || echo "0")
    if [[ $history_count -ge $MAX_MAJOR ]]; then
      echo "=== Reached max major loops ($MAX_MAJOR). Stopping. ==="
      exit 0
    fi
  fi
}

# Main loop - Ralph feeds the same prompt, but the state file changes
ITERATION=0
while true; do
  ITERATION=$((ITERATION + 1))

  check_major_count

  echo ""
  echo "=========================================="
  echo "  Stateful Ralph Loop - Iteration $ITERATION"
  echo "=========================================="
  echo ""

  # Feed the prompt to Claude Code with JSON output for token tracking.
  # Write to temp file, capture exit code, parse JSON for tokens.
  RESULT_FILE=".ralph-state/last-result.json"
  PROMPT_FILE=".ralph-state/last-prompt.txt"
  echo "$PROMPT" > "$PROMPT_FILE"
  claude --print --dangerously-skip-permissions --output-format json < "$PROMPT_FILE" > "$RESULT_FILE" 2>.ralph-state/last-stderr.log
  EXIT_CODE=$?

  # Print the text result for the log
  jq -r '.result // empty' "$RESULT_FILE" 2>/dev/null

  # Extract token usage from JSON and log it.
  # Wrapped in subshell to prevent set -u from killing the main loop on parse errors.
  (
    set +u  # allow unset vars in this block
    if [[ -s "$RESULT_FILE" ]] && jq -e '.usage' "$RESULT_FILE" &>/dev/null; then
      INPUT_TOKENS=$(jq -r '.usage.input_tokens // 0' "$RESULT_FILE")
      OUTPUT_TOKENS=$(jq -r '.usage.output_tokens // 0' "$RESULT_FILE")
      CACHE_READ=$(jq -r '.usage.cache_read_input_tokens // 0' "$RESULT_FILE")
      CACHE_CREATE=$(jq -r '.usage.cache_creation_input_tokens // 0' "$RESULT_FILE")
      COST_USD=$(jq -r '.total_cost_usd // 0' "$RESULT_FILE")
      DURATION_MS=$(jq -r '.duration_ms // 0' "$RESULT_FILE")
      NUM_TURNS=$(jq -r '.num_turns // 0' "$RESULT_FILE")
      NON_CACHED=$INPUT_TOKENS
      TOTAL_INPUT=$((INPUT_TOKENS + CACHE_READ + CACHE_CREATE))

      TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
      TOKEN_LINE="[$TIMESTAMP] Iter $ITERATION | Total-in: $TOTAL_INPUT | Non-cached: $NON_CACHED | Cache-read: $CACHE_READ | Cache-create: $CACHE_CREATE | Output: $OUTPUT_TOKENS | Cost: \$$COST_USD | Duration: ${DURATION_MS}ms | Turns: $NUM_TURNS"
      echo "$TOKEN_LINE" >> .ralph-state/token-usage.log
      echo "$TOKEN_LINE"

      # Update tokens.md summary
      SUM_ITERS=$(wc -l < .ralph-state/token-usage.log)
      SUM_COST=$(awk -F'Cost: \\$' '{sum += $2} END {printf "%.4f", sum}' .ralph-state/token-usage.log)
      SUM_INPUT=$(awk -F'Total-in: ' '{split($2, a, " "); sum += a[1]} END {print sum}' .ralph-state/token-usage.log)
      SUM_OUTPUT=$(awk -F'Output: ' '{split($2, a, " "); sum += a[1]} END {print sum}' .ralph-state/token-usage.log)
      SUM_NON_CACHED=$(awk -F'Non-cached: ' '{split($2, a, " "); sum += a[1]} END {print sum}' .ralph-state/token-usage.log)

      cat > .ralph-state/tokens.md << TOKENS_EOF
# Token Usage Summary

**Total iterations**: $SUM_ITERS
**Total cost**: \$$SUM_COST
**Total input tokens**: $SUM_INPUT
**Total output tokens**: $SUM_OUTPUT
**Total non-cached input tokens**: $SUM_NON_CACHED

## Last Iteration (#$ITERATION)
- Total input: $TOTAL_INPUT (non-cached: $NON_CACHED)
- Output: $OUTPUT_TOKENS
- Cache read: $CACHE_READ | Cache create: $CACHE_CREATE
- Cost: \$$COST_USD
- Duration: ${DURATION_MS}ms | Turns: $NUM_TURNS

## Full Log
See \`token-usage.log\` for per-iteration breakdown.
TOKENS_EOF
    else
      echo "[$TIMESTAMP] Iter $ITERATION | No token data (result file empty or parse failed)" >> .ralph-state/token-usage.log
    fi
  ) || true

  if [[ $EXIT_CODE -ne 0 ]]; then
    echo ""
    echo "Claude exited with code $EXIT_CODE. Pausing 5s before retry..."
    sleep 5
  fi

  echo ""
  echo "--- Iteration $ITERATION complete. State persisted. Starting next iteration... ---"
  echo ""
  sleep 2
done
