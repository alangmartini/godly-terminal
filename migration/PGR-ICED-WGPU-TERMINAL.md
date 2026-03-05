# PGR: Iced + Custom wgpu Terminal Surface

## 0) Status Refresh (2026-03-05)

This PGR was refreshed against the current repository state (code + tests), not only prior milestone notes.

Validated locally on 2026-03-05:
- `cargo test -p godly-parity-harness`
- `cargo test -p godly-iced-shell`
- `cargo test -p godly-features-shell -p godly-app-adapter -p godly-layout-core -p godly-tabs-core -p godly-workspaces-core -p godly-terminal-surface`

Current summary:
- Native frontend path is active and `FrontendMode::Native` is the default.
- Core native shell functionality exists (sessions, tabs, splits, workspace state, daemon bridge).
- Feature parity is not complete yet (several P0/P1 items remain open).
- Release-gate artifacts referenced in earlier notes (`migration/native-release-gates.md`, `.github/workflows/native-shadow.yml`) are not present in this worktree.

## 1) Goal

Build a native Rust UI (`Iced`) with a custom on-screen `wgpu` terminal surface while:

1. Running in parallel with the current Tauri + TypeScript UI.
2. Swapping only at the end (low-risk cutover).
3. Maximizing parallel throughput for multiple agents/worktrees.

This plan assumes the current daemon/protocol stack remains the system of record.

## 2) Current Assets To Reuse

- `godly-daemon` / `godly-protocol` terminal/session core.
- `godly-vt` parser and rich grid model.
- `godly-renderer` crate (already `wgpu`, currently headless/offscreen).
- Existing persistence model and command semantics.

Primary references:
- `src-tauri/renderer/`
- `src-tauri/protocol/`
- `src-tauri/daemon/`
- `src/services/terminal-service.ts`

## 3) Migration Strategy (Parallel + Late Swap)

### 3.1 Strangler Strategy

- Keep existing web UI as production path.
- Build native UI as a separate executable path behind a frontend mode flag.
- Keep backend protocol stable so both frontends consume the same contracts.

### 3.2 Frontend Modes

- `native` (current default): Iced app.
- `web` (fallback): existing Tauri + TS UI.
- `shadow`: run native path in CI/dev for parity capture, without end-user cutover.

### 3.3 Swap Policy

Original policy: cut over to `native` only after objective gates pass (Section 8).
Current state: `native` is already default in code; Section 8 is treated as stabilization/quality target before removing fallback.
Rollback keeps `web` path intact for at least 2 release cycles.

## 4) Target Architecture

```
+----------------------+        +----------------------+
| Web Frontend (TS)    |        | Native Frontend      |
| current production   |        | Iced + wgpu surface  |
+----------+-----------+        +----------+-----------+
           |                               |
           +------------ shared ------------+
                        contracts
                   (godly-protocol)
                             |
                      godly-daemon
                             |
                        PTY sessions
```

### 4.1 Native Workspace Additions

Implemented crates (under `src-tauri/native/`):

- `src-tauri/native/iced-shell`: windows, layout shell, tab/workspace chrome.
- `src-tauri/native/terminal-surface`: on-screen terminal surface and text rendering.
- `src-tauri/native/app-adapter`: commands/events adapter to existing backend contracts.
- `src-tauri/native/parity-harness`: protocol + daemon integration parity harness.

## 5) Multi-Agent Delivery Model

Design principle: each lane has low merge conflict and clear acceptance criteria.

### Lane A: Contract Freeze + Compatibility

- Scope: protocol/event DTOs, command semantics, versioning.
- Deliverables:
  - `frontend_contract_v1.md`
  - contract tests covering current TS behavior.
- Output: stable API for all other lanes.

### Lane B: Iced App Shell

- Scope: app frame, workspaces, tab strip, split container shell, settings shell.
- Deliverables:
  - navigable native app shell with mock data and real wiring hooks.

### Lane C: wgpu Terminal Surface

- Scope: convert `godly-renderer` from offscreen pipeline to on-screen surface path.
- Deliverables:
  - steady 60fps target under normal output.
  - cursor, selection, scroll, text rendering parity baseline.

