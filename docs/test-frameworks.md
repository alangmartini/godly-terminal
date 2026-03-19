# Test Frameworks Reference

Eight test tiers, each targeting a different layer of the stack. When reproducing a bug, pick the tier that exercises the real failure point — not the one that's easiest to write.

## Quick Reference

| Tier | Naming | Command | Environment | Mocks | Best For |
|------|--------|---------|-------------|-------|----------|
| **Unit** | `*.test.ts` | `pnpm test` | Node/jsdom | Tauri APIs | Store logic, services, pure functions, keyboard routing |
| **Browser** | `*.browser.test.ts` | `pnpm test:browser` | Real Chromium | Tauri APIs | Canvas2D rendering, pixel correctness, real layout, pointer events |
| **Integration** | `*.integration.test.ts` | `pnpm test:integration` | Node + spawned daemon | Nothing | Daemon protocol, session lifecycle, Quick Claude flow, IPC correctness |
| **Contract** | `contracts/*.json` | `pnpm --dir testing run run-contract contracts/<id>.json` | Godly Staging + MCP | Nothing | Feature acceptance, persistence across restart, regression coverage |
| **E2E** | `e2e/specs/*.e2e.ts` | `pnpm test:e2e` | Full Tauri app + WebdriverIO | Nothing | Full user workflows, persistence across restarts, input latency |
| **Daemon** | `daemon/tests/*.rs` | `cargo nextest run -p godly-daemon` | Isolated daemon process | Nothing | Concurrency, lock contention, memory leaks, pipe saturation, handler starvation |
| **Crate** | `#[test]` in `*.rs` | `cargo nextest run -p <crate>` | Rust unit | — | Parser correctness, serialization, data structures |
| **YAML E2E** | `tests/e2e-yaml/*.yaml` | `npm run test:yaml` | Full app + MCP HTTP | Nothing | Declarative Maestro-style E2E: freeze/minimize, workspace CRUD, split panes |

## Tier Details

### 1. Unit Tests (`pnpm test`)
- **Location**: `src/**/*.test.ts`
- **Environment**: Vitest + jsdom (Node.js DOM simulator)
- **What's real**: JavaScript logic, state machines, event bus
- **What's mocked**: All Tauri APIs (invoke, listen, Store, dialogs)
- **Catches**: State management bugs, event routing errors, keyboard shortcut conflicts, service logic regressions, plugin system errors
- **Cannot catch**: Canvas rendering bugs, real DOM layout, real CSS flexbox, pointer events (jsdom returns zeros for `getBoundingClientRect`)
- **Examples**: `src/state/store.split-navigation.test.ts`, `src/services/workspace-service.test.ts`

### 2. Browser Tests (`pnpm test:browser`)
- **Location**: `src/**/*.browser.test.ts`
- **Environment**: Vitest Browser Mode + real Chromium via Playwright
- **What's real**: DOM, CSS flexbox, Canvas2D, `measureText()`, `getImageData()`, pointer events
- **What's mocked**: Tauri APIs (via `src/test-utils/browser-setup.ts`)
- **Catches**: Canvas paint order bugs, font metric errors, pixel color correctness, flexbox layout regressions, split pane sizing bugs, divider positioning errors
- **Cannot catch**: Daemon interaction, session lifecycle, persistence
- **Use `pnpm test:browser:headed`** to see the Chromium window during tests
- **Examples**: `Canvas2DGridRenderer.browser.test.ts` (pixel inspection), `SplitContainer.browser.test.ts` (real layout)

### 3. Integration Tests (`pnpm test:integration`)
- **Location**: `integration/tests/**/*.integration.test.ts`
- **Environment**: Node.js + real spawned daemon (isolated per suite via `DaemonFixture`)
- **What's real**: Daemon binary, named pipe IPC, PTY sessions, shell processes, binary frame protocol
- **What's mocked**: Nothing — exercises the real daemon
- **Catches**: Protocol correctness (binary frames, JSON messages), session create/attach/detach lifecycle, IPC pipe saturation, command execution + output parsing, Quick Claude flow (trust prompt, incremental echo)
- **Cannot catch**: Frontend rendering, Tauri app lifecycle, persistence across restarts
- **Key infrastructure**: `DaemonFixture` (spawns isolated daemon), `DaemonClient` (TypeScript wire protocol), `SessionHandle` (high-level session API)
- **Examples**: `smoke.integration.test.ts`, `quick-claude.integration.test.ts`

