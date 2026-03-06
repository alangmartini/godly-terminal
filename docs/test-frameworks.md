# Test Frameworks Reference

Six test tiers, each targeting a different layer of the stack. When reproducing a bug, pick the tier that exercises the real failure point — not the one that's easiest to write.

## Quick Reference

| Tier | Naming | Command | Environment | Mocks | Best For |
|------|--------|---------|-------------|-------|----------|
| **Unit** | `*.test.ts` | `pnpm test` | Node/jsdom | Tauri APIs | Store logic, services, pure functions, keyboard routing |
| **Browser** | `*.browser.test.ts` | `pnpm test:browser` | Real Chromium | Tauri APIs | Canvas2D rendering, pixel correctness, real layout, pointer events |
| **Integration** | `*.integration.test.ts` | `pnpm test:integration` | Node + spawned daemon | Nothing | Daemon protocol, session lifecycle, Quick Claude flow, IPC correctness |
| **E2E** | `e2e/specs/*.e2e.ts` | `pnpm test:e2e` | Full Tauri app + WebdriverIO | Nothing | Full user workflows, persistence across restarts, input latency |
| **Daemon** | `daemon/tests/*.rs` | `cargo nextest run -p godly-daemon` | Isolated daemon process | Nothing | Concurrency, lock contention, memory leaks, pipe saturation, handler starvation |
| **Crate** | `#[test]` in `*.rs` | `cargo nextest run -p <crate>` | Rust unit | — | Parser correctness, serialization, data structures |

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

### 4. E2E Tests (`pnpm test:e2e`)
- **Location**: `e2e/specs/**/*.e2e.ts`
- **Environment**: Full Tauri debug binary + WebdriverIO + tauri-driver + WebView2
- **What's real**: Everything — full app, daemon, renderer, persistence, IPC
- **What's mocked**: Nothing
- **Catches**: Session persistence across app restart, layout/scrollback/CWD persistence, keyboard shortcut routing (app vs terminal), tab drag-and-drop, input latency (key-to-grid, key-to-pixel), full user workflows end-to-end
- **Cannot catch**: Isolated component bugs (too high-level to pinpoint)
- **Gotchas**: Use `browser.execute()` for DOM queries (not `browser.$()`), use `invoke('write_to_terminal')` for input (not `browser.keys()`)
- **Examples**: `session-persistence.e2e.ts`, `input-latency.e2e.ts`, `keyboard-shortcuts.e2e.ts`

### 5. Daemon Tests (`cargo nextest run -p godly-daemon`)
- **Location**: `src-tauri/daemon/tests/**/*.rs`
- **Environment**: Isolated daemon process per test (unique pipe, unique instance, non-detached)
- **What's real**: Daemon binary, PTY sessions, ring buffers, godly-vt parser, named pipe IPC
- **What's mocked**: Nothing
- **Catches**: Mutex deadlocks, handler thread starvation, memory leaks (RSS monitoring), input latency under load, resize during output, adaptive batching behavior, pause/resume state, Ctrl+C signal handling
- **Cannot catch**: Frontend rendering, Tauri app integration
- **CRITICAL isolation rules**: unique `GODLY_PIPE_NAME` + `GODLY_INSTANCE` + `GODLY_NO_DETACH=1` + kill by PID (never `taskkill /IM`). See `DaemonFixture` pattern in `handler_starvation.rs`.
- **Examples**: `handler_starvation.rs` (lock contention), `input_latency.rs` (I/O bottleneck), `memory_stress.rs` (RSS tracking)

### 6. Crate Tests (`cargo nextest run -p <crate>`)
- **Location**: Inline `#[test]` blocks in crate source + `tests/` dirs
- **Environment**: Standard Rust unit tests
- **Catches**: VT parser state machine bugs, ANSI sequence handling, grid/cursor operations, binary frame serialization, image protocol (Kitty/iTerm2/Sixel) decoding
- **Key crates**: `godly-vt` (100+ tests), `godly-protocol` (message serialization)

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
| Workspace/tab state bug | **Unit** or **E2E** | Unit for store logic, E2E for persistence |
| Quick Claude flow broken | **Integration** | DaemonFixture + SessionHandle exercises real CLI |
| Protocol parsing error | **Crate** | godly-protocol unit tests |
| VT escape sequence mishandled | **Crate** | godly-vt parser tests |
| Drag-and-drop, pointer interaction broken | **Browser** or **E2E** | Browser for component, E2E for full workflow |

## Workflow Notes

- **Bug fixes**: Write a full test **suite** (not a single test) to reproduce the bug. Pick the tier from the decision tree above.
- **Features**: Write **E2E tests** (`pnpm test:e2e`), not just unit tests. For Canvas2D/layout features, also write **browser tests** (`*.browser.test.ts`).
- **Performance issues**: Always write automated reproducible tests that demonstrate the problem under realistic conditions. See `daemon/tests/input_latency.rs` and `daemon/tests/handler_starvation.rs` for patterns.