### Lane D: Input + Keybinding + IME

- Scope: keyboard mapping, dead keys/IME, clipboard, selection semantics.
- Deliverables:
  - parity test matrix for shortcuts and composition input.

### Lane E: Backend Adapter

- Scope: Rust-side adapter replacing TS service layer (`invoke/listen` equivalents).
- Deliverables:
  - strongly typed adapter with reconnect/retry/circuit breaker behavior parity.

### Lane F: Persistence + Session Restore

- Scope: layout restore/save, scrollback restore, session attach/detach parity.
- Deliverables:
  - deterministic startup and recovery tests.

### Lane G: Plugin Compatibility

- Scope: migration strategy for current JS/DOM plugin API.
- Deliverables (recommended staged plan):
  - Stage 1: keep existing plugin runtime in web mode only.
  - Stage 2: provide compatibility bridge or Rust-native plugin API for native mode.

### Lane H: QA + Parity + Perf

- Scope: cross-frontend diffing, performance benches, soak tests, release gates.
- Deliverables:
  - parity dashboard and release gate report per milestone.

## 6) Phased Plan

## Phase 0: Foundations (2 weeks) ✅ MOSTLY IMPLEMENTED

> **Status:** Contract + crate foundations are in place. Some earlier CI-gate notes need follow-up in this repo state.
>
> **Deviation:** New crates live at `src-tauri/native/` (not `src-native/`) because Cargo
> requires workspace members to be below the workspace root (`src-tauri/Cargo.toml`).

- ✅ Freeze frontend contract (`v1`) — `FRONTEND_CONTRACT_VERSION = 1.0.0`.
- ✅ Add `frontend_mode` flag and release plumbing — `FrontendMode` enum, `GODLY_FRONTEND_MODE` env var, `native-frontend` feature flag.
- ✅ Scaffold `iced-shell`, `terminal-surface`, `app-adapter` — under `src-tauri/native/`, Iced 0.14 workspace dep.
- ✅ Build parity harness skeleton — `godly-parity-harness` with contract + daemon integration tests and `GridSnapshotComparator`.
- ⚠️ CI jobs referenced in earlier notes are not currently present as active workflow files in this worktree.
- ✅ Build scripts (`pnpm build:native`, `scripts/build-native.ps1`).

Exit criteria:
- ⚠️ both frontends build in CI: not verified from current repo workflow files.
- ✅ contract tests green (current parity harness test suite passes locally).

## Phase 1: Native Terminal Vertical Slice (3 weeks) ✅ IMPLEMENTED

> **Status:** Implemented and test-green in current repo.

- Render one terminal session in native UI via daemon rich grid.
- Implement resize, input write path, scrollback pull/diff flow.
- Basic event bridge behavior implemented.

Exit criteria:
- ✅ interactive single-terminal workflow usable.
- ✅ no protocol divergence from web path.

## Phase 2: Multi-Terminal + Layout Core (3 weeks) 🟡 MOSTLY IMPLEMENTED

> **Status:** Core implemented; persistence parity remains incomplete/wiring-incomplete.

- ✅ tabs/workspaces/splits in native shell.
- ✅ focus management and pane activation.
- ⚠️ save/load layout parity is not fully closed in active app wiring.

Exit criteria:
- ✅ core daily workflow (multi-tab + split) runnable in native.
- ⚠️ restart/session restore parity still needs completion.

## Phase 3: Feature Parity Wave (4-6 weeks) 🟡 IN PROGRESS

> **Status:** Partially delivered; several parity-critical features remain open.

- ✅ shortcut plumbing, sidebar/workspace basics, split/layout core, process title updates.
- ✅ parity and native crates have broad unit/integration test coverage for implemented paths.
- ⚠️ tab rename flow is still TODO in app action handling.
- ⚠️ settings parity is partial (current runtime shows limited settings content).
- ⚠️ notification UX/audio parity is partial (state logic exists, full UX/audio pipeline not complete).
- ⚠️ several `docs/native-parity-plan.md` P0/P1 checklist items remain open.

Exit criteria:
- ⚠️ >=95% parity on critical workflow suite: not yet evidenced in the current parity harness.

