# Contract Testing

Contract tests verify features end-to-end against a running Godly Staging instance via MCP tools. They are declarative JSON files that define a sequence of actions, queries, waits, and assertions.

## Architecture

```
                                   stdio
testing/runner/index.ts  ───────────────────►  godly-mcp.exe
        │                                          │
        ▼                                          │ Named Pipe IPC
  ContractRunner                                   ▼
   ├─ reads contract JSON              Godly Terminal (Staging)
   ├─ sets up fixture                   ├─ Tauri app
   ├─ executes steps via MCP            ├─ Daemon + PTY sessions
   ├─ evaluates assertions              └─ Persistence layer
   └─ writes artifacts
```

### Components

| Component | Location | Purpose |
|-----------|----------|---------|
| **Contracts** | `testing/contracts/*.json` | Declarative test definitions (steps, assertions, cleanup) |
| **Fixtures** | `testing/fixtures/*.ts` | Deterministic setup/teardown for contracts |
| **Runner** | `testing/runner/` | Executes contracts: parses JSON, calls MCP tools, evaluates assertions |
| **MCP Client** | `testing/runner/mcp-client.ts` | Stdio bridge to `godly-mcp.exe` |
| **Assertions** | `testing/runner/assertions.ts` | Evaluators: `equals`, `gt`, `gte`, `lt`, `lte`, `not_null`, `contains` |
| **Artifacts** | `testing/artifacts/` | Per-run traces, screenshots, and results |

### Contract Schema

```json
{
  "id": "feature-name",
  "description": "What this contract verifies",
  "frontends": ["web", "native"],
  "fixture": "clean-profile",
  "requires_restart": false,
  "steps": [
    {
      "id": "step-name",
      "description": "Human-readable description",
      "type": "action | query | wait | snapshot",
      "target": "workspace.active | terminal.count | ...",
      "action": "create | switch | ...",
      "condition": "app.ready | workspace.switched | ...",
      "args": {},
      "assertions": [
        { "id": "assertion-name", "type": "equals", "path": "field", "expected": "value" }
      ],
      "timeout_ms": 5000,
      "on_failure": "abort | continue"
    }
  ],
  "cleanup": [
    { "type": "reset" }
  ]
}
```

### Step Types

| Type | MCP Tool | Purpose |
|------|----------|---------|
| `action` | `ui_act(target, action, args?)` | Mutate state (create workspace, switch tab, restart app) |
| `query` / `assert` | `ui_query(target, args?)` | Read state and check assertions |
| `wait` | `ui_wait(condition, timeout_ms, args?)` | Poll until a condition is met |
| `snapshot` | `capture_screenshot` | Capture a screenshot for visual verification |

### Semantic Targets

| Target | Type | Description |
|--------|------|-------------|
| `workspace.active` | query | Get the active workspace |
| `workspace.list` | query | List all workspaces |
| `workspace.details` | query | Get workspace details (name, folder_path, etc.) |
| `workspace.switched` | wait | Wait for workspace switch to complete |
| `workspace` | action | `create`, `switch`, `delete` |
| `terminal.count` | query | Count terminals in active workspace |
| `terminal.created` | wait | Wait for terminal creation |
| `terminal.cwd` | query | Get terminal's current working directory |
| `terminal.idle` | wait | Wait for terminal to be idle |
| `layout.tree` | query | Get the layout tree |
| `app.ready` | wait | Wait for app to be ready |
| `app.lifecycle` | action | `restart` |
| `app` | action | `save_layout` |

### Fixtures

Fixtures provide deterministic state setup before a contract runs.

| Fixture | Description |
|---------|-------------|
| `clean-profile` | Reset to empty state (no workspaces, no terminals) |
| `two-terminals` | One workspace with two terminals |
| `multi-workspace` | Two workspaces, first with 2 terminals, second with 1 |
| `split-basic` | One workspace with a horizontal split |

Fixtures are registered in `testing/fixtures/index.ts`. Each fixture implements:
- `create(client)` — set up the state (must be deterministic)
- `verifyReady(client)` — confirm the state is correct
- `tearDown(client)` — clean up (idempotent)

## Commands

```bash
# List all contracts
pwsh testing/list-contracts.ps1

# Run a specific contract (requires Godly Staging running)
pnpm --dir testing run run-contract contracts/<id>.json

# Typecheck contracts and fixtures
pnpm --dir testing run typecheck
```

## Prerequisites

1. **Build and install Godly Staging:**
   ```bash
   pnpm staging:build && pnpm staging:install
   ```

2. **Launch Godly Staging** from the Start Menu ("Godly Terminal (Staging)")

3. **Run contracts** against the running instance

## Writing a Contract

1. Create `testing/contracts/<feature-name>.json`
2. Choose a fixture (`clean-profile` for most cases)
3. Define steps that exercise the feature via semantic targets
4. Add assertions to verify expected state
5. Add cleanup (usually `{ "type": "reset" }`)
6. Run `pnpm --dir testing run typecheck` to validate

Use `/godly-create-contract <name>` to generate a contract interactively.

## Fixing a Failing Contract

Use `/godly-fix-contract <contract-id>` to run a contract in a loop, diagnose failures, fix the code, and re-run until all steps pass.

## Artifacts

Each contract run produces artifacts in `testing/artifacts/<contract-id>-<timestamp>/`:
- Step traces (JSON per step with timing and assertion results)
- Screenshots (if snapshot steps are used)
- Overall result summary
