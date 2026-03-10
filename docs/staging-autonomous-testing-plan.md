# Godly Staging Autonomous Testing Plan

## Summary

This document defines the implementation plan for a staging-only test architecture that allows an AI agent to test Godly Terminal autonomously with high confidence.

The target is not "magic 100% correctness." The target is:

1. 100% autonomous control of app behavior needed for testing
2. 100% machine-readable observability for feature state and outcomes
3. Deterministic pass/fail oracles for every supported feature
4. Crash-safe artifact capture so failures remain debuggable
5. Frontend-agnostic coverage for both WebView and native Iced frontends

## Why This Exists

The current stack already has useful pieces:

- Staging isolation via `GODLY_INSTANCE=staging`
- MCP tools for terminal, workspace, split, and screenshot operations
- `execute_js` for DOM/store inspection
- PyAutoGUI-based OS automation for true pointer/keyboard workflows
- A flow engine that can eventually act as a reusable scenario DSL

But the current system still has four structural gaps:

1. `execute_js` is too generic and too WebView-specific.
2. `capture_screenshot` is tied to visible canvases, so it is not a reliable oracle.
3. There is no first-class test harness mode with reset, fixture loading, graceful restart, and artifact bundling.
4. There is no shared semantic API that works across both the web frontend and the native Iced frontend.

## Scope

This plan covers:

- Godly Staging only
- Web frontend and native Iced frontend
- MCP-facing tooling for AI-driven testing
- Deterministic assertions, fixtures, restart flows, and artifact capture

This plan does not cover:

- Production-only hidden test hooks
- Replacing unit/browser/integration/e2e tests already in the repo
- Fuzzy "LLM judges screenshots" as the primary correctness oracle

## Success Criteria

The implementation is successful when all of the following are true:

- An agent can launch or connect to Godly Staging in test-harness mode.
- An agent can reset the test profile to a known clean state.
- An agent can load a named fixture and know when it is ready.
- An agent can drive feature flows through typed semantic actions instead of raw DOM scripts.
- An agent can query typed app state, layout state, focus state, terminal state, and persistence state.
- An agent can request offscreen render snapshots for any pane or terminal, even if not currently visible.
- A test run produces a single artifact bundle containing logs, event traces, snapshots, assertions, and crash evidence.
- The same test contract can run against web and native frontends, with frontend-specific adapter code hidden behind a shared protocol.

## Core Principles

### 1. Deterministic Oracle First

Structured state assertions come first.

Examples:

- active workspace id
- active terminal id
- split tree shape
- terminal grid text
- session persistence status
- focused pane id
- layout bounds

Visual comparison is still important, but it is a secondary oracle unless the feature is explicitly visual.

### 2. Semantic Driver First

The primary control plane must be semantic, not selector-based and not coordinate-based.

Good:

- `ui.act(target="workspace.sidebar.toggle", action="click")`
- `ui.query(target="tab.active")`
- `ui.wait(condition="terminal.created")`

Bad:

- `execute_js("document.querySelector(...)")`
- `click(843, 217)` without a semantic origin

### 3. External Runner Always

The test runner must live outside the app process. If the app crashes, hangs, or deadlocks, the runner must still be able to:

- detect the failure
- capture logs and dumps
- relaunch if the contract allows it
- report a failed test with artifacts

### 4. Frontend-Agnostic Contract

The test contract should be identical whether the UI is rendered by WebView or native Iced. Frontend-specific details belong inside adapters, not inside test cases.

### 5. Staging-Only Privileges

Test hooks belong in Godly Staging only. Do not add production-only backdoors.

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

Add a dedicated `test-harness` mode to Godly Staging.

Responsibilities:

- start with a clean isolated profile
- expose test-only RPC operations
- support graceful shutdown and restart
- support fixture loading
- expose crash status and health
- flush logs and state on demand

Required capabilities:

- `launch_app`
- `connect_app`
- `graceful_quit`
- `graceful_restart`
- `reset_profile`
- `load_fixture`
- `wait_until_ready`
- `export_state_dump`
- `collect_artifact_bundle`