## Phase 4: Shadow + Soak (2-4 weeks) 🔴 NOT COMPLETE IN CURRENT REPO STATE

> **Status:** Some performance-focused code changes exist, but release-gate artifacts/evidence are incomplete here.

- ✅ optimization work present (e.g., tab/terminal data-path improvements, render path efficiency work).
- ⚠️ shadow validation workflow artifacts referenced in prior notes are not present in current workflow directory.
- ⚠️ release gate report artifacts are not present in current migration folder.

### Deferred Optimizations (from Phase 4 Feature Parity Wave 2 review)

- [x] **HashMap for TerminalCollection** — PR #569
- [x] **Eliminate grid clone in render path** — PR #570
- [x] **Background clipboard paste** — PR #568

Exit criteria:
- ⚠️ release gates all pass for two consecutive candidate builds: not yet demonstrated in current repository artifacts.

## Phase 5: Swap + Stabilize (1-2 weeks) 🟡 PARTIALLY IMPLEMENTED

> **Status:** Mode flip is implemented; stabilization/parity closure is still ongoing.

- ✅ default frontend mode is `native`.
- ✅ rollback path via `GODLY_FRONTEND_MODE=web` remains available.
- ⚠️ parity regressions and unfinished parity stream items still need closure.

Exit criteria:
- ✅ production default switched, rollback retained.
- ⚠️ stabilization complete: pending.

## 7) Worktree/Branch Orchestration

Use one worktree per lane, plus one integration worktree.

Branch pattern:
- `feat/native-lane-a-contract`
- `feat/native-lane-b-shell`
- `feat/native-lane-c-wgpu-surface`
- ...
- `feat/native-integration`

Merge cadence:
- lane branches merge into `feat/native-integration` continuously.
- `feat/native-integration` rebased/merged to main only at milestones.

Conflict minimization:
- lane ownership by directories.
- adapter interfaces frozen per sprint.
- weekly integration hardening day.

## 8) Hard Release Gates (Stabilization Criteria)

All should pass before declaring parity-complete and removing web fallback:

1. Stability:
   - crash-free session rate at/above current baseline.
2. Performance:
   - input-to-paint p95 <= current baseline.
   - sustained output frame pacing no worse than current baseline.
3. Behavioral parity:
   - critical workflow suite pass rate >= 98%.
4. Persistence:
   - restart/session restore suite pass rate 100%.
5. Rollback:
   - mode flip back to `web` validated in production build.

## 9) Test Strategy For Parallel Development

- Contract tests:
  - command/event schemas and semantic invariants.
- Golden terminal tests:
  - same fixture output rendered and compared across web vs native.
- Input semantics tests:
  - key chords, IME/dead keys, selection/copy, zoom.
- End-to-end smoke:
  - multi-workspace, split lifecycle, reconnect, persistence.
- Performance CI:
  - cold start, steady-state memory, output throughput, input latency.

## 10) Risk Register (Top Items)

1. Plugin API mismatch (`HTMLElement`/DOM dependency).
   - Mitigation: Stage 1 compatibility policy and explicit native-mode scope.
2. Input parity (dead keys/IME/shortcuts).
   - Mitigation: dedicate Lane D and make it a hard gate.
3. On-screen `wgpu` rendering complexity.
   - Mitigation: reuse `godly-renderer` core and iterate from offscreen to surface.
4. Contract drift between frontends.
   - Mitigation: Lane A ownership + mandatory contract tests.
5. Merge thrash with many agents.
   - Mitigation: lane directory ownership + integration branch + weekly hardening.

## 11) Recommended Team Shape

- 5 active lanes in parallel (max) for steady velocity:
  - A (contract), C (wgpu), E (adapter), B (shell), H (parity/perf).
- Then bring in D/F/G as wave-2 once vertical slice is stable.

## 12) Current Backlog (Execution Focus)

Priority order is aligned with open P0/P1 parity blockers from `docs/native-parity-plan.md`:

