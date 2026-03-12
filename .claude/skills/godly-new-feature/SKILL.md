# Design a new feature: interview, spec, plan, and contract

Describe a feature through a brief interview, define acceptance criteria, create an implementation plan, and generate a test contract. Use this when starting a new feature from scratch — it produces a full spec before any code is written.

## Usage

```
/godly-new-feature <feature-name> [description]
```

Examples:
- `/godly-new-feature tab-pinning Allow users to pin tabs so they stay leftmost`
- `/godly-new-feature workspace-rename Rename workspaces via double-click on sidebar`
- `/godly-new-feature split-resize Drag-to-resize split panes with visual feedback`

## Instructions

Follow all phases in order. Do not skip phases. The test contract (Phase 5) is a **mandatory artifact** — the skill has not completed successfully unless a contract file exists in `testing/contracts/`.

---

### Phase 1: Feature Interview

Conduct a brief, focused interview to understand the feature before doing any codebase analysis. Use AskUserQuestion for each round.

**Round 1 — Core behavior:**
Ask 3-5 questions covering:
- What is the primary user action and expected outcome?
- Where in the UI does this live? (which component, what interaction trigger)
- Are there any specific UX details? (animation, feedback, keyboard shortcut, visual indicator)

**Round 2 — Edge cases & scope** (after Round 1 answers):
Based on the answers, ask 2-4 follow-up questions:
- What happens in boundary conditions? (empty state, max limits, concurrent actions)
- Does this interact with existing features? (persistence, workspaces, splits, tabs)
- What's explicitly out of scope for v1?
- Any platform-specific considerations? (Windows-only behaviors, shell types)

**Round 3 — Confirmation** (optional, only if ambiguity remains):
If any critical detail is still unclear after Round 2, ask ONE focused clarifying question. Otherwise, proceed.

**Rules for the interview:**
- Keep questions concrete and specific to THIS feature — no generic checklists
- Reference existing Godly Terminal patterns when relevant (e.g., "Should this follow the same pattern as workspace switching?")
- Never ask questions you can answer from the codebase — save those for Phase 2
- Summarize your understanding after the last round before proceeding

### Phase 2: Codebase Analysis

Use Glob/Grep/Read (no Explore agents) to identify:

1. **Frontend files** that will be modified (components, store modules, services, styles)
2. **Backend files** that will be modified (Rust crates: protocol, daemon, tauri commands, persistence)
3. **Existing patterns** to follow (find the closest analogous feature and note how it's implemented)
4. **Test tiers** needed (use the decision tree in CLAUDE.md)

Produce a summary table:

```
Affected Areas:
| Layer     | Files                          | Change Type |
|-----------|--------------------------------|-------------|
| Frontend  | src/components/TabBar.ts       | Modify      |
| Store     | src/state/store-terminal.ts    | Modify      |
| Backend   | src-tauri/src/commands/...      | Modify      |
| Protocol  | protocol/src/messages.rs       | Add variant |
| Tests     | src/**/*.test.ts               | New         |
```

### Phase 2b: Already-Implemented Check

If Phase 2 reveals the feature is **already fully implemented**:

1. Present the findings to the user with a summary table showing each requirement mapped to existing code
2. **Still proceed to Phase 3** (acceptance criteria) — define what "working correctly" means
3. **Skip Phase 4** (implementation plan) — nothing to build
4. **Still proceed to Phase 5** (create test contract) — the contract verifies existing behavior and catches regressions
5. Adjust Phase 6 summary to note: "Feature already implemented. Contract added for regression coverage."
6. In Phase 7, replace "Proceed with `/feature`" with "No implementation needed — contract covers regression."

Do NOT stop the skill early just because no code changes are needed. The contract is the primary artifact.

### Phase 3: Acceptance Criteria

Write concrete, testable acceptance criteria derived from the interview answers:

```
Acceptance Criteria:
1. User can <action> and <expected result>
2. <State> persists across <event> (restart, workspace switch, etc.)
3. <Edge case> is handled by <behavior>
```

Each criterion should map directly to one or more test contract steps. Number them — they'll be referenced in the contract and implementation plan.

### Phase 4: Implementation Plan

Create a step-by-step implementation plan that can be handed to `/feature` or executed manually.

**Structure each step as:**

```
Step N: <title>
  Layer: frontend | backend | protocol | persistence | test
  Files: <specific files to modify or create>
  What: <concise description of the change>
  Depends on: <step numbers, or "none">
  Criteria: <which acceptance criteria this step satisfies>
```

**Plan rules:**
- Order steps by dependency (protocol first if new types needed, then backend, then frontend)
- Identify which steps can run in parallel (for multi-agent execution via `/feature`)
- Call out the critical path explicitly
- Include test steps — which test tier and what to test
- Reference the analogous pattern found in Phase 2 (e.g., "Follow the same pattern as `create_workspace` in `commands/workspace.rs`")

**Present a parallelism diagram:**

```
Parallelism:
  [1: Protocol types] ──→ [2: Daemon handler] ──→ [5: Integration tests]
  [3: Frontend UI] ─────────────────────────────→ [6: Browser tests]
  [4: Store logic] ─────→ [5]

  Critical path: 1 → 2 → 5
  Parallel groups: {1}, {3,4}, {2}, {5,6}
```

### Phase 5: Create Test Contract

Invoke the `godly-create-contract` skill to create the test contract:

```
/godly-create-contract <feature-name>
```

**Important**: When creating the contract, map each acceptance criterion from Phase 3 to concrete steps. The contract should cover:

- **Setup**: Use an existing fixture if possible, create a new one only if needed
- **Happy path**: The primary feature workflow
- **Persistence** (if applicable): Save → restart → verify state survived
- **Edge cases**: At least one edge case from the acceptance criteria

If the skill tool is not available, follow the instructions from `.claude/skills/godly-create-contract.md` directly.

### Phase 5b: Verify Contract Exists

**This is a hard gate. Do NOT proceed to Phase 6 without passing this check.**

After Phase 5 completes, verify the contract file was actually created:

1. Run `ls testing/contracts/<feature-name>.json` (or the expected contract filename)
2. If the file does **not** exist:
   - State clearly: "Contract verification FAILED — file not found at `testing/contracts/<expected-name>.json`"
   - Re-attempt Phase 5 (create the contract)
   - If the second attempt also fails, ask the user how to proceed
3. If the file exists, run `pnpm --dir testing run typecheck` to validate it
4. Report: "Contract verified: `testing/contracts/<name>.json` (<N steps>)"

This gate exists because earlier phases (interview, analysis, planning) produce no artifacts — the contract is the only durable output of this skill. Skipping it makes the entire skill run worthless.

### Phase 6: Summary

Present the final summary to the user:

```
Feature: <name>
Interview: <N rounds, key decisions made>
Affected: <N frontend files>, <N backend files>
Plan: <N steps>, <N parallel groups>, critical path: <steps>
Contract: testing/contracts/<id>.json (<N steps>)
Fixture: <existing or new fixture name>
Criteria: <N acceptance criteria> → <N contract steps>

Ready for implementation via /feature <name>
```

### Phase 7: User Confirmation

Ask the user if they want to:
1. **Proceed** — run `/feature <name>` to start implementation
2. **Adjust** — modify the plan, criteria, or contract before starting
3. **Save for later** — keep the spec as-is for future implementation

---

## Notes

- This skill does NOT write implementation code — it produces a spec, plan, and test contract
- The contract serves as the feature's definition of done
- The implementation plan feeds directly into `/feature`'s Phase 2 (it can skip its own analysis)
- Even trivial features need a contract — a 1-step contract is fine. Never skip it.
- Always run `pnpm --dir testing run typecheck` after creating the contract