Implementation rule:

- These capabilities must be behind staging/test feature gates, not active in production.

### 2. Semantic Automation API

Create a typed API for actions, queries, and waits.

The API should be narrow and explicit. Avoid a generic "run arbitrary code" escape hatch as the main path.

Recommended top-level operations:

- `ui.act`
- `ui.query`
- `ui.wait`
- `app.lifecycle`
- `workspace.query`
- `workspace.act`
- `terminal.query`
- `terminal.act`
- `layout.query`
- `settings.query`
- `settings.act`
- `render.snapshot`
- `metrics.query`

Example request shape:

```json
{
  "target": "workspace.sidebar.toggle",
  "action": "click",
  "args": {}
}
```

Example response shape:

```json
{
  "ok": true,
  "target": "workspace.sidebar.toggle",
  "action": "click",
  "timestamp_ms": 1760000000000
}
```

### 3. Semantic IDs

Every testable UI surface needs a stable semantic id.

Examples:

- `workspace.sidebar.toggle`
- `workspace.list`
- `workspace.active`
- `tab.add`
- `tab.active`
- `tab.close:<terminal-id>`
- `pane.active`
- `pane.divider:<workspace-id>`
- `terminal.surface:<terminal-id>`
- `settings.dialog`
- `settings.theme.select`
- `quick-claude.prompt`

Rules:

- Semantic ids must not depend on CSS selectors, DOM tree depth, or pixel coordinates.
- The same id must mean the same thing in web and native frontends.
- Dynamic ids should follow a documented pattern.

### 4. Frontend Adapters

Implement two adapters behind the same protocol.

### Web Adapter

Responsibilities:

- resolve semantic ids to DOM/store operations
- report layout bounds and visibility
- perform semantic actions
- return typed state

This adapter can use `window.__STORE__` internally, but callers should not know or care.

### Native Adapter

Responsibilities:

- resolve semantic ids to Iced app state and message dispatch
- report layout bounds and visibility
- perform semantic actions
- return typed state

This avoids locking the architecture to WebView-only `execute_js`.

### 5. Observability Plane

The harness must provide machine-readable observability for all important state.

Required state exports:

- app info
- active frontend type
- workspace list and active workspace
- terminal list and active terminal
- layout tree and pane focus
- split ratios and orientation
- terminal metadata
- terminal grid text and dimensions
- terminal scroll position
- persistence metadata
- daemon session metadata
- notification settings
- feature-specific state for any new subsystem

Required event streams:

- workspace switched
- terminal created
- terminal focused
- split changed
- notification changed
- process changed
- persistence save started/completed
- persistence restore started/completed
- app ready
- app shutting down
- app crashed

Required health/perf metrics:

- key-to-grid latency percentiles
- snapshot fetch latency
- terminal output backlog
- daemon bridge queue depth
- render time
- dropped frame or skipped render counters if available

### 6. Render Snapshot Layer

Add a first-class render oracle that does not depend on the currently visible canvas.

Required snapshot types:

- terminal grid snapshot as text
- terminal rich snapshot as structured cells
- pane render snapshot as image
- app/window snapshot as image

Design rule:

- Any terminal or pane should be snapshot-capable even when hidden, backgrounded, or offscreen.

This is required because the current visible-canvas screenshot model is not a complete oracle.

### 7. Artifact Bundle

Every run must produce an artifact bundle on failure, and optionally on success.

Recommended bundle contents:

```text
artifacts/<run-id>/
  manifest.json
  result.json
  assertions.json
  app-state.json
  layout-tree.json
  daemon-sessions.json
  event-trace.jsonl
  metrics.json
  screenshots/
  terminal-grids/
  frontend.log
  godly-daemon-debug.log
  godly-bridge-debug.log
  crash.txt
```

Rules:

- Crash evidence must survive restarts.
- The bundle path must be returned to the caller.
- The runner must write enough metadata to replay what happened.

### 8. Feature Contracts

Each autonomously testable feature needs a contract file.