1. ✅ B1: tab drag-drop reorder in tab bar.
2. ✅ B3/B4: drag tab to split zones / sidebar workspace move.
3. ✅ E1/E5: complete tab rename and tab context menu.
4. ✅ D2/D3/D4: workspace notification indicators + audible notifications + presets.
5. ✅ A4: scrollback restoration parity on reconnect (code/test complete).
6. ✅ B5: tab overflow horizontal scrolling in tab bar (with captured wheel isolation).
7. ✅ B6: file drag-drop onto focused terminal (quoted path insertion).
8. ✅ L13: sidebar resize handle with bounded interactive width.
9. ✅ D5: per-workspace mute patterns for notification audio.
10. ✅ C5: Quick Claude settings tab with runtime preset editor.
11. ✅ C6: AI Tools settings tab with runtime tool registry editor.
12. ✅ D7: native taskbar attention flash with focus/debounce gating.
13. ✅ L10: sidebar header icon polish (settings/new-workspace icons).
14. ✅ L18/L19: settings modal + tab strip visual polish.
15. ✅ L20/L21/L22: shortcuts tab spacing, control styling, and vertical-only scroll polish.
16. ⚠️ E6: MRU keyboard cycling (Ctrl+Tab / Ctrl+Shift+Tab) is implemented; visual popup switcher remains pending until popup lane lands.

## 13) Completed Step: D2/D3/D4 Workspace Notifications + Audio

Goal:
- Close notification parity by surfacing workspace-level indicators and delivering audible notifications with configurable presets.

Scope:
- `src-tauri/native/iced-shell/src/sidebar.rs`
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/notification_state.rs`
- `src-tauri/native/app-adapter/src/` (audio playback bridge if needed)
- `src-tauri/native/features-shell/src/workspaces.rs` (only if reducer extensions are needed)

Implementation steps:
1. Add workspace notification indicator rendering in sidebar rows (D2), keyed from terminal unread/bell state.
2. Introduce audio playback path for bell events (D3) with debounce preserved.
3. Add sound preset selection + routing (D4), wiring settings state to playback backend.
4. Verify notification state transitions across workspace switches and tab focus changes.

Acceptance criteria:
- Inactive workspaces with unread/bell activity show a visible indicator in sidebar.
- Bell events play audio according to selected preset and mute settings.
- Notification behavior remains deterministic under rapid output/bell events.

Validation:
- `cargo test -p godly-tabs-core -p godly-features-shell -p godly-iced-shell`
- Manual smoke:
  1. Trigger output/bell on non-focused terminal and verify sidebar indicators.
  2. Switch sound presets and validate playback differences.
  3. Verify mute/debounce behavior under rapid repeated bells.

## 14) Completed Step: A4 Scrollback Restoration Parity On Reconnect

Goal:
- Restore scrollback position and scrollback-related state deterministically after reconnect/session recovery so native behavior matches web parity expectations.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/terminal_state.rs`
- `src-tauri/native/features-shell/src/` (only if reducer support is needed)
- reconnect/recovery-related tests in `src-tauri/native/iced-shell/src/` and `src-tauri/native/features-shell/tests/`

Implementation steps:
1. Persist/rehydrate terminal scrollback offset and related fields during recovery paths.
2. Ensure recovered sessions fetch and apply grid state without dropping prior scroll position metadata.
3. Add reducer/UI tests for reconnect + restore flows (single and multi-session cases).
4. Verify behavior under rapid reconnect and workspace switch sequences.

Acceptance criteria:
- Recovered sessions restore expected scrollback offset and total scrollback metadata.
- Switching workspaces/tabs after recovery does not reset restored scrollback unexpectedly.
- Behavior is deterministic across repeated reconnect cycles.

Validation:
- `cargo test -p godly-features-shell -p godly-iced-shell`
- Manual smoke:
  1. Scroll up in a terminal, reconnect/restart native shell, verify offset restoration.
  2. Validate restoration for multiple recovered sessions/workspaces.

Status update (2026-03-05):
- Implemented persisted scrollback offset save/load and recovery-time restore (`scroll_fetch`) for recovered sessions.
- Added focused reconnect restoration tests:
  - mixed recovery fetch plan (`fetch_grid` vs `scroll_fetch`),
  - stale recovered session filtering,
  - stale persistence pruning against live session IDs.
- Added backend-clamp persistence hygiene and positive-offset-only persistence to avoid unbounded offset file growth.

