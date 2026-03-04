# Native Iced Architecture for Parallel Agent Work

## Goals

- Maximize independent workstreams with low merge conflict risk.
- Keep feature logic deterministic and testable without UI or daemon processes.
- Keep Iced-specific code thin and replaceable.

## Workspace Shape

Target structure inside `src-tauri/native`:

```
native/
  app-adapter/         # adapters: daemon client, keyboard mapping, clipboard
  iced-shell/          # app shell: routing, wiring, subscriptions, top-level view
  terminal-surface/    # rendering primitive
  parity-harness/      # integration + parity validation

  tabs-core/           # core: pure tab state machine (added)
  workspaces-core/     # core: pure workspace ordering/selection state machine (added)
  features/            # future: one crate per feature
  ports/               # future: shared side-effect traits
  testkit/             # future: fakes for ports, deterministic clocks/schedulers
```

## Layering Rules

- `core/*`: pure state machines and domain rules; no `iced`, no filesystem/network/process access.
- `features/*`: reducers/orchestrators over `core` + `ports`; returns effect intents.
- `ports/*`: traits for effects (daemon session lifecycle, clipboard, notifications, persistence, clock).
- `adapters/*` (`app-adapter`, etc.): concrete I/O implementations of `ports`.
- `iced-shell`: composes features and adapters; no business rules that cannot be unit-tested outside UI.

Dependency direction:

```
iced-shell -> features -> core
iced-shell -> adapters -> ports
features   -> ports
core       -> (nothing)
```

## First Vertical Slice: Tabs

Implemented in this change:

- `godly-tabs-core` crate (`native/tabs-core`):
  - Ordered tab state machine (`open`, `close`, `activate`, `next`, `previous`, `reorder`)
  - Pure logic with focused unit tests
- `godly-iced-shell` now delegates tab ordering/activation in `TerminalCollection` to `godly-tabs-core`.
- `godly-workspaces-core` crate (`native/workspaces-core`):
  - Ordered workspace state machine (`add`, `remove`, `set_active`, `next`, `previous`, `move_up/down`)
  - Pure logic with focused unit tests
- `godly-iced-shell` now delegates workspace ordering/active selection in `WorkspaceCollection`
  to `godly-workspaces-core`, while keeping `LayoutNode` construction local to shell code.

This creates a stable seam for future tab features (reorder DnD, pinned tabs, MRU, grouping) without
expanding `app.rs`.

## Testing Strategy

1. Core tests (`core/*`): fast unit tests and property-like invariant tests.
2. Feature tests (`features/*`): reducer tests with fake ports.
3. Adapter contract tests (`adapters/*` + `ports/*`): same behavior for real + fake implementations.
4. Parity/integration tests (`parity-harness`): native behavior vs expected protocol/runtime behavior.
5. Critical E2E only for full user journeys and rendering regressions.

## Parallel Ownership Plan

Use crate-level ownership so agents can work concurrently:

- Lane A: `tabs-core` and future `features/tabs`
- Lane B: workspace/split feature extraction (`core/layout`, `features/workspace`)
- Lane C: ports + testkit scaffolding
- Lane D: `iced-shell` composition and message wiring
- Lane E: parity/integration harness and regression suites

Each lane changes distinct crates with explicit interfaces, reducing rebase churn.