Recommended location:

- `testing/contracts/*.json`

Recommended fields:

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

Minimum contract requirements:

- named fixture
- ordered steps
- explicit assertions
- cleanup section
- artifact policy
- supported frontend list

Definition of done for a new feature:

- the feature has a contract
- the feature has fixture coverage
- the feature has semantic ids
- the feature has structured state queries
- the feature has a deterministic assertion path

### 9. Fixture System

Fixtures must produce known-good starting states without manual setup.

Recommended fixture types:

- clean profile
- single workspace single terminal
- two terminals same workspace
- two-pane split
- multi-tab workspace
- quick-claude workspace
- scrollback-heavy terminal
- restored persistence session

Required fixture capabilities:

- create
- verify ready
- tear down
- idempotent reset

### 10. External Runner

Build an external runner process that orchestrates test execution.

Recommended implementation:

- TypeScript runner first, because the repo already has Node/Vitest tooling
- later, keep the protocol generic enough that other runners can exist

Runner responsibilities:

- start or attach to staging
- select frontend mode
- reset profile
- load fixture
- execute the contract
- capture artifacts continuously
- fail hard on crash or timeout
- clean up resources
- print a concise PASS/FAIL summary

Important rule:

- Do not hide failures with retry loops. Retry only at infrastructure connection boundaries if absolutely necessary, and record every retry in the artifact bundle.

### 11. OS Automation Fallback

PyAutoGUI remains useful, but only as the last-mile interaction layer.

Use it for:

- native drag and drop
- real divider drags
- keyboard shortcut routing
- window focus
- context menus that cannot yet be addressed semantically

Do not use it for:

- primary state assertions
- locating UI via fragile screenshots when a semantic query is available

Preferred pattern:

1. query semantic bounds
2. convert to window coordinates
3. perform OS-level action
4. verify via semantic state and render snapshots

## Suggested Repo Layout

This is the recommended file layout for the first implementation pass.

```text
docs/
  staging-autonomous-testing-plan.md

testing/
  contracts/
    split-basic.json
    workspace-switch.json
    persistence-restart.json
  fixtures/
    clean-profile.ts
    two-terminals.ts
    split-basic.ts
  runner/
    index.ts
    contract-runner.ts
    artifact-bundle.ts
    assertions.ts

src/
  testing/
    semantic-ids.ts
    web-adapter.ts
    test-harness-bridge.ts

src-tauri/
  protocol/src/
    testing.rs
    mcp_messages.rs
  src/
    commands/testing.rs
    mcp_server/handler.rs
    testing/
      harness.rs
      artifacts.rs
      state_dump.rs
  mcp/src/
    tools.rs
  native/iced-shell/src/
    testing/
      adapter.rs
      semantic_ids.rs
```

## Recommended MCP Tools

Add these test-focused tools on top of the existing MCP surface.

Lifecycle:

- `launch_staging_app`
- `connect_staging_app`
- `graceful_quit_app`
- `graceful_restart_app`
- `reset_staging_profile`
- `wait_for_app_ready`

Fixtures and contracts:

- `list_test_contracts`
- `run_test_contract`
- `load_test_fixture`
- `cleanup_test_fixture`

Semantic control:

- `ui_act`
- `ui_query`
- `ui_wait`

State and artifacts:

- `export_app_state`
- `export_layout_tree`
- `export_event_trace`
- `capture_render_snapshot`
- `capture_window_snapshot`
- `collect_artifact_bundle`

Metrics:

- `get_test_metrics`

## Concrete Task List

Use this section as the implementation checklist. The goal is to land this as a sequence of small vertical slices, not one large branch.

Execution rules:

- Finish tasks in order unless a later task explicitly says it can run in parallel.
- Each task should end with a runnable verification step.
- Prefer shipping a thin end-to-end slice before widening the surface area.
- Do not add production-only test hooks. Everything here is staging/test gated.

### Phase 0 Tasks: Harness Foundation