## 15) Completed Step: B6 File Drag-Drop Onto Terminal

Goal:
- Support dropping files/folders onto terminal panes and send shell-safe quoted paths to the focused terminal.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/app-adapter/src/commands.rs` (if additional helper needed)
- `src-tauri/native/iced-shell/src/` tests for drop event routing/quoting behavior

Implementation steps:
1. Capture OS file drop events in native shell and route them to the focused terminal session.
2. Quote/escape dropped paths safely for Windows shell command insertion.
3. Write quoted path payload to PTY input via existing command bridge.
4. Add tests for path quoting (spaces, unicode, special chars) and no-focused-terminal noop behavior.

Acceptance criteria:
- Dropping one or multiple files pastes quoted paths into the focused terminal input.
- Paths with spaces/special characters are correctly escaped.
- No crash/regression when drop occurs with no focused terminal.

Validation:
- `cargo test -p godly-iced-shell -p godly-app-adapter`
- Manual smoke:
  1. Drop a path with spaces onto focused terminal and confirm correct text insertion.
  2. Drop multiple files and confirm delimiter/quoting correctness.

Status update (2026-03-05):
- Wired `window::Event::FileDropped` handling in native app subscription.
- Added focused-terminal PTY write path using quoted drop payloads.
- Added path quoting unit tests (spaces, unicode, embedded double-quotes).

## 16) Completed Step: L13 Sidebar Resize Handle

Goal:
- Add a visible sidebar resize affordance and allow adjusting sidebar width interactively with sane bounds.

Scope:
- `src-tauri/native/iced-shell/src/sidebar.rs`
- `src-tauri/native/iced-shell/src/app.rs`
- potential reducer/helper additions if needed

Implementation steps:
1. Add sidebar width state in app and feed it into sidebar rendering.
2. Add drag handle UI element at sidebar boundary.
3. Handle drag updates to resize width with min/max clamping.
4. Add tests for clamp behavior and no-regression rendering.

Acceptance criteria:
- User can drag sidebar boundary to resize width.
- Width remains within configured min/max.
- Existing sidebar actions/context menus continue working.

Validation:
- `cargo test -p godly-iced-shell`
- Manual smoke:
  1. Drag sidebar handle left/right and verify clamped width updates.
  2. Verify workspace click/context menu still functions after resize.

Status update (2026-03-05):
- Added bounded sidebar resize constants and helper (`SIDEBAR_MIN_WIDTH`, `SIDEBAR_MAX_WIDTH`, `clamp_sidebar_width`).
- Updated `view_sidebar` to accept runtime width and emit resize start/end messages from the boundary handle.
- Added app-level `sidebar_width` + drag state, with clamp-on-cursor-move and resize dispatch on drag end.
- Updated column/grid math (`calculate_cols`, `pixel_to_grid`) to use dynamic sidebar width.
- Added sidebar clamp unit coverage and kept full native shell test suite green.

## 17) Completed Step: D5 Per-Workspace Mute Patterns

Goal:
- Implement configurable mute patterns so selected workspaces suppress audible notifications while preserving visual indicators.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/sidebar.rs` (if workspace-level mute affordance is added)
- `src-tauri/native/iced-shell/src/settings_dialog.rs` / notifications settings tab wiring
- `src-tauri/native/app-adapter/src/sound.rs` (only if additional preset/mute helpers are required)

Implementation steps:
1. Add mute pattern state and matcher helper (glob-like matching against workspace names/ids).
2. Integrate matcher into notification sound path so muted workspaces skip audio playback.
3. Add settings UI controls to add/remove patterns and persist values in app state.
4. Add tests for matching semantics and mute/no-mute behavior across workspace switches.

Acceptance criteria:
- Bell events from muted workspaces do not play sound.
- Non-muted workspaces still play sound under existing debounce rules.
- Existing notification badges remain visible regardless of mute status.

Validation:
- `cargo test -p godly-iced-shell -p godly-app-adapter -p godly-features-shell`
- Manual smoke:
  1. Add a mute pattern and trigger bells in matching/non-matching workspaces.
  2. Confirm visual indicators continue while muted sounds are suppressed.

