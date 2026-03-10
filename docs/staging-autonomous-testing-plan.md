# Godly Staging Autonomous Testing Plan

## Summary

Implementation plan for a staging-only test architecture that lets an AI agent test Godly Terminal autonomously with deterministic pass/fail results.

Goals:
1. Autonomous control of all app behavior needed for testing
2. Machine-readable observability for feature state and outcomes
3. Deterministic pass/fail oracles for every supported feature
4. Crash-safe artifact capture so failures remain debuggable
5. Frontend-agnostic coverage for both WebView and native Iced frontends

## Motivation

The current stack has MCP tools, `execute_js`, PyAutoGUI automation, and staging isolation -- but four structural gaps remain:

1. `execute_js` is too generic and WebView-specific
2. `capture_screenshot` only works on visible canvases -- unreliable as an oracle
3. No first-class test harness mode (reset, fixtures, restart, artifact bundling)
4. No shared semantic API across web and native Iced frontends

## Scope

**In scope:** Godly Staging only, web + native frontends, MCP-facing tooling, deterministic assertions, fixtures, restart flows, artifact capture.

**Out of scope:** Production test hooks, replacing existing unit/browser/integration/e2e tests, LLM-judges-screenshots as primary oracle.

## Success Criteria

- Agent can launch/connect staging in test-harness mode
- Agent can reset to a clean profile and load named fixtures
- Agent can drive features through typed semantic actions (not raw DOM scripts)
- Agent can query typed app/layout/focus/terminal/persistence state
- Agent can snapshot any terminal/pane even when hidden
- Test runs produce artifact bundles (logs, event traces, snapshots, assertions, crash evidence)
- Same test contract runs on web and native frontends

## Core Principles

1. **Deterministic oracle first** -- Structured state assertions are primary. Visual comparison is secondary unless the feature is explicitly visual.
2. **Semantic driver first** -- Control via `ui.act(target, action)` / `ui.query(target)` / `ui.wait(condition)`, not `execute_js` or raw coordinates.
3. **External runner always** -- Runner lives outside the app process. If the app crashes/hangs, the runner still captures artifacts and reports failure.
4. **Frontend-agnostic contracts** -- Test contracts are identical for web and native. Frontend-specific details live in adapters.
5. **Staging-only privileges** -- No production backdoors.

## Target Architecture

```text
AI Agent / Runner
        |
        v
Typed MCP Test Tools
        |
        v
Staging Test Harness Service
        |
        +---------------------+
        |                     |
        v                     v
Web Adapter             Native Adapter
DOM/store bridge        Iced state/action bridge
        |                     |
        +----------+----------+
                   |
                   v
         App State + Daemon State
                   |
                   v
            Artifact Collector
```

## Major Components

### 1. Test Harness Mode

Staging boots in `test-harness` mode with: clean profile, test-only RPC, graceful shutdown/restart, fixture loading, health/crash reporting, on-demand log flushing. All behind staging/test feature gates.

Capabilities: `launch_app`, `connect_app`, `graceful_quit`, `graceful_restart`, `reset_profile`, `load_fixture`, `wait_until_ready`, `export_state_dump`, `collect_artifact_bundle`.

### 2. Semantic Automation API

Typed API for actions, queries, and waits. Narrow and explicit -- no generic "run arbitrary code" escape hatch.

Operations: `ui.act`, `ui.query`, `ui.wait`, `app.lifecycle`, `workspace.query/act`, `terminal.query/act`, `layout.query`, `settings.query/act`, `render.snapshot`, `metrics.query`.

Request/response shape:
```json
{"target": "workspace.sidebar.toggle", "action": "click", "args": {}}
{"ok": true, "target": "workspace.sidebar.toggle", "action": "click", "timestamp_ms": 1760000000000}
```

### 3. Semantic IDs

Every testable UI surface gets a stable semantic ID. IDs must not depend on CSS selectors, DOM depth, or pixel coordinates. Same ID means the same thing in web and native.

Examples: `workspace.sidebar.toggle`, `workspace.active`, `tab.add`, `tab.active`, `tab.close:<id>`, `pane.active`, `pane.divider:<id>`, `terminal.surface:<id>`, `settings.dialog`, `quick-claude.prompt`.

### 4. Frontend Adapters

Two adapters behind the same protocol:

- **Web adapter**: Resolves semantic IDs to DOM/store operations. Uses `window.__STORE__` internally but callers don't know.
- **Native adapter**: Resolves semantic IDs to Iced app state and message dispatch.

Both report layout bounds, visibility, typed state, and perform semantic actions.

### 5. Observability Plane

Machine-readable exports for: app info, active frontend, workspace/terminal lists, layout tree, pane focus, split ratios, terminal metadata/grid/scroll, persistence metadata, daemon sessions, notification settings.

Event streams: workspace switched, terminal created/focused, split changed, notification changed, process changed, persistence save/restore, app ready/shutdown/crash.

Perf metrics: key-to-grid latency percentiles, snapshot fetch latency, output backlog, bridge queue depth, render time, dropped frames.