- [~] `P0.1` Add a staging test-harness mode switch.
  Files:
  `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/native/iced-shell/src/main.rs`, `src-tauri/Cargo.toml`
  Outcome:
  Staging can boot in a known `test-harness` mode without affecting production behavior.
  Acceptance:
  The app reports that test-harness mode is enabled in both web and native frontend paths.

- [~] `P0.2` Define shared protocol types for testing operations.
  Files:
  `src-tauri/protocol/src/testing.rs`, `src-tauri/protocol/src/mcp_messages.rs`, any shared type export files
  Outcome:
  There is a typed request/response model for lifecycle, semantic actions, queries, waits, snapshots, metrics, and artifacts.
  Acceptance:
  Web and native code compile against the same testing protocol types.

- [~] `P0.3` Create the backend test harness service skeleton.
  Files:
  `src-tauri/src/testing/harness.rs`, `src-tauri/src/testing/mod.rs`
  Outcome:
  A central service can answer readiness/health, track current run metadata, and manage fixture/reset lifecycle.
  Acceptance:
  A command can ask "are you ready?" and receive a structured response with frontend type and harness status.

- [~] `P0.4` Add artifact bundle infrastructure.
  Files:
  `src-tauri/src/testing/artifacts.rs`, logging/config helpers as needed
  Outcome:
  Every run gets a run id, artifact directory, manifest, and append-only artifact writes.
  Acceptance:
  A no-op run creates an artifact bundle containing at least `manifest.json` and `result.json`.

- [~] `P0.5` Add state dump infrastructure.
  Files:
  `src-tauri/src/testing/state_dump.rs`
  Outcome:
  The harness can export app state, layout state, and daemon/session metadata in one call.
  Acceptance:
  A state-dump command returns valid JSON and writes it into the artifact bundle.

- [~] `P0.6` Add Tauri commands for test-harness lifecycle.
  Files:
  `src-tauri/src/commands/testing.rs`, `src-tauri/src/lib.rs`
  Outcome:
  The frontend and MCP layer can call launch/connect/reset/ready/state-dump/artifact operations.
  Acceptance:
  Commands are registered and callable without using raw `execute_js`.

- [~] `P0.7` Add MCP tools for the phase-0 lifecycle surface.
  Files:
  `src-tauri/mcp/src/tools.rs`, `src-tauri/src/mcp_server/handler.rs`
  Outcome:
  MCP exposes `connect_staging_app`, `wait_for_app_ready`, `reset_staging_profile`, `collect_artifact_bundle`, and related lifecycle tools.
  Acceptance:
  An agent can connect to a running staging app and request a state dump plus artifact bundle path.

- [~] `P0.8` Implement graceful quit and graceful restart for staging.
  Files:
  `src-tauri/src/testing/harness.rs`, platform/process helpers, native integration as needed
  Outcome:
  The harness can shut down and relaunch staging without bypassing persistence hooks.
  Acceptance:
  Restart goes through the same quit path used for real persistence, not process-kill shortcuts.

- [~] `P0.9` Implement profile reset for staging test runs.
  Files:
  harness/service files, staging profile helpers, persistence paths
  Outcome:
  Tests can start from a clean isolated staging profile.
  Acceptance:
  Reset removes prior test state and recreates a clean baseline without touching production data.

- [~] `P0.10` Add a single smoke test for harness connectivity.
  Files:
  new integration test or targeted runner smoke file under `testing/` or existing integration area
  Outcome:
  There is one automated verification that the harness can connect, reset, wait for ready, and collect artifacts.
  Acceptance:
  The smoke test passes locally against staging.

### Phase 1 Tasks: Web Semantic API

- [~] `P1.1` Define the semantic id registry for the web frontend.
  Files:
  `src/testing/semantic-ids.ts`
  Outcome:
  Stable semantic ids exist for workspaces, tabs, panes, terminal surfaces, settings, and quick-claude inputs.
  Acceptance:
  The semantic id list is documented and reviewed before adapter implementation starts.