Status update (2026-03-05):
- Added wildcard mute-pattern matching (`*`) with normalized case-insensitive comparisons.
- Integrated mute matching into bell sound playback so matching workspaces suppress audio while keeping existing debounce logic.
- Added notifications-tab controls for adding/removing mute patterns at runtime.
- Added helper tests for pattern normalization/matching and workspace mute matching behavior.

## 18) Completed Step: C5 Quick Claude Tab

Goal:
- Add the first remaining settings-parity tab by introducing a Quick Claude configuration surface in native settings.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/settings_dialog.rs`
- `src-tauri/native/app-adapter/src/` (only if persistence/command bridge additions are needed)

Implementation steps:
1. Add a `quick-claude` settings tab entry and route content rendering from settings state.
2. Build initial preset form (name + prompt template + layout mode).
3. Add actions to create/edit/delete local presets in app state.
4. Add tests for tab rendering path and preset state mutations.

Acceptance criteria:
- Quick Claude tab is visible and selectable in settings.
- User can add/update/remove presets without crashing or losing current session state.
- Existing settings tabs remain unaffected.

Validation:
- `cargo test -p godly-iced-shell`
- Manual smoke:
  1. Open settings, switch to Quick Claude tab, create/edit/delete a preset.
  2. Reopen settings and verify state consistency for the current runtime session.

Status update (2026-03-05):
- Added a new `quick-claude` settings tab entry in native settings.
- Implemented in-app Quick Claude preset editor (name/prompt/layout) with add/update/delete/clear flows.
- Added reusable layout enum (`Single`, `VSplit`, `HSplit`, `2x2`) and wired selection controls.
- Added helper tests for Quick Claude input normalization while preserving full shell test stability.

## 19) Completed Step: C6 AI Tools Tab

Goal:
- Add AI Tools settings parity by allowing users to register named tool entries with launch metadata.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/settings_dialog.rs`
- `src-tauri/native/app-adapter/src/` (if persistence/launch wiring is required)

Implementation steps:
1. Add `ai-tools` tab entry and content routing in settings.
2. Implement runtime list editor for tool entries (display name + command + optional icon tag).
3. Add add/update/delete flows with input validation.
4. Add tests for editor state transitions and validation behavior.

Acceptance criteria:
- AI Tools tab is visible and interactive in settings.
- User can create/edit/delete tool definitions in current runtime state.
- Existing settings tabs (Shortcuts/Notifications/Quick Claude) remain unaffected.

Validation:
- `cargo test -p godly-iced-shell`
- Manual smoke:
  1. Add/edit/remove an AI tool entry in settings.
  2. Reopen settings and verify runtime state consistency.

Status update (2026-03-05):
- Added a new `ai-tools` settings tab entry and content routing.
- Implemented in-app AI tool registry editor with add/update/delete/clear flows.
- Added helper normalization for AI tool fields and unit coverage.
- Preserved stability across existing settings tabs and native shell test matrix.

## 20) Completed Step: C7 Plugins Tab

Goal:
- Add Plugins settings parity with baseline plugin inventory management controls.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/settings_dialog.rs`
- `src-tauri/native/app-adapter/src/` (if plugin bridge calls are introduced)

Implementation steps:
1. Add `plugins` tab entry and rendering route in settings.
2. Implement local plugin list model with enable/disable and remove actions.
3. Add add/install input flow (name + source path/URL placeholder).
4. Add tests for plugin list mutations and editor validation.

Acceptance criteria:
- Plugins tab is visible and interactive.
- User can add/enable/disable/remove plugin entries in runtime state.
- Existing settings tabs remain unaffected.

Validation:
- `cargo test -p godly-iced-shell`
- Manual smoke:
  1. Add a plugin entry, toggle enabled state, then remove it.
  2. Verify other settings tabs keep their state and behavior.

Status update (2026-03-05):
- Added `plugins` tab routing in native settings.
- Implemented runtime plugin inventory editor with add/enable/disable/remove actions.
- Added focused plugin-tab mutation coverage while preserving existing shell test stability.

## 21) Completed Step: C8 Flows Tab

Goal:
- Add Flows settings parity with a baseline automation flow editor in native settings.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/settings_dialog.rs`
- `src-tauri/native/app-adapter/src/` (if flow execution bridge wiring is introduced)