### 4. Contract Tests (`pnpm --dir testing run run-contract`)
- **Location**: `testing/contracts/*.json`
- **Environment**: Running Godly Staging instance + godly-mcp stdio bridge
- **What's real**: Everything — full app, daemon, MCP tools, persistence, restarts
- **What's mocked**: Nothing
- **Catches**: Feature acceptance (does the feature work end-to-end?), workspace/terminal state bugs, persistence across app restart, regression when refactoring
- **Cannot catch**: Pixel-level rendering, sub-second timing, keyboard shortcut routing (contracts use MCP semantic actions, not raw input)
- **Key infrastructure**: `ContractRunner` (executes steps), `McpClient` (stdio bridge to godly-mcp), fixtures (`testing/fixtures/`)
- **Examples**: `workspace-folder-path.json` (folder CWD + persistence), `workspace-persistence.json` (workspace survive restart), `split-basic.json` (split pane operations)
- **List all contracts**: `pwsh testing/list-contracts.ps1`
- See [`testing/README.md`](../testing/README.md) for full contract architecture docs

### 5. E2E Tests (`pnpm test:e2e`)
- **Location**: `e2e/specs/**/*.e2e.ts`
- **Environment**: Full Tauri debug binary + WebdriverIO + tauri-driver + WebView2
- **What's real**: Everything — full app, daemon, renderer, persistence, IPC
- **What's mocked**: Nothing
- **Catches**: Session persistence across app restart, layout/scrollback/CWD persistence, keyboard shortcut routing (app vs terminal), tab drag-and-drop, input latency (key-to-grid, key-to-pixel), full user workflows end-to-end
- **Cannot catch**: Isolated component bugs (too high-level to pinpoint)
- **Gotchas**: Use `browser.execute()` for DOM queries (not `browser.$()`), use `invoke('write_to_terminal')` for input (not `browser.keys()`)
- **Examples**: `session-persistence.e2e.ts`, `input-latency.e2e.ts`, `keyboard-shortcuts.e2e.ts`

### 6. Daemon Tests (`cargo nextest run -p godly-daemon`)
- **Location**: `src-tauri/daemon/tests/**/*.rs`
- **Environment**: Isolated daemon process per test (unique pipe, unique instance, non-detached)
- **What's real**: Daemon binary, PTY sessions, ring buffers, godly-vt parser, named pipe IPC
- **What's mocked**: Nothing
- **Catches**: Mutex deadlocks, handler thread starvation, memory leaks (RSS monitoring), input latency under load, resize during output, adaptive batching behavior, pause/resume state, Ctrl+C signal handling
- **Cannot catch**: Frontend rendering, Tauri app integration
- **CRITICAL isolation rules**: unique `GODLY_PIPE_NAME` + `GODLY_INSTANCE` + `GODLY_NO_DETACH=1` + kill by PID (never `taskkill /IM`). See `DaemonFixture` pattern in `handler_starvation.rs`.
- **Examples**: `handler_starvation.rs` (lock contention), `input_latency.rs` (I/O bottleneck), `memory_stress.rs` (RSS tracking)

### 7. Crate Tests (`cargo nextest run -p <crate>`)
- **Location**: Inline `#[test]` blocks in crate source + `tests/` dirs
- **Environment**: Standard Rust unit tests
- **Catches**: VT parser state machine bugs, ANSI sequence handling, grid/cursor operations, binary frame serialization, image protocol (Kitty/iTerm2/Sixel) decoding
- **Key crates**: `godly-vt` (100+ tests), `godly-protocol` (message serialization)

### 8. YAML E2E Tests (`npm run test:yaml`)
- **Location**: `tests/e2e-yaml/*.yaml`
- **Runner**: `godly-test/godly-test.mjs` — a Maestro-like declarative YAML test runner
- **Environment**: Running Godly Terminal app + MCP HTTP server (Streamable HTTP on port 45557)
- **What's real**: Everything — full native app, daemon, MCP tools, OS window management
- **What's mocked**: Nothing
- **Catches**: Terminal freeze after minimize/restore, workspace CRUD lifecycle, terminal creation and command execution, split pane operations, theme switching, any scenario that needs time-based waits or OS-level window manipulation
- **Cannot catch**: Pixel-level rendering, sub-frame timing, keyboard shortcut routing (uses MCP semantic actions)

#### How it works