- [~] `P1.2` Implement the web adapter query surface.
  Files:
  `src/testing/web-adapter.ts`
  Outcome:
  Semantic ids can resolve to typed state and layout data without exposing DOM selectors to callers.
  Acceptance:
  The adapter can answer at least: active workspace, active terminal, split tree, pane focus, tab order, and element bounds.

- [~] `P1.3` Implement the web adapter action surface.
  Files:
  `src/testing/web-adapter.ts`, component/store glue as needed
  Outcome:
  Semantic actions can click/focus/type/trigger app operations through typed calls.
  Acceptance:
  The adapter can perform at least: create terminal, focus terminal, create split, clear split, switch workspace, and open settings.

- [~] `P1.4` Implement the web adapter wait surface.
  Files:
  `src/testing/web-adapter.ts`
  Outcome:
  The harness can wait for semantic state transitions instead of using arbitrary sleeps.
  Acceptance:
  Waits support timeout, polling interval, and a structured timeout error.

- [~] `P1.5` Expose the web adapter through a narrow bridge.
  Files:
  `src/testing/test-harness-bridge.ts`, `src/main.ts`
  Outcome:
  The frontend registers the adapter with the harness when in test mode.
  Acceptance:
  The harness can query and act through the adapter without direct `execute_js` from contracts.

- [~] `P1.6` Add typed MCP tools for `ui_act`, `ui_query`, and `ui_wait`.
  Files:
  `src-tauri/mcp/src/tools.rs`, `src-tauri/src/mcp_server/handler.rs`, protocol types
  Outcome:
  Agents can drive semantic UI operations through stable tools.
  Acceptance:
  The MCP surface can perform a web-only happy path without raw JS.

- [~] `P1.7` Add typed state export tools for core domains.
  Files:
  protocol, MCP handler, state dump helpers
  Outcome:
  There are first-class exports for app state, layout tree, active workspace, active terminal, and terminal grid state.
  Acceptance:
  Existing manual-testing flows can be rewritten using typed queries for at least one feature.

- [~] `P1.8` Create the first web-only vertical slice contract.
  Files:
  `testing/contracts/split-basic.json`, fixture files, runner files
  Outcome:
  One complete end-to-end contract runs entirely through semantic APIs.
  Acceptance:
  `split-basic` passes on the web frontend from a clean staging profile.

### Phase 2 Tasks: Contracts, Fixtures, and Runner

- [ ] `P2.1` Define the contract schema.
  Files:
  `testing/contracts/schema.json` or equivalent TypeScript schema files
  Outcome:
  Contracts have a strict shape for fixture, steps, assertions, restart expectations, and cleanup.
  Acceptance:
  Invalid contracts fail schema validation before execution.

- [ ] `P2.2` Define the assertion model.
  Files:
  `testing/runner/assertions.ts`
  Outcome:
  Contracts can express equality, contains, regex, bounds, existence, and metric-threshold assertions.
  Acceptance:
  A failing assertion reports expected vs actual and a stable assertion id.

- [ ] `P2.3` Build the fixture loader API.
  Files:
  `testing/fixtures/*.ts`, harness integration files
  Outcome:
  Fixtures can create and verify known-good starting states.
  Acceptance:
  The runner can call `load_fixture("two-terminals")` and get a ready result.

- [ ] `P2.4` Implement the external runner shell.
  Files:
  `testing/runner/index.ts`, `testing/runner/contract-runner.ts`
  Outcome:
  A standalone runner can connect to staging, reset, load fixture, run contract, and clean up.
  Acceptance:
  The runner prints a short PASS/FAIL summary and exits non-zero on failure.

- [ ] `P2.5` Implement artifact-aware contract execution.
  Files:
  runner files, `artifact-bundle.ts`
  Outcome:
  Every step and assertion is recorded in the bundle during execution, not just at the end.
  Acceptance:
  A failed contract leaves behind a complete artifact directory with step-by-step trace data.