Implementation steps:
1. Add `flows` tab entry and rendering route in settings.
2. Implement runtime flow list/editor state with create/edit/delete actions.
3. Add validation for required flow fields and deterministic state transitions.
4. Add tests for flow editor mutations and tab isolation behavior.

Acceptance criteria:
- Flows tab is visible and interactive.
- User can create/edit/delete flow entries in runtime state.
- Existing settings tabs remain unaffected.

Validation:
- `cargo test -p godly-iced-shell`
- Manual smoke:
  1. Add/edit/remove a flow entry in settings.
  2. Reopen settings and verify runtime flow state consistency.

Status update (2026-03-05):
- Added `flows` tab routing in native settings.
- Implemented runtime flow editor state with add/update/delete/clear actions.
- Added focused flow-tab mutation coverage without regressions in existing settings paths.

## 22) Completed Step: C9 Remote SSH Tab

Goal:
- Add Remote SSH settings parity for editing SSH connection defaults in native settings.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/settings_dialog.rs`
- `src-tauri/native/app-adapter/src/` (if remote settings persistence/launch bridge wiring is introduced)

Implementation steps:
1. Add `remote` tab entry and rendering route in settings.
2. Implement runtime SSH settings editor fields and update handlers.
3. Add validation/normalization for remote SSH settings input.
4. Add tests for remote settings mutations and cross-tab isolation behavior.

Acceptance criteria:
- Remote SSH tab is visible and interactive.
- User can view/edit SSH connection settings in runtime state.
- Existing settings tabs remain unaffected.

Validation:
- `cargo test -p godly-iced-shell`
- Manual smoke:
  1. Update SSH settings in Remote tab and verify runtime state updates.
  2. Switch across settings tabs and confirm no state regression.

Status update (2026-03-05):
- Added `remote` tab routing in native settings.
- Implemented runtime Remote SSH settings editor handlers in native app state.
- Added focused remote-tab mutation coverage while preserving shell test stability.

## 23) Completed Step: D6 Toast Notification Overlay

Goal:
- Close D6 parity by adding a native toast notification overlay with deterministic auto-dismiss behavior.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/notification_state.rs`
- `src-tauri/native/iced-shell/src/tab_bar.rs` and `src-tauri/native/iced-shell/src/sidebar.rs` (toast trigger wiring as needed)

Implementation steps:
1. Add toast queue/state model with per-toast TTL and 4s auto-dismiss.
2. Render overlay-level toast UI above terminal content without breaking input focus behavior.
3. Wire notification events into toast enqueue path, preserving debounce semantics.
4. Add tests for enqueue/dequeue/timeout behavior and repeated notification bursts.

Acceptance criteria:
- Toast appears for notification events and auto-dismisses after ~4 seconds.
- Repeated events queue predictably without visual overlap regressions.
- Overlay does not block core terminal interactions.

Validation:
- `cargo test -p godly-iced-shell`
- `cargo test -p godly-iced-shell -p godly-features-shell -p godly-tabs-core -p godly-app-adapter`
- Manual smoke:
  1. Trigger notification/bell events and confirm toast appears and auto-dismisses.
  2. Trigger rapid repeated events and verify debounce + queue behavior remains stable.

Status update (2026-03-05):
- Added toast queue state, monotonic toast ids, and bounded active-toast retention.
- Added 4s auto-dismiss behavior via periodic subscription tick and deterministic pruning.
- Added overlay toast renderer in native shell view stack and hooked bell notifications into toast enqueue for non-focused terminals.
- Added helper tests for queue bounding and expiration behavior while preserving existing native shell test stability.

## 24) Completed Step: D7 Native Windows Taskbar Flash

