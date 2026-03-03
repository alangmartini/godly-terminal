# PGR: Iced + Custom wgpu Terminal Surface

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

- `web` (default): existing Tauri + TS UI.
- `native`: Iced app.
- `shadow`: run native path in CI/dev for parity capture, without end-user cutover.

### 3.3 Swap Policy

Cutover to `native` only after objective gates pass (Section 8).
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

Proposed crates:

- `src-native/iced-shell`: windows, layout shell, tab/workspace chrome.
- `src-native/terminal-surface`: on-screen `wgpu` terminal rendering/input.
- `src-native/app-adapter`: commands/events adapter to existing backend contracts.
- `src-native/parity-harness`: snapshot and behavior parity assertions vs web UI.

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

## Phase 0: Foundations (2 weeks) ✅ IMPLEMENTED

> **Status:** Complete — PR #542 (merged from `feat/phase0-iced-wgpu-foundations`), tracking issue #541.
>
> **Deviation:** New crates live at `src-tauri/native/` (not `src-native/`) because Cargo
> requires workspace members to be below the workspace root (`src-tauri/Cargo.toml`).

- ✅ Freeze frontend contract (`v1`) — `docs/frontend_contract_v1.md`, `FRONTEND_CONTRACT_VERSION` constant.
- ✅ Add `frontend_mode` flag and release plumbing — `FrontendMode` enum, `GODLY_FRONTEND_MODE` env var, `native-frontend` feature flag.
- ✅ Scaffold `iced-shell`, `terminal-surface`, `app-adapter` — under `src-tauri/native/`, Iced 0.13 workspace dep.
- ✅ Build parity harness skeleton — `godly-parity-harness` with 11 contract tests + `GridSnapshotComparator`.
- ✅ CI jobs added (`native-build`, `contract-tests`).
- ✅ Build scripts (`pnpm build:native`, `scripts/build-native.ps1`).

Exit criteria:
- ✅ both frontends build in CI.
- ✅ contract tests green (11/11 pass).

## Phase 1: Native Terminal Vertical Slice (3 weeks)

- Render one terminal session in native UI via daemon rich grid.
- Implement resize, input write path, scrollback pull/diff flow.
- Basic latency instrumentation.

Exit criteria:
- interactive single-terminal workflow usable.
- no protocol divergence from web path.

## Phase 2: Multi-Terminal + Layout Core (3 weeks)

- tabs/workspaces/splits in native shell.
- focus management and pane activation.
- save/load layout parity.

Exit criteria:
- core daily workflow (multi-tab + split) fully runnable in native.

## Phase 3: Feature Parity Wave (4-6 weeks)

- shortcuts, notifications, settings, process title updates, recovery flows.
- close behavioral gaps called out by parity harness.
- plugin plan Stage 1 implemented (explicitly gated behavior in native mode).

Exit criteria:
- >=95% parity on critical workflow suite.

## Phase 4: Shadow + Soak (2-4 weeks)

- internal dogfooding in `native` and `shadow` validation in CI.
- performance burn-in and memory/leak checks.
- release candidate with rollback flag.

Exit criteria:
- release gates all pass for two consecutive candidate builds.

## Phase 5: Swap + Stabilize (1-2 weeks)

- default frontend mode to `native`.
- keep `web` fallback for 2 release cycles.
- prioritize parity regressions and crash triage.

Exit criteria:
- production default switched, rollback retained.

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

## 8) Hard Release Gates (Cutover Criteria)

All must pass before defaulting to native:

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

## 12) First 2 Weeks Backlog (Execution Starter)

1. Define `frontend_contract_v1.md` from current TS service usage.
2. Add `frontend_mode` config and packaging hooks.
3. Scaffold `src-native/iced-shell`, `src-native/terminal-surface`, `src-native/app-adapter`.
4. Port one session read/write/resize loop end-to-end in native app.
5. Build parity harness stub that can compare web/native grid snapshots.
6. Add CI jobs:
   - native build
   - contract tests
   - minimal native smoke test