- [ ] `P2.6` Seed the first fixture set.
  Files:
  `testing/fixtures/clean-profile.ts`, `testing/fixtures/two-terminals.ts`, `testing/fixtures/split-basic.ts`, `testing/fixtures/restored-session.ts`
  Outcome:
  The minimum useful fixtures exist for terminal, split, and persistence tests.
  Acceptance:
  Each fixture has create, verify-ready, and cleanup logic.

- [ ] `P2.7` Seed the first contract set.
  Files:
  `testing/contracts/terminal-crud.json`, `testing/contracts/workspace-switch.json`, `testing/contracts/split-basic.json`, `testing/contracts/persistence-restart.json`
  Outcome:
  The highest-value basic flows are codified as contracts.
  Acceptance:
  At least `terminal-crud` and `split-basic` pass on the web frontend.

- [ ] `P2.8` Add a `run_test_contract` MCP tool.
  Files:
  `src-tauri/mcp/src/tools.rs`, handler/protocol files
  Outcome:
  Agents can invoke a single named contract instead of hand-driving each step.
  Acceptance:
  An MCP call can execute a contract and return PASS/FAIL plus artifact path.

### Phase 3 Tasks: Render Oracles and Metrics

- [ ] `P3.1` Add terminal grid snapshot export to the artifact bundle.
  Files:
  state dump / artifact files, terminal snapshot helpers
  Outcome:
  Each test can save structured terminal snapshots during assertions.
  Acceptance:
  The runner can attach text-grid or rich-grid snapshots to a failing step.

- [ ] `P3.2` Add offscreen terminal render snapshots.
  Files:
  renderer-facing web/native snapshot code, handler/protocol files
  Outcome:
  A hidden or background terminal can be snapshotted as an image without forcing it visible.
  Acceptance:
  A non-visible terminal still yields a valid render snapshot.

- [ ] `P3.3` Add pane/window render snapshots.
  Files:
  web/native adapter snapshot code, artifact bundle code
  Outcome:
  Visual features can be verified at pane-level and window-level.
  Acceptance:
  A split-pane contract can capture before/after images of the same workspace.

- [ ] `P3.4` Add an event journal.
  Files:
  harness/event logging files
  Outcome:
  Important semantic events are written to `event-trace.jsonl` during the run.
  Acceptance:
  The event trace shows state transitions in execution order with timestamps.

- [ ] `P3.5` Add perf metric capture.
  Files:
  frontend perf hooks, daemon/bridge metrics export, protocol types
  Outcome:
  The runner can query latency and queue-depth metrics for high-risk flows.
  Acceptance:
  A contract can assert a threshold such as key-to-grid p95 or bridge backlog upper bound.

- [ ] `P3.6` Add crash and hang classification.
  Files:
  harness/runner/artifact bundle code
  Outcome:
  The runner can distinguish assertion failure, app crash, app hang, and infrastructure failure.
  Acceptance:
  Failure results contain a structured `failure_type`.

### Phase 4 Tasks: Native Frontend Parity

- [ ] `P4.1` Define the native semantic id registry.
  Files:
  `src-tauri/native/iced-shell/src/testing/semantic_ids.rs`
  Outcome:
  Native uses the same semantic ids and patterns as the web adapter.
  Acceptance:
  There is a parity mapping document or code comments showing one-to-one id coverage.

- [ ] `P4.2` Implement the native query adapter.
  Files:
  `src-tauri/native/iced-shell/src/testing/adapter.rs`
  Outcome:
  Native can answer the same core queries as the web adapter.
  Acceptance:
  The harness can query active workspace, active terminal, focused pane, layout tree, and bounds from native.

- [ ] `P4.3` Implement the native action adapter.
  Files:
  `src-tauri/native/iced-shell/src/testing/adapter.rs`, Iced message plumbing
  Outcome:
  Native can perform the same core semantic actions as web.
  Acceptance:
  The native adapter can create terminals, switch workspace, split, focus, resize, and open settings.

- [ ] `P4.4` Implement the native wait/snapshot surface.
  Files:
  native adapter and snapshot code
  Outcome:
  Native supports waits and visual/state snapshots without relying on DOM hooks.
  Acceptance:
  At least `split-basic` and `persistence-restart` can capture native artifacts.