### 6. Render Snapshot Layer

Any terminal or pane can be snapshotted even when hidden/backgrounded. Types: terminal grid as text, terminal rich snapshot as structured cells, pane render as image, window snapshot as image.

### 7. Artifact Bundle

Every run produces a bundle on failure (optional on success):
```text
artifacts/<run-id>/
  manifest.json, result.json, assertions.json, app-state.json,
  layout-tree.json, daemon-sessions.json, event-trace.jsonl,
  metrics.json, screenshots/, terminal-grids/,
  frontend.log, godly-daemon-debug.log, godly-bridge-debug.log, crash.txt
```
Crash evidence survives restarts. Bundle path returned to caller.

### 8. Feature Contracts

Each testable feature has a contract in `testing/contracts/*.json`:
```json
{
  "id": "split-basic",
  "description": "Create a split, move focus, resize, and verify persistence.",
  "frontends": ["web", "native"],
  "fixture": "two-terminals",
  "requires_restart": true,
  "steps": [],
  "assertions": [],
  "cleanup": []
}
```
Definition of done for a new feature: has a contract, fixture coverage, semantic IDs, structured state queries, and a deterministic assertion path.

### 9. Fixture System

Fixtures produce known-good starting states: clean profile, single workspace/terminal, two terminals, two-pane split, multi-tab, quick-claude workspace, scrollback-heavy, restored persistence session. Each fixture supports create, verify-ready, teardown, and idempotent reset.

### 10. External Runner

TypeScript runner (repo already has Node/Vitest tooling). Responsibilities: start/attach staging, select frontend, reset profile, load fixture, execute contract, capture artifacts continuously, fail on crash/timeout, clean up, print PASS/FAIL. No retry loops that mask failures.

### 11. OS Automation Fallback

PyAutoGUI remains useful for native drag-and-drop, divider drags, keyboard shortcuts, window focus, and context menus that can't be addressed semantically. Not for primary assertions or locating UI via screenshots when a semantic query exists. Pattern: query semantic bounds, convert to window coordinates, perform OS action, verify via semantic state.

## Repo Layout

```text
testing/
  contracts/          # Feature contract JSON files
  fixtures/           # Fixture scripts (clean-profile.ts, two-terminals.ts, etc.)
  runner/             # External runner (index.ts, contract-runner.ts, artifact-bundle.ts, assertions.ts)

src/testing/          # Frontend test adapters
  semantic-ids.ts, web-adapter.ts, test-harness-bridge.ts

src-tauri/
  protocol/src/testing.rs             # Shared protocol types
  src/commands/testing.rs             # Tauri commands
  src/testing/harness.rs, artifacts.rs, state_dump.rs
  mcp/src/tools.rs                    # MCP tool additions
  native/iced-shell/src/testing/adapter.rs, semantic_ids.rs
```

## Planned MCP Tools

**Lifecycle:** `launch_staging_app`, `connect_staging_app`, `graceful_quit_app`, `graceful_restart_app`, `reset_staging_profile`, `wait_for_app_ready`

**Fixtures/contracts:** `list_test_contracts`, `run_test_contract`, `load_test_fixture`, `cleanup_test_fixture`

**Semantic control:** `ui_act`, `ui_query`, `ui_wait`

**State/artifacts:** `export_app_state`, `export_layout_tree`, `export_event_trace`, `capture_render_snapshot`, `capture_window_snapshot`, `collect_artifact_bundle`

**Metrics:** `get_test_metrics`

## Task List

Execute in order. Each task ends with a runnable verification step. Ship thin end-to-end slices before widening. Everything is staging/test gated.

### Phase 0: Harness Foundation

- [~] **P0.1** Add staging test-harness mode switch (`main.rs`, `lib.rs`, `Cargo.toml`). App reports harness mode in both frontends.
- [~] **P0.2** Define shared protocol types for testing operations (`protocol/src/testing.rs`). Web and native compile against same types.
- [~] **P0.3** Create backend test harness service skeleton (`src/testing/harness.rs`). Answers readiness/health, tracks run metadata, manages fixture/reset lifecycle.
- [~] **P0.4** Add artifact bundle infrastructure (`src/testing/artifacts.rs`). Each run gets a run ID, artifact directory, manifest, and append-only writes.
- [~] **P0.5** Add state dump infrastructure (`src/testing/state_dump.rs`). Exports app state, layout, and daemon metadata in one call.
- [~] **P0.6** Add Tauri commands for test-harness lifecycle (`src/commands/testing.rs`, `lib.rs`).
- [~] **P0.7** Add MCP tools for phase-0 lifecycle (`tools.rs`, `handler.rs`). Agent can connect, request state dump, and get artifact bundle path.
- [~] **P0.8** Implement graceful quit and restart for staging. Uses real persistence quit path, not process-kill.
- [~] **P0.9** Implement profile reset. Clean isolated staging profile without touching production data.
- [~] **P0.10** Add one smoke test for harness connectivity (connect, reset, ready, collect artifacts).

### Phase 1: Web Semantic API

