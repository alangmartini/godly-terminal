# Write a test contract for an existing or new feature

Create or update a staging test contract in `testing/contracts/`, including fixture work in `testing/fixtures/` when needed.

## Usage

```
/godly-create-contract <feature-name-or-contract-id>
```

Examples:
- `/godly-create-contract workspace persistence`
- `/godly-create-contract split-basic`
- `/godly-create-contract quick-claude`

## Instructions

### 1. Read the contract surface

Read these files before making any changes:
- `testing/runner/types.ts` — `Contract`, `Step`, `CleanupStep`, `Assertion` interfaces
- `testing/fixtures/index.ts` — registered fixtures
- `testing/fixtures/types.ts` — `Fixture` interface, helpers (`resetProfile`, `waitForReady`, `getActiveWorkspaceId`, `listTerminalIds`)
- One or two existing contracts in `testing/contracts/*.json` for format reference

Do NOT read all contracts — just enough to understand the shape.

### 2. Decide: new or update

- If a contract already covers this feature, update it
- If coverage is missing, create a new contract
- If a fixture already covers the needed setup, reuse it
- Only add a fixture when deterministic setup cannot be expressed with existing ones

### 3. Author the contract

Create `testing/contracts/<id>.json` following these rules:

**Contract shape:**
```json
{
  "id": "<hyphen-case-id>",
  "description": "<one-line summary of what this contract verifies>",
  "frontends": ["web", "native"],
  "fixture": "<registered-fixture-name>",
  "requires_restart": false,
  "steps": [],
  "cleanup": []
}
```

**Step types:**
- `action` — performs a mutation via `ui_act(target, action, args?)`
- `query` / `assert` — reads state via `ui_query(target, args?)`, checks `assertions`
- `wait` — polls a condition via `ui_wait(condition, timeout_ms, args?)`
- `snapshot` — captures a screenshot via `capture_screenshot`

**Semantic targets available:**
- `workspace.active`, `workspace.list`, `workspace.switched`
- `terminal.count`, `terminal.created`
- `layout.tree`
- `app.ready`, `app.lifecycle` (action: `restart`)
- `app` (action: `save_layout`)

**Assertion types:** `equals`, `gt`, `gte`, `lt`, `lte`, `not_null`, `contains`

**Restart pattern (only when `requires_restart: true`):**
```json
{ "id": "restart-app", "type": "action", "target": "app.lifecycle", "action": "restart" },
{ "id": "wait-app-ready", "type": "wait", "condition": "app.ready", "timeout_ms": 30000 }
```

**Cleanup:** Use `{ "type": "reset" }` to restore clean profile. Add targeted cleanup actions before reset if needed.

**Style rules:**
- Use stable, descriptive step IDs (e.g., `verify-terminals-restored`, not `step-7`)
- Prefer semantic assertions over visual snapshots
- Set `on_failure: "continue"` only when later steps don't depend on this step
- Default `on_failure` is `"abort"` — failing step skips the rest

### 4. Add a fixture only if needed

If no existing fixture covers the required setup:

1. Create `testing/fixtures/<name>.ts`
2. Import helpers from `./types`
3. Implement `create()`, `verifyReady()`, `tearDown()`
4. Register in `testing/fixtures/index.ts`

Fixture rules:
- `create()` must be deterministic — same state every time
- `tearDown()` must be idempotent — safe to call multiple times
- Use MCP tools via the client, not ad hoc JSON-RPC
- Always start with `resetProfile(client)` in `create()`

### 5. Validate

Always run:
```bash
pnpm --dir testing run typecheck
```

### 6. Report

Summarize:
- Which contract file was created or changed
- Whether any fixture was added or changed
- Typecheck result
- Any missing semantic adapter work that blocks full execution

Always end with the command to run the contract:
```
pnpm --dir testing run run-contract contracts/<id>.json
```