- [ ] `P4.5` Add parity runs to the runner.
  Files:
  runner files, contract metadata
  Outcome:
  One command can execute the same contract against web and native and compare outcomes.
  Acceptance:
  Contracts fail clearly if a frontend is unsupported or deviates from expected behavior.

### Phase 5 Tasks: Agent UX and Doc Migration

- [ ] `P5.1` Freeze the MCP tool names and output shape.
  Files:
  tool registry, protocol types, handler files
  Outcome:
  The autonomous testing tools have a stable, documented public shape for agents.
  Acceptance:
  Tool descriptions match actual behavior and include cleanup/artifact semantics.

- [ ] `P5.2` Update `docs/mcp-testing.md` to the contract-based workflow.
  Files:
  `docs/mcp-testing.md`
  Outcome:
  The old procedural checklist is replaced or wrapped by the new contract runner workflow.
  Acceptance:
  The doc shows how to run named contracts instead of manually replaying long step lists.

- [ ] `P5.3` Update `.claude/skills/manual-testing.md`.
  Files:
  `.claude/skills/manual-testing.md`
  Outcome:
  Manual testing becomes a fallback path for features without full autonomous harness support.
  Acceptance:
  The skill prefers autonomous harness contracts when available.

- [ ] `P5.4` Add example prompts and slash-command conventions.
  Files:
  this doc, MCP docs, any prompt docs
  Outcome:
  Humans know how to invoke a contract run in one prompt.
  Acceptance:
  There is one default prompt and one slash-command shape documented in all relevant testing docs.

- [ ] `P5.5` Add one end-to-end acceptance demo contract.
  Files:
  new contract/fixture files, docs
  Outcome:
  There is one showcase contract suitable for routine regression runs after feature work.
  Acceptance:
  The demo contract exercises launch, fixture load, semantic actions, assertions, artifacts, cleanup, and final report.

## Recommended PR Breakdown

If you want to keep this implementable, split the work like this:

1. `PR1` Harness mode, protocol skeleton, lifecycle commands, artifact bundle shell
2. `PR2` Web semantic ids, `ui_query`/`ui_act`/`ui_wait`, first state exports
3. `PR3` Contract schema, fixture loader, runner, `split-basic` web contract
4. `PR4` Offscreen snapshots, event journal, metrics export
5. `PR5` Native adapter parity for core contracts
6. `PR6` Doc migration, manual-testing skill update, agent-facing command polish

## First Vertical Slice

Before broadening the surface area, ship this slice first:

- [ ] Boot staging in test-harness mode
- [ ] Reset the staging profile
- [ ] Load the `two-terminals` fixture
- [ ] Execute `ui_act(create_split)`
- [ ] Verify split tree via `ui_query`
- [ ] Capture one artifact bundle
- [ ] Clean up and print PASS/FAIL

If that slice works end to end, the rest of the system can be built incrementally without redesigning the foundation.

## Implementation Phases

### Phase 0: Foundation

Deliverables:

- staging-only test harness mode
- profile reset
- graceful quit and restart
- artifact bundle skeleton
- health/readiness endpoint

Acceptance:

- The runner can launch staging, wait for ready, reset state, and collect logs.

### Phase 1: Semantic API for Web Frontend

Deliverables:

- semantic ids
- `ui_act`, `ui_query`, `ui_wait`
- typed workspace/terminal/layout queries
- removal of raw `execute_js` from primary happy-path tests

Acceptance:

- A split-pane contract can run end-to-end on the web frontend without raw DOM selectors in the contract.

### Phase 2: Fixtures and Contracts

Deliverables:

- fixture loader
- contract schema
- contract runner
- initial contract set

Initial contract set:

- terminal create/close/read/write
- workspace create/switch/move
- split create/focus/resize/clear
- tab focus/order
- persistence restart

Acceptance:

- The runner can execute a named contract from a clean profile and report deterministic PASS/FAIL.

### Phase 3: Render Oracles and Perf Data

