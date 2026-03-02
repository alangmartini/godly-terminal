# Fix Integration Tests

Run integration tests in a loop, diagnosing and fixing real failures until they pass.

## Usage

```
/fix-integration [test-filter]
```

- No argument: runs all integration tests
- With argument: passes it as a Vitest test name filter (e.g., `/fix-integration quick-claude`)

## Instructions

### Philosophy

**Never mask failures. Never add workarounds. Fix the actual bug.**

This skill exists because integration tests exercise real daemon I/O with real timing. Failures reveal genuine protocol bugs, race conditions, or timing issues in the Quick Claude flow. Every failure is a signal — investigate it, find the root cause, and fix the code.

Forbidden workarounds:
- Adding try/catch to swallow errors in test code
- Increasing timeouts to paper over races
- Adding retry loops around flaky assertions
- Wrapping failing steps in `if/else` fallbacks
- Skipping or `.todo()`-ing the failing test
- Changing assertions to be less specific

### Loop procedure

1. **Build daemon** (if not recently built):
   ```bash
   pnpm build:daemon
   ```

2. **Run integration tests**:
   ```bash
   # All tests
   pnpm test:integration

   # Or filtered
   pnpm exec vitest run --project integration -t "<filter>"
   ```

3. **If all tests pass** → done. Report the green result.

4. **If a test fails** → investigate:

   a. **Read the failure output carefully.** Identify the exact assertion that failed and the actual vs expected values.

   b. **Classify the failure:**
      - **Protocol mismatch** — TypeScript types don't match Rust serde output → fix `integration/protocol.ts`
      - **Timing race** — test assumes ordering that isn't guaranteed → fix the *production code* that should guarantee it (e.g., the daemon's response ordering), OR fix the test's polling logic to match what the production code actually does
      - **Daemon bug** — daemon returns wrong response or crashes → fix the daemon code, rebuild, re-run
      - **Test logic error** — test asserts something incorrect → fix the assertion to match correct behavior
      - **Environment issue** — `claude` not on PATH, pipe conflict, etc. → report to user, don't mask

   c. **Fix the root cause.** Edit the relevant source file (daemon Rust code, protocol types, test logic, or session-handle). If editing Rust, rebuild the daemon.

   d. **Go to step 2.** Re-run the tests. Repeat until green.

5. **Maximum iterations: 5.** If still failing after 5 loops, stop and report the diagnosis to the user. Do NOT keep looping — there may be a deeper issue that needs human judgement.

### Key source files

| File | What it contains |
|------|------------------|
| `integration/protocol.ts` | TS types matching `protocol/src/messages.rs` + `types.rs` |
| `integration/daemon-client.ts` | Named pipe client, binary/JSON frame dispatch |
| `integration/daemon-fixture.ts` | Isolated daemon spawn per test suite |
| `integration/session-handle.ts` | High-level session API (write, wait, read, search) |
| `integration/tests/smoke.integration.test.ts` | Daemon lifecycle + session I/O |
| `integration/tests/quick-claude.integration.test.ts` | Quick Claude e2e flow |
| `src-tauri/protocol/src/messages.rs` | Rust Request/Response/Event types |
| `src-tauri/protocol/src/frame.rs` | Wire protocol (binary + JSON framing) |
| `src-tauri/daemon/src/server.rs` | Daemon request handler |
| `src-tauri/src/commands/terminal.rs` | Quick Claude background flow (lines 533-738) |

### After fixing

- If you changed Rust daemon code, rebuild: `pnpm build:daemon`
- If you changed integration test framework files, just re-run tests
- Commit fixes following conventional commits (`fix:` for bugs, `test:` for test corrections)
- Report what broke, why, and what you fixed
