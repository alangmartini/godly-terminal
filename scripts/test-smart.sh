#!/usr/bin/env bash
# test-smart.sh — Run only the tests affected by current changes.
#
# Detects changed files via git diff HEAD, maps them to workspace crates,
# propagates through the dependency graph, and runs cargo nextest + npm test
# only where needed.

set -euo pipefail

# ── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

info()  { echo -e "${CYAN}[info]${RESET}  $*"; }
pass()  { echo -e "${GREEN}[pass]${RESET}  $*"; }
fail()  { echo -e "${RED}[FAIL]${RESET}  $*"; }
skip()  { echo -e "${YELLOW}[skip]${RESET}  $*"; }
header(){ echo -e "\n${BOLD}$*${RESET}"; }

# ── Gather changed files ───────────────────────────────────────────────────
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

CHANGED_FILES=$(git diff HEAD --name-only 2>/dev/null || true)
# Also include untracked files (new files not yet committed)
UNTRACKED_FILES=$(git ls-files --others --exclude-standard 2>/dev/null || true)
ALL_CHANGED="${CHANGED_FILES}
${UNTRACKED_FILES}"
ALL_CHANGED=$(echo "$ALL_CHANGED" | sed '/^$/d' | sort -u)

if [ -z "$ALL_CHANGED" ]; then
  skip "No changed files detected — nothing to test."
  exit 0
fi

header "Changed files:"
echo "$ALL_CHANGED" | while IFS= read -r f; do echo "  $f"; done

# ── Map files → crates ─────────────────────────────────────────────────────
# Associative-array-free approach for Git Bash compatibility (bash 3.x).
# We track affected crates as a space-separated string.

AFFECTED=""       # space-separated crate names
FRONTEND=false    # whether to run npm test

add_crate() {
  local crate="$1"
  # Deduplicate: only add if not already present
  case " $AFFECTED " in
    *" $crate "*) ;;
    *) AFFECTED="$AFFECTED $crate" ;;
  esac
}

while IFS= read -r file; do
  case "$file" in
    src-tauri/protocol/*)   add_crate "godly-protocol"  ;;
    src-tauri/godly-vt/*)   add_crate "godly-vt"        ;;
    src-tauri/daemon/*)     add_crate "godly-daemon"     ;;
    src-tauri/pty-shim/*)   add_crate "godly-pty-shim"   ;;
    src-tauri/src/*)        add_crate "godly-terminal"   ;;
    src-tauri/Cargo.*)      # Workspace-level Cargo changes — test everything
                            add_crate "godly-protocol"
                            add_crate "godly-vt"
                            add_crate "godly-daemon"
                            add_crate "godly-pty-shim"
                            add_crate "godly-terminal"
                            ;;
    src/*.ts|src/*.js|src/**/*.ts|src/**/*.js)
                            FRONTEND=true ;;
    *.ts|*.js)
      # Top-level or other TS/JS files (e.g. vite.config.ts)
      FRONTEND=true ;;
  esac
done <<< "$ALL_CHANGED"

# Second pass for frontend: check any file under src/ (not src-tauri/src/)
# The case patterns above may not catch nested paths, so do an explicit grep.
if echo "$ALL_CHANGED" | grep -qE '^src/.*\.(ts|js|tsx|jsx)$'; then
  FRONTEND=true
fi

# ── Dependency propagation ──────────────────────────────────────────────────
# godly-protocol → godly-daemon, godly-vt, godly-terminal
# godly-vt       → godly-daemon
# godly-pty-shim → godly-daemon

propagate() {
  local changed=true
  while $changed; do
    changed=false

    case " $AFFECTED " in
      *" godly-protocol "*)
        for dep in godly-daemon godly-vt godly-terminal; do
          case " $AFFECTED " in
            *" $dep "*) ;;
            *) AFFECTED="$AFFECTED $dep"; changed=true ;;
          esac
        done
        ;;
    esac

    case " $AFFECTED " in
      *" godly-vt "*)
        case " $AFFECTED " in
          *" godly-daemon "*) ;;
          *) AFFECTED="$AFFECTED godly-daemon"; changed=true ;;
        esac
        ;;
    esac

    case " $AFFECTED " in
      *" godly-pty-shim "*)
        case " $AFFECTED " in
          *" godly-daemon "*) ;;
          *) AFFECTED="$AFFECTED godly-daemon"; changed=true ;;
        esac
        ;;
    esac
  done
}

BEFORE_PROPAGATION="$AFFECTED"
propagate

# Trim leading/trailing spaces
AFFECTED=$(echo "$AFFECTED" | xargs)

header "Affected crates:"
if [ -n "$AFFECTED" ]; then
  for crate in $AFFECTED; do
    case " $BEFORE_PROPAGATION " in
      *" $crate "*) echo "  $crate  (direct change)" ;;
      *)            echo "  $crate  (dependency propagation)" ;;
    esac
  done
else
  skip "No Rust crates affected."
fi
if $FRONTEND; then
  echo "  frontend  (direct change)"
fi

# ── Count affected crates ──────────────────────────────────────────────────
CRATE_COUNT=0
if [ -n "$AFFECTED" ]; then
  CRATE_COUNT=$(echo "$AFFECTED" | wc -w | tr -d ' ')
fi

FULL_SUITE=false
if [ "$CRATE_COUNT" -gt 5 ]; then
  FULL_SUITE=true
  info "More than 5 crates affected ($CRATE_COUNT) — falling back to full suite."
fi

# ── Run tests ───────────────────────────────────────────────────────────────
EXIT_CODE=0

run_rust_tests() {
  if [ "$CRATE_COUNT" -eq 0 ]; then
    skip "No Rust crates to test."
    return
  fi

  header "Running Rust tests..."

  if $FULL_SUITE; then
    info "cargo nextest run --workspace --profile fast"
    if (cd src-tauri && cargo nextest run --workspace --profile fast); then
      pass "Full Rust test suite passed."
    else
      fail "Full Rust test suite FAILED."
      EXIT_CODE=1
    fi
  else
    for crate in $AFFECTED; do
      info "cargo nextest run -p $crate --profile fast"
      if (cd src-tauri && cargo nextest run -p "$crate" --profile fast); then
        pass "$crate"
      else
        fail "$crate"
        EXIT_CODE=1
      fi
    done
  fi
}

run_frontend_tests() {
  if ! $FRONTEND; then
    skip "No frontend files changed — skipping npm test."
    return
  fi

  header "Running frontend tests..."
  info "npm test"
  if npm test; then
    pass "Frontend tests passed."
  else
    fail "Frontend tests FAILED."
    EXIT_CODE=1
  fi
}

run_rust_tests
run_frontend_tests

# ── Summary ─────────────────────────────────────────────────────────────────
echo ""
if [ "$EXIT_CODE" -eq 0 ]; then
  pass "All affected tests passed."
else
  fail "Some tests failed (exit code $EXIT_CODE)."
fi

exit $EXIT_CODE
