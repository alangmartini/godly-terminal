# Run a contract, diagnose failures, and fix until green

Run a test contract against Godly Staging, diagnose failures, fix the code, and loop until the contract passes. Use this to fix bugs caught by contracts or to validate that a feature works end-to-end.

## Usage

```
/godly-fix-contract <contract-id-or-path>
```

Examples:
- `/godly-fix-contract workspace-folder-path`
- `/godly-fix-contract contracts/workspace-folder-path.json`
- `/godly-fix-contract split-basic`

## Instructions

### Philosophy

**Never mask failures. Never add workarounds. Fix the actual bug.**

Contracts test real behavior through MCP tools against a running Godly Staging instance. Failures reveal genuine bugs — wrong state, missing implementation, broken persistence, race conditions. Every failure is a signal.

**Forbidden workarounds:**
- Weakening contract assertions to match broken behavior
- Adding try/catch to swallow errors in production code
- Increasing timeouts to paper over races
- Adding retry loops around flaky operations
- Changing the contract to skip failing steps
- Masking errors with fallback values

---

### Phase 1: Resolve Contract

1. If the argument is a contract ID (no path separators, no `.json`), resolve to `testing/contracts/<id>.json`
2. If the argument is a path, resolve it relative to the project root
3. Read the contract file and report: contract ID, description, step count, fixture, requires_restart

### Phase 2: Ensure Staging is Running

Before running the contract, verify Godly Staging is accessible:

1. Run a quick MCP probe:
   ```bash
   pnpm --dir testing run run-contract contracts/<id>.json 2>&1 | head -5
   ```
2. If connection fails, tell the user:
   > "Godly Staging must be running. Launch 'Godly Terminal (Staging)' from the Start Menu, then try again."
3. Do NOT proceed until staging is confirmed reachable.

### Phase 3: Run Contract (First Run)

```bash
pnpm --dir testing run run-contract contracts/<id>.json
```

Parse the output to extract:
- Overall result: PASS or FAIL
- For each step: step ID, passed/failed, error message, assertion details
- Artifact directory (for screenshots, traces)

**If the contract passes on first run** → skip to Phase 6 (report success).

### Phase 4: Diagnose & Fix (Loop)

For each iteration of the loop:

#### 4a. Identify the first failing step

Read the contract runner output. Find the first step that failed and note:
- Step ID and description
- Error message or failed assertion (expected vs actual)
- Which MCP tool was called (`ui_act`, `ui_query`, `ui_wait`)
- What target/action/condition was used

#### 4b. Classify the failure

| Failure type | Symptom | Where to fix |
|---|---|---|
| **Missing implementation** | `ui_act` returns error "unknown target/action" | Add handler in semantic adapter or backend |
| **Wrong state** | Assertion fails (expected X, got Y) | Fix the production code that produces the state |
| **Missing data** | Query returns null/undefined for expected field | Fix the backend command or MCP response |
| **Timing issue** | Wait times out | Fix the production code (don't just increase timeout) |
| **Fixture failure** | `fixture:<name>` step fails | Fix the fixture in `testing/fixtures/` |
| **Contract bug** | Step targets impossible state | Fix the contract (only if the contract is genuinely wrong) |
| **Adapter gap** | `ui_act`/`ui_query` doesn't handle the target | Implement the semantic adapter for this target |

#### 4c. Investigate root cause

Use targeted codebase searches to find the relevant code:

1. **For `ui_act`/`ui_query`/`ui_wait` failures**: Check how the semantic adapter routes the target. Search for the target string in `src-tauri/mcp/src/tools.rs` or relevant adapter code.
2. **For backend state issues**: Trace from the Tauri command through state management to the MCP response.
3. **For frontend issues**: Check the component/store/service that handles this feature.

Hypothesis first, then max 2 targeted searches to confirm.

#### 4d. Apply the fix

Edit the minimum code needed to fix the root cause. Follow existing patterns.

**If Rust code was changed:**
```bash
cd src-tauri && cargo check -p <modified-crate>
```

**If TypeScript was changed:**
```bash
pnpm test  # if unit tests exist for the changed code
```

**If the fix requires a staging rebuild:**
Ask the user before rebuilding:
> "The fix requires changes to [daemon/backend/MCP]. Need to rebuild staging. Proceed?"

If approved:
```bash
pnpm staging:build && pnpm staging:install
```

Wait for staging to restart before re-running the contract.

#### 4e. Re-run the contract

```bash
pnpm --dir testing run run-contract contracts/<id>.json
```

#### 4f. Evaluate

- **All steps pass** → break out of loop, go to Phase 5
- **Same step still fails** → re-investigate, the fix didn't work. Try a different approach.
- **Different step fails** → progress was made. Loop back to 4a for the new failure.
- **More steps pass than before** → progress. Continue looping.

### Phase 5: Loop Guard

**Maximum iterations: 5.**

If the contract still fails after 5 fix-run cycles:

1. Summarize all attempts: what was tried, what worked, what didn't
2. List remaining failing steps with their error messages
3. Present the diagnosis to the user and ask how to proceed:
   - Continue with more iterations
   - Change approach
   - File as a known issue

Do NOT keep looping silently — escalate after 5 rounds.

### Phase 6: Report

When the contract passes (or the loop guard triggers), present:

```
Contract: <id>
Result: PASS | FAIL (after N iterations)
Duration: <total time across all runs>

Fixes applied:
- <file>: <what was changed and why>

Steps:
  [PASS] step-1: <description>
  [PASS] step-2: <description>
  ...

Artifacts: testing/artifacts/<run-id>/
```

### Phase 7: Commit

If code changes were made to fix failures, commit them following conventional commits:
- `fix:` for production code fixes
- `test:` for contract/fixture corrections (only if the contract was genuinely wrong)

Use the git-workflow-manager agent for commits and PR creation.

---

## Key Files

| File | Purpose |
|------|---------|
| `testing/contracts/*.json` | Contract definitions |
| `testing/fixtures/*.ts` | Fixture setup/teardown |
| `testing/runner/contract-runner.ts` | Contract execution engine |
| `testing/runner/mcp-client.ts` | MCP stdio client |
| `testing/runner/assertions.ts` | Assertion evaluators |
| `src-tauri/mcp/src/tools.rs` | MCP tool definitions + semantic adapter routing |
| `src-tauri/src/commands/` | Tauri backend command handlers |
| `src/services/` | Frontend service wrappers |
| `src/state/` | Frontend state management |

## Running the Contract

```bash
# From project root
pnpm --dir testing run run-contract contracts/<id>.json
```

Requires Godly Staging to be running (installed via `pnpm staging:build && pnpm staging:install`).
