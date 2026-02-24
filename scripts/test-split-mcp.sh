#!/usr/bin/env bash
# Test split panel MCP tools on staging instance
set -euo pipefail

MCP_BIN="/c/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri/target/debug/godly-mcp.exe"

call_mcp() {
  local tool="$1"
  local args="$2"
  printf '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$tool" "$args" \
    | GODLY_INSTANCE=staging timeout 15 "$MCP_BIN" 2>/dev/null \
    | tail -1
}

extract_text() {
  python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(d['result']['content'][0]['text'])" 2>/dev/null
}

extract_id() {
  python3 -c "import sys,json; d=json.loads(sys.stdin.read()); t=json.loads(d['result']['content'][0]['text']); print(t['id'])" 2>/dev/null
}

echo "=========================================="
echo " Split Panel MCP Test Suite (Staging)"
echo "=========================================="
echo ""

# Test 1: List workspaces
echo "--- Test 1: List workspaces ---"
call_mcp "list_workspaces" '{}' | extract_text
echo ""

# Test 2: Create test workspace
echo "--- Test 2: Create test workspace ---"
WS_ID=$(call_mcp "create_workspace" '{"name":"Split Test","folder_path":"C:\\\\Users\\\\alanm"}' | extract_id)
echo "Workspace ID: $WS_ID"
echo ""

# Test 3: Create Terminal A
echo "--- Test 3: Create Terminal A ---"
TERM_A=$(call_mcp "create_terminal" "{\"workspace_id\":\"$WS_ID\",\"command\":\"echo Terminal-A\"}" | extract_id)
echo "Terminal A ID: $TERM_A"
sleep 2
echo ""

# Test 4: Create Terminal B
echo "--- Test 4: Create Terminal B ---"
TERM_B=$(call_mcp "create_terminal" "{\"workspace_id\":\"$WS_ID\",\"command\":\"echo Terminal-B\"}" | extract_id)
echo "Terminal B ID: $TERM_B"
sleep 2
echo ""

# Test 5: Create Terminal C (extra tab)
echo "--- Test 5: Create Terminal C ---"
TERM_C=$(call_mcp "create_terminal" "{\"workspace_id\":\"$WS_ID\",\"command\":\"echo Terminal-C\"}" | extract_id)
echo "Terminal C ID: $TERM_C"
sleep 2
echo ""

# Test 6: Switch to the test workspace
echo "--- Test 6: Switch to test workspace ---"
call_mcp "switch_workspace" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
sleep 1
echo ""

# Test 7: Get split state (should be empty)
echo "--- Test 7: Get split state (before split) ---"
call_mcp "get_split_state" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
echo ""

# Test 8: Create horizontal split
echo "--- Test 8: Create horizontal split (A | B) ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_A\",\"right_terminal_id\":\"$TERM_B\",\"direction\":\"horizontal\",\"ratio\":0.5}" | extract_text
sleep 2
echo ""

# Test 9: Get split state (should show split)
echo "--- Test 9: Get split state (after split) ---"
call_mcp "get_split_state" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
echo ""

# Test 10: Execute JS to query store state
echo "--- Test 10: Execute JS - query split views ---"
call_mcp "execute_js" '{"script":"return JSON.stringify(window.__STORE__.getState().splitViews)"}' | extract_text
echo ""

# Test 11: Execute JS - get divider position
echo "--- Test 11: Execute JS - divider position ---"
call_mcp "execute_js" '{"script":"const d = document.querySelector(\".split-divider\"); return d ? JSON.stringify(d.getBoundingClientRect()) : null"}' | extract_text
echo ""

# Test 12: Capture screenshot
echo "--- Test 12: Capture screenshot ---"
call_mcp "capture_screenshot" '{}' | extract_text
echo ""

# Test 13: Change split ratio
echo "--- Test 13: Update split ratio to 0.7 ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_A\",\"right_terminal_id\":\"$TERM_B\",\"direction\":\"horizontal\",\"ratio\":0.7}" | extract_text
sleep 1
echo ""

# Test 14: Get split state after ratio change
echo "--- Test 14: Get split state (ratio should be 0.7) ---"
call_mcp "get_split_state" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
echo ""

# Test 15: Switch to vertical split
echo "--- Test 15: Switch to vertical split ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_A\",\"right_terminal_id\":\"$TERM_B\",\"direction\":\"vertical\",\"ratio\":0.5}" | extract_text
sleep 1
echo ""

# Test 16: Get split state (should show vertical)
echo "--- Test 16: Get split state (vertical) ---"
call_mcp "get_split_state" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
echo ""

# Test 17: Swap terminals in split (B | A instead of A | B)
echo "--- Test 17: Swap panes (B left, A right) ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_B\",\"right_terminal_id\":\"$TERM_A\",\"direction\":\"horizontal\",\"ratio\":0.5}" | extract_text
sleep 1
echo ""

# Test 18: Split with terminal C replacing B
echo "--- Test 18: Replace right pane with Terminal C ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_B\",\"right_terminal_id\":\"$TERM_C\",\"direction\":\"horizontal\",\"ratio\":0.5}" | extract_text
sleep 1
echo ""

# Test 19: Clear split
echo "--- Test 19: Clear split ---"
call_mcp "clear_split" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
sleep 1
echo ""

# Test 20: Get split state after clear (should be empty)
echo "--- Test 20: Get split state (after clear) ---"
call_mcp "get_split_state" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
echo ""

# Test 21: Edge case - split with invalid terminal
echo "--- Test 21: Edge case - invalid terminal ID ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"nonexistent\",\"right_terminal_id\":\"$TERM_B\",\"direction\":\"horizontal\"}" | extract_text 2>&1 || echo "(Expected error)"
echo ""

# Test 22: Edge case - split with invalid direction
echo "--- Test 22: Edge case - invalid direction ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_A\",\"right_terminal_id\":\"$TERM_B\",\"direction\":\"diagonal\"}" | extract_text 2>&1 || echo "(Expected error)"
echo ""

# Test 23: Edge case - extreme ratio (clamped to 0.15-0.85)
echo "--- Test 23: Edge case - extreme ratio 0.01 (should clamp to 0.15) ---"
call_mcp "create_split" "{\"workspace_id\":\"$WS_ID\",\"left_terminal_id\":\"$TERM_A\",\"right_terminal_id\":\"$TERM_B\",\"direction\":\"horizontal\",\"ratio\":0.01}" | extract_text
call_mcp "get_split_state" "{\"workspace_id\":\"$WS_ID\"}" | extract_text
echo ""

# Cleanup
echo "--- Cleanup ---"
call_mcp "clear_split" "{\"workspace_id\":\"$WS_ID\"}" | extract_text 2>/dev/null
call_mcp "close_terminal" "{\"terminal_id\":\"$TERM_A\"}" | extract_text 2>/dev/null
call_mcp "close_terminal" "{\"terminal_id\":\"$TERM_B\"}" | extract_text 2>/dev/null
call_mcp "close_terminal" "{\"terminal_id\":\"$TERM_C\"}" | extract_text 2>/dev/null
call_mcp "delete_workspace" "{\"workspace_id\":\"$WS_ID\"}" | extract_text 2>/dev/null
echo "Cleanup complete"
echo ""

echo "=========================================="
echo " Test Suite Complete"
echo "=========================================="