- [~] **P1.1** Define semantic ID registry (`src/testing/semantic-ids.ts`).
- [~] **P1.2** Implement web adapter query surface (`web-adapter.ts`). Resolves semantic IDs to typed state/layout data.
- [~] **P1.3** Implement web adapter action surface. Semantic actions for create/focus terminal, split, switch workspace, open settings.
- [~] **P1.4** Implement web adapter wait surface. Supports timeout, polling, structured timeout errors.
- [~] **P1.5** Expose web adapter through test-harness bridge (`test-harness-bridge.ts`).
- [~] **P1.6** Add `ui_act`/`ui_query`/`ui_wait` MCP tools.
- [~] **P1.7** Add typed state export tools for core domains.
- [~] **P1.8** Create first web-only vertical slice contract (`split-basic`).

### Phase 2: Contracts, Fixtures, and Runner

- [ ] **P2.1** Define contract schema (`testing/contracts/schema.json`). Invalid contracts fail validation before execution.
- [ ] **P2.2** Define assertion model (`testing/runner/assertions.ts`). Supports equality, contains, regex, bounds, existence, metric thresholds.
- [ ] **P2.3** Build fixture loader API (`testing/fixtures/*.ts`).
- [ ] **P2.4** Implement external runner (`testing/runner/`). Prints PASS/FAIL, exits non-zero on failure.
- [ ] **P2.5** Implement artifact-aware contract execution. Every step/assertion recorded in bundle during execution.
- [ ] **P2.6** Seed initial fixtures (clean-profile, two-terminals, split-basic, restored-session).
- [ ] **P2.7** Seed initial contracts (terminal-crud, workspace-switch, split-basic, persistence-restart).
- [ ] **P2.8** Add `run_test_contract` MCP tool.

### Phase 3: Render Oracles and Metrics

- [ ] **P3.1** Terminal grid snapshot export to artifact bundles.
- [ ] **P3.2** Offscreen terminal render snapshots (hidden/background terminals).
- [ ] **P3.3** Pane/window render snapshots.
- [ ] **P3.4** Event journal (`event-trace.jsonl` with timestamps).
- [ ] **P3.5** Perf metric capture (latency percentiles, queue depths).
- [ ] **P3.6** Crash/hang classification (structured `failure_type` in results).

### Phase 4: Native Frontend Parity

- [ ] **P4.1** Define native semantic ID registry (`iced-shell/src/testing/semantic_ids.rs`).
- [ ] **P4.2** Implement native query adapter.
- [ ] **P4.3** Implement native action adapter.
- [ ] **P4.4** Implement native wait/snapshot surface.
- [ ] **P4.5** Add parity runs -- one command runs same contract against web and native.

### Phase 5: Agent UX and Doc Migration

- [ ] **P5.1** Freeze MCP tool names and output shapes.
- [ ] **P5.2** Update `docs/mcp-testing.md` to contract-based workflow.
- [ ] **P5.3** Update `.claude/skills/manual-testing.md` to prefer harness when available.
- [ ] **P5.4** Add example prompts and slash-command conventions.
- [ ] **P5.5** Add one end-to-end acceptance demo contract.

## PR Breakdown

1. Harness mode, protocol skeleton, lifecycle commands, artifact bundle shell
2. Web semantic IDs, `ui_query`/`ui_act`/`ui_wait`, first state exports
3. Contract schema, fixture loader, runner, `split-basic` web contract
4. Offscreen snapshots, event journal, metrics export
5. Native adapter parity for core contracts
6. Doc migration, manual-testing skill update, agent-facing command polish

## First Vertical Slice

Ship this before broadening:
1. Boot staging in test-harness mode
2. Reset profile
3. Load `two-terminals` fixture
4. Execute `ui_act(create_split)`
5. Verify split tree via `ui_query`
6. Capture artifact bundle
7. Clean up and print PASS/FAIL

## Risks and Guardrails

| Risk | Guardrail |
|------|-----------|
| Reintroducing DOM-specific API | Keep `execute_js` as debug only, not in contracts |
| Production backdoors | All test APIs behind staging/test feature flags |
| Screenshot-based flakiness | Prefer structured assertions; visual only for visual features |
| Native/web divergence | Require semantic ID parity and contract parity tests |
| Masking crashes | External runner, append-mode logs, crash-safe artifact bundle |

## Definition of Done

The architecture is ready for general use when: stable test harness mode exists, contract runner works, fixture and artifact systems work, web and native adapter parity covers core surfaces, and top five highest-risk feature families have passing contracts.

## Recommended Prompt

```text
Run an autonomous Godly Staging test for contract "<contract-id>".
Reset staging profile, launch "<web|native>" frontend, load fixture "<fixture-id>",
execute contract with semantic assertions first and visual snapshots second,
restart if persistence checks required, collect artifact bundle, clean up, report PASS/FAIL
with first failing assertion, root-cause hypothesis, and artifact paths.
```

Slash command (once implemented):
```text
/staging-test <contract-id> --frontend <web|native> --fixture <fixture-id>
```