Goal:
- Close D7 parity by flashing the native Windows taskbar on notification when the app is unfocused.

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/notifications.rs`

Implementation summary:
1. Added explicit window focus tracking and window id capture in native shell state.
2. Added gated attention request path on bell notifications (`window::request_user_attention`) for unfocused windows only.
3. Added deterministic debounce for repeated attention requests during bursty bell streams.
4. Added pure helper tests for focus/debounce gating and platform-critical attention decision behavior.

Acceptance criteria:
- Bell notification while unfocused triggers a visible taskbar flash on Windows.
- Focused-window notifications do not trigger taskbar flash.
- Repeated bursts remain bounded and do not spam platform attention requests.

Validation:
- `cargo test --manifest-path src-tauri/Cargo.toml -p godly-iced-shell -p godly-app-adapter`
- `cargo test --manifest-path src-tauri/Cargo.toml -p godly-iced-shell -p godly-features-shell -p godly-tabs-core -p godly-app-adapter`

Status update (2026-03-05):
- D7 attention flow landed with deterministic debounce and focus gating.
- Native shell + parity reducer package test matrix is green.

## 25) Completed Batch: L10 + L18/L19 + L20/L21/L22 + E6 Foundation

Goal:
- Close UI polish parity items and start E6 MRU parity in reducer/state foundations.

Scope:
- `src-tauri/native/iced-shell/src/sidebar.rs`
- `src-tauri/native/iced-shell/src/settings_dialog.rs`
- `src-tauri/native/iced-shell/src/shortcuts_tab.rs`
- `src-tauri/native/iced-shell/src/terminal_state.rs`
- `src-tauri/native/features-shell/src/tabs.rs`

Implementation summary:
1. L10: replaced text glyph controls with canvas-drawn sidebar header icons and reusable button styling.
2. L18/L19: refreshed settings modal/tab-strip visuals with stronger affordances and horizontal tab overflow support.
3. L20/L21/L22: improved shortcuts tab spacing, key-badge styling, section card presentation, and vertical-only scroll behavior.
4. E6 foundation: added MRU reducer primitives and `TerminalCollection` MRU tracking helpers with coverage for touch/cleanup/cycle semantics.

Validation:
- `cargo test --manifest-path src-tauri/Cargo.toml -p godly-iced-shell -p godly-features-shell -p godly-tabs-core -p godly-app-adapter`

Status update (2026-03-05):
- Batch landed and is test-green after fixing `iced::Pixels` spacing type usage in `shortcuts_tab`.
- E6 foundation from this batch is now extended by the keyboard-semantic completion captured in Section 26.

## 26) Completed Step: E6 MRU Keyboard Semantics (Popup Pending)

Goal:
- Close the keyboard-semantic portion of E6 by wiring MRU behavior into tab switching shortcuts (Ctrl+Tab / Ctrl+Shift+Tab parity).

Scope:
- `src-tauri/native/iced-shell/src/app.rs`
- `src-tauri/native/iced-shell/src/terminal_state.rs`
- `src-tauri/native/features-shell/src/tabs.rs` (only if reducer adjustments are needed)

Implementation steps:
1. Route tab-cycle shortcut handling through MRU reducers/state instead of strict visual tab order.
2. Ensure close/create/workspace-switch flows keep MRU deterministic and free of stale ids.
3. Add focused tests in `app.rs` or reducer-flow suites for forward/backward MRU cycling and wrap behavior.
4. Validate no regression in existing tab reorder, split, and rename flows.

Acceptance criteria:
- Ctrl+Tab and Ctrl+Shift+Tab follow MRU order rather than static index order.
- MRU list remains consistent after tab close/new/split/workspace moves.
- Existing tab navigation and layout flows remain green.
- Visual popup switcher UX is explicitly out of scope for this step and remains pending until popup lane lands.

Validation:
- `cargo test --manifest-path src-tauri/Cargo.toml -p godly-iced-shell -p godly-features-shell -p godly-tabs-core`
- Manual smoke:
  1. Open 3+ tabs, activate in custom sequence, verify MRU cycle order.
  2. Close a mid-MRU tab and verify cycle skips stale entries.
  3. Create/split/move tabs and verify MRU updates remain stable.

Status update (2026-03-05):
- Wired `AppAction::NextTab` / `AppAction::PreviousTab` to MRU cycling in native app action dispatch.
- Added helper tests for MRU forward/backward/wrap and missing-current-id handling.
- E6 visual popup switcher remains pending until popup lane lands.