Each `.yaml` file is a flat list of steps. Every step maps to one MCP tool call or one assertion — no control flow, no loops. Inspired by [Maestro](https://maestro.mobile.dev/) for mobile.

```yaml
name: "Workspace create, switch, delete"
tags: [workspace, crud]
---
- resetApp                              # calls reset_staging_profile
- waitForReady                          # calls wait_for_app_ready

- createWorkspace:
    name: "Test CRUD"
    folder_path: "C:/tmp"
  store: ws                             # capture result into variable

- switchWorkspace:
    workspace_id: $ws.workspace_id      # reference captured value

- assertActiveWorkspace:
    workspace_id: $ws.workspace_id      # built-in assertion

- deleteWorkspace:
    workspace_id: $ws.workspace_id
```

**Key concepts:**
- **Step names** are camelCase aliases for MCP tools (`createWorkspace` → `create_workspace`)
- **`store: varName`** captures the MCP response JSON into a variable
- **`$var.field`** references captured values in subsequent steps
- **`assert*` steps** are built-in assertions that call MCP tools and check results
- **Auto-cleanup**: runner tracks created workspaces/terminals and tears them down on exit

#### Available steps

| Category | Steps |
|----------|-------|
| Lifecycle | `resetApp`, `waitForReady`, `getAppInfo` |
| Workspace | `createWorkspace`, `switchWorkspace`, `deleteWorkspace`, `renameWorkspace`, `listWorkspaces`, `getActiveWorkspace` |
| Terminal | `createTerminal`, `closeTerminal`, `focusTerminal`, `listTerminals`, `getActiveTerminal` |
| I/O | `executeCommand`, `writeToTerminal`, `sendKeys`, `readTerminal`, `readGrid`, `eraseContent` |
| Wait | `waitForText`, `waitForIdle`, `sleep` |
| Split | `splitTerminal`, `createSplit`, `clearSplit`, `unsplitTerminal`, `getSplitState` |
| Theme | `setTheme`, `listThemes`, `getActiveTheme` |
| Assertion | `assertTextContains`, `assertGridContains`, `assertWorkspaceCount`, `assertTerminalCount`, `assertActiveWorkspace`, `assertEqual`, `assertNotEmpty` |
| Other | `screenshot`, `exportStateDump`, `notify`, `log` |

#### CLI commands

```bash
# Run all YAML tests
npm run test:yaml

# Run a single test
node godly-test/godly-test.mjs run tests/e2e-yaml/smoke.yaml

# Run with options
node godly-test/godly-test.mjs run tests/e2e-yaml/ --filter smoke --bail --verbose

# Validate YAML syntax without running
npm run test:yaml:validate

# List available tests
npm run test:yaml:list
```

#### Output (Maestro-style)

```
 godly-test v0.1.0

 workspace-crud.yaml
   [1/6] resetApp                         ✓  120ms
   [2/6] waitForReady                     ✓  340ms
   [3/6] createWorkspace "Test CRUD"      ✓   85ms
   [4/6] switchWorkspace                  ✓   42ms
   [5/6] assertActiveWorkspace            ✓   38ms
   [6/6] deleteWorkspace                  ✓   55ms

 ──────────────────────────────────────
 Results: 1 passed, 0 failed (6 steps)
 Duration: 0.68s
```

On failure: indented error message + auto-captured screenshot.

#### Writing new tests

1. Create a `.yaml` file in `tests/e2e-yaml/`
2. Add front matter (`name:`, `tags:`) separated by `---` from the step list
3. Use `store:` to capture results, `$var.field` to reference them
4. Use `assert*` steps to verify expected state
5. Any raw snake_case MCP tool name also works as a step (e.g., `get_font_size`)
6. Validate with `npm run test:yaml:validate` before running

## Bug → Test Tier Decision Tree

| Bug symptom | Test tier | Why |
|-------------|-----------|-----|
| Rendering glitch, wrong colors, garbled text on screen | **Browser** | Needs real Canvas2D + pixel inspection |
| Layout broken, panes wrong size, divider misplaced | **Browser** | Needs real CSS flexbox + `getBoundingClientRect` |
| Keyboard shortcut doesn't work or conflicts | **Unit** | Shortcut routing is pure logic (keybinding-store) |
| Terminal output missing, wrong, or delayed | **Integration** | Needs real daemon + shell process |
| Session lost after app restart | **E2E** | Needs full app lifecycle with persistence |
| Daemon freezes, all terminals unresponsive | **Daemon** | Lock contention / handler starvation |
| High input latency, slow typing | **Daemon** or **E2E** | Daemon for I/O bottleneck, E2E for full pipeline measurement |
| Memory leak over time | **Daemon** | RSS monitoring with `GetProcessMemoryInfo` |
| Workspace/tab state bug | **Unit** or **Contract** | Unit for store logic, Contract for persistence + acceptance |
| Feature acceptance / regression | **Contract** | Declarative steps against running staging app |
| Terminal freeze after minimize/restore | **YAML E2E** | Needs OS-level window manipulation + time-based waits |
| Long-duration idle/resume scenarios | **YAML E2E** | Declarative YAML with `sleep` + assertions, easy to tweak timing |
| Quick Claude flow broken | **Integration** | DaemonFixture + SessionHandle exercises real CLI |
| Protocol parsing error | **Crate** | godly-protocol unit tests |
| VT escape sequence mishandled | **Crate** | godly-vt parser tests |
| Drag-and-drop, pointer interaction broken | **Browser** or **E2E** | Browser for component, E2E for full workflow |

## Workflow Notes

- **Bug fixes**: Write a full test **suite** (not a single test) to reproduce the bug. Pick the tier from the decision tree above.
- **Features**: Write **E2E tests** (`pnpm test:e2e`), not just unit tests. For Canvas2D/layout features, also write **browser tests** (`*.browser.test.ts`).
- **Performance issues**: Always write automated reproducible tests that demonstrate the problem under realistic conditions. See `daemon/tests/input_latency.rs` and `daemon/tests/handler_starvation.rs` for patterns.
