# MCP Window Freeze Investigation

## Symptom
When Claude Code creates terminals via MCP tools (godly-terminal MCP server), the Godly Terminal app freezes after a new "Agent" window opens.

## Root Cause

**Event race condition:** The MCP handler creates the Agent window and immediately emits `mcp-terminal-created` events. But the new window's WebView2 hasn't loaded its JavaScript yet, so the event listener in `setupMcpEventListeners()` isn't registered. The event is lost.

**Result:** The MCP window appears blank/frozen — no terminals are displayed despite them existing in the daemon.

**Secondary issue:** The MCP window was created with default focus behavior, stealing focus from the main terminal window. This made the main window appear unresponsive.

## Timeline of the Bug

1. MCP tool `create_terminal` is called
2. Backend calls `ensure_mcp_window()` → creates new WebView window
3. Backend continues: creates daemon session, attaches, emits `mcp-terminal-created`
4. **Main window** receives event → adds hidden "Agent" workspace, returns (OK)
5. **MCP window** is still loading HTML/JS → misses the event entirely
6. MCP window finishes loading → empty state, appears frozen

## Fix (PR: fix/mcp-window-freeze)

### 1. Added `get_mcp_state` Tauri command (`commands/workspace.rs`)
Returns the Agent workspace and its terminals from live backend state. The MCP window calls this during `init()` to bootstrap any terminals that were created before the window was ready.

### 2. MCP window bootstraps on load (`App.ts`)
Instead of relying solely on events, the MCP window now queries `get_mcp_state` after setting up event listeners. This handles both:
- Terminals created before the window was ready (race condition)
- Terminals created after the window is ready (via event listeners)

### 3. MCP window doesn't steal focus (`handler.rs`)
Added `.focused(false)` to `WebviewWindowBuilder` so the Agent window opens without taking focus from the main terminal.

## Resolution
Fixed. All three changes applied in branch `fix/mcp-window-freeze`.