Deliverables:

- offscreen pane snapshots
- offscreen terminal snapshots
- event journal
- perf metrics export

Acceptance:

- Hidden/background terminals can still be verified visually and structurally.

### Phase 4: Native Frontend Adapter

Deliverables:

- native semantic adapter
- native layout/state queries
- native action dispatch
- parity tests across web and native

Acceptance:

- The same contract passes on both frontends unless the contract explicitly opts out.

### Phase 5: Agent-Friendly Command Surface

Deliverables:

- stable MCP tool names
- concise failure summaries
- artifact bundle links in results
- a documented prompt shape for agents

Acceptance:

- A human can ask for a contract run in plain language and the agent can execute it without custom explanation.

## Recommended Initial Contracts

Prioritize contracts that match product risk:

1. session persistence
2. workspace switching
3. multi-session efficiency signals
4. split pane layout/focus/resize
5. terminal CRUD and output correctness
6. quick-claude flows
7. notification configuration
8. settings changes that affect rendering or behavior

## Mapping to Existing Code

Use the current system as the starting point, not as the final interface.

- `src/main.ts` already exposes `window.__STORE__` and initializes the flow engine.
- `src/flow-engine/` can eventually host reusable scenario definitions, but the runner must still live out of process.
- `src-tauri/mcp/src/tools.rs` is the current MCP tool registry and should gain typed test tools.
- `src-tauri/src/mcp_server/handler.rs` should route the new test requests.
- `docs/mcp-testing.md` should be updated once the new contract-based flow replaces the old procedural checklist.
- `.claude/skills/manual-testing.md` should eventually become a wrapper around the autonomous harness for features that support it.

## Risks and Guardrails

### Risk: Reintroducing a DOM-Specific Testing API

Guardrail:

- keep `execute_js` as a debug tool only
- do not make it the contract surface

### Risk: Hidden Production Backdoors

Guardrail:

- gate all privileged test APIs behind staging/test feature flags

### Risk: Screenshot-Based Flakiness

Guardrail:

- prefer structured assertions
- use visual comparisons only where the feature is truly visual

### Risk: Native/Web Divergence

Guardrail:

- require semantic id parity
- require contract parity tests

### Risk: Masking Crashes

Guardrail:

- external runner
- append-mode logs
- crash-safe artifact bundle

## Definition of Done

The autonomous staging architecture is "done enough" for general use when:

- there is a stable test harness mode
- there is a contract runner
- there are fixture and artifact systems
- there is web and native adapter parity for the core surfaces
- the top five highest-risk feature families each have passing contracts

## Follow-Up Docs to Update After Implementation

- `docs/mcp-testing.md`
- `.claude/skills/manual-testing.md`
- any feature-specific QA docs that currently assume manual-only steps

## Recommended Prompt After Implementation

Use this as the default prompt once the architecture is implemented:

```text
Run an autonomous Godly Staging test for contract "<contract-id>".
Reset the staging test profile first, launch the "<web|native>" frontend, load fixture "<fixture-id>", execute the full contract with semantic assertions first and visual snapshots second, restart the app if the contract requires persistence checks, collect the artifact bundle, clean up all created resources, and report PASS or FAIL with the first failing assertion, root-cause hypothesis, and artifact paths.
```

If we add a dedicated slash command later, the recommended shape is:

```text
/staging-test <contract-id> --frontend <web|native> --fixture <fixture-id>
```

Examples:

```text
Run an autonomous Godly Staging test for contract "split-basic".
Reset the staging test profile first, launch the "web" frontend, load fixture "two-terminals", execute the full contract with semantic assertions first and visual snapshots second, collect the artifact bundle, clean up all created resources, and report PASS or FAIL with artifact paths.
```

```text
Run an autonomous Godly Staging test for contract "persistence-restart".
Reset the staging test profile first, launch the "native" frontend, load fixture "restored-session", execute the full contract, restart the app once, collect the artifact bundle, clean up all created resources, and report PASS or FAIL with the first failing assertion and artifact paths.
```
