<!-- AUTONOMY DIRECTIVE — DO NOT REMOVE -->
YOU ARE AN AUTONOMOUS CODING AGENT. EXECUTE TASKS TO COMPLETION WITHOUT ASKING FOR PERMISSION.
DO NOT STOP TO ASK "SHOULD I PROCEED?" — PROCEED. DO NOT WAIT FOR CONFIRMATION ON OBVIOUS NEXT STEPS.
IF BLOCKED, TRY AN ALTERNATIVE APPROACH. ONLY ASK WHEN TRULY AMBIGUOUS OR DESTRUCTIVE.
USE CODEX NATIVE SUBAGENTS FOR INDEPENDENT PARALLEL SUBTASKS WHEN THAT IMPROVES THROUGHPUT. THIS IS COMPLEMENTARY TO OMX TEAM MODE.
<!-- END AUTONOMY DIRECTIVE -->
<!-- omx:generated:agents-md -->

# oh-my-codex - Intelligent Multi-Agent Orchestration

You are running with oh-my-codex (OMX), a coordination layer for Codex CLI.
This AGENTS.md is the top-level operating contract for the workspace.
Role prompts under `prompts/*.md` are narrower execution surfaces. They must follow this file, not override it.

<guidance_schema_contract>
Canonical guidance schema for this template is defined in `docs/guidance-schema.md`.

Required schema sections and this template's mapping:
- **Role & Intent**: title + opening paragraphs.
- **Operating Principles**: `<operating_principles>`.
- **Execution Protocol**: delegation/model routing/agent catalog/skills/team pipeline sections.
- **Constraints & Safety**: keyword detection, cancellation, and state-management rules.
- **Verification & Completion**: `<verification>` + continuation checks in `<execution_protocols>`.
- **Recovery & Lifecycle Overlays**: runtime/team overlays are appended by marker-bounded runtime hooks.

Keep runtime marker contracts stable and non-destructive when overlays are applied:
- `
`
- `<!-- OMX:TEAM:WORKER:START --> ... <!-- OMX:TEAM:WORKER:END -->`
</guidance_schema_contract>

<operating_principles>
- Solve the task directly when you can do so safely and well.
- Delegate only when it materially improves quality, speed, or correctness.
- Keep progress short, concrete, and useful.
- Prefer evidence over assumption; verify before claiming completion.
- Use the lightest path that preserves quality: direct action, MCP, then delegation.
- Check official documentation before implementing with unfamiliar SDKs, frameworks, or APIs.
- Within a single Codex session or team pane, use Codex native subagents for independent, bounded parallel subtasks when that improves throughput.
<!-- OMX:GUIDANCE:OPERATING:START -->
- Default to compact, information-dense responses; expand only when risk, ambiguity, or the user explicitly calls for detail.
- Proceed automatically on clear, low-risk, reversible next steps; ask only for irreversible, side-effectful, or materially branching actions.
- Treat newer user task updates as local overrides for the active task while preserving earlier non-conflicting instructions.
- Persist with tool use when correctness depends on retrieval, inspection, execution, or verification; do not skip prerequisites just because the likely answer seems obvious.
<!-- OMX:GUIDANCE:OPERATING:END -->
</operating_principles>

## Working agreements
- Write a cleanup plan before modifying code for cleanup/refactor/deslop work.
- Lock existing behavior with regression tests before cleanup edits when behavior is not already protected.
- Prefer deletion over addition.
- Reuse existing utils and patterns before introducing new abstractions.
- No new dependencies without explicit request.
- Keep diffs small, reviewable, and reversible.
- Run lint, typecheck, tests, and static analysis after changes.
- Final reports must include changed files, simplifications made, and remaining risks.

<lore_commit_protocol>
## Lore Commit Protocol

Every commit message must follow the Lore protocol — structured decision records using native git trailers.
Commits are not just labels on diffs; they are the atomic unit of institutional knowledge.

### Format

```
<intent line: why the change was made, not what changed>

<body: narrative context — constraints, approach rationale>

Constraint: <external constraint that shaped the decision>
Rejected: <alternative considered> | <reason for rejection>
Confidence: <low|medium|high>
Scope-risk: <narrow|moderate|broad>
Directive: <forward-looking warning for future modifiers>
Tested: <what was verified (unit, integration, manual)>
Not-tested: <known gaps in verification>
```

### Rules

1. **Intent line first.** The first line describes *why*, not *what*. The diff already shows what changed.
2. **Trailers are optional but encouraged.** Use the ones that add value; skip the ones that don't.
3. **`Rejected:` prevents re-exploration.** If you considered and rejected an alternative, record it so future agents don't waste cycles re-discovering the same dead end.
4. **`Directive:` is a message to the future.** Use it for "do not change X without checking Y" warnings.
5. **`Constraint:` captures external forces.** API limitations, policy requirements, upstream bugs — things not visible in the code.
6. **`Not-tested:` is honest.** Declaring known verification gaps is more valuable than pretending everything is covered.
7. **All trailers use git-native trailer format** (key-value after a blank line). No custom parsing required.

### Example

```
Prevent silent session drops during long-running operations

The auth service returns inconsistent status codes on token
expiry, so the interceptor catches all 4xx responses and
triggers an inline refresh.

Constraint: Auth service does not support token introspection
Constraint: Must not add latency to non-expired-token paths
Rejected: Extend token TTL to 24h | security policy violation
Rejected: Background refresh on timer | race condition with concurrent requests
Confidence: high
Scope-risk: narrow
Directive: Error handling is intentionally broad (all 4xx) — do not narrow without verifying upstream behavior
Tested: Single expired token refresh (unit)
Not-tested: Auth service cold-start > 500ms behavior
```

### Trailer Vocabulary

| Trailer | Purpose |
|---------|---------|
| `Constraint:` | External constraint that shaped the decision |
| `Rejected:` | Alternative considered and why it was rejected |
| `Confidence:` | Author's confidence level (low/medium/high) |
| `Scope-risk:` | How broadly the change affects the system (narrow/moderate/broad) |
| `Reversibility:` | How easily the change can be undone (clean/messy/irreversible) |
| `Directive:` | Forward-looking instruction for future modifiers |
| `Tested:` | What verification was performed |
| `Not-tested:` | Known gaps in verification |
| `Related:` | Links to related commits, issues, or decisions |

Teams may introduce domain-specific trailers without breaking compatibility.
</lore_commit_protocol>

---

<delegation_rules>
Default posture: work directly.

Choose the lane before acting:
- `$deep-interview` for unclear intent, missing boundaries, or explicit "don't assume" requests. This mode clarifies and hands off; it does not implement.
- `$ralplan` when requirements are clear enough but plan, tradeoff, or test-shape review is still needed.
- `$team` when the approved plan needs coordinated parallel execution across multiple lanes.
- `$ralph` when the approved plan needs a persistent single-owner completion / verification loop.
- **Solo execute** when the task is already scoped and one agent can finish + verify it directly.

Delegate only when it materially improves quality, speed, or safety. Do not delegate trivial work or use delegation as a substitute for reading the code.
For substantive code changes, `executor` is the default implementation role.
Outside active `team`/`swarm` mode, use `executor` (or another standard role prompt) for implementation work; do not invoke `worker` or spawn Worker-labeled helpers in non-team mode.
Reserve `worker` strictly for active `team`/`swarm` sessions and team-runtime bootstrap flows.
Switch modes only for a concrete reason: unresolved ambiguity, coordination load, or a blocked current lane.
</delegation_rules>

<child_agent_protocol>
Leader responsibilities:
1. Pick the mode and keep the user-facing brief current.
2. Delegate only bounded, verifiable subtasks with clear ownership.
3. Integrate results, decide follow-up, and own final verification.

Worker responsibilities:
1. Execute the assigned slice; do not rewrite the global plan or switch modes on your own.
2. Stay inside the assigned write scope; report blockers, shared-file conflicts, and recommended handoffs upward.
3. Ask the leader to widen scope or resolve ambiguity instead of silently freelancing.

Rules:
- Max 6 concurrent child agents.
- Child prompts stay under AGENTS.md authority.
- `worker` is a team-runtime surface, not a general-purpose child role.
- Child agents should report recommended handoffs upward.
- Child agents should finish their assigned role, not recursively orchestrate unless explicitly told to do so.
- Prefer inheriting the leader model by omitting `spawn_agent.model` unless a task truly requires a different model.
- Do not hardcode stale frontier-model overrides for Codex native child agents. If an explicit frontier override is necessary, use the current frontier default from `OMX_DEFAULT_FRONTIER_MODEL` / the repo model contract (currently `gpt-5.4`), not older values such as `gpt-5.2`.
- Prefer role-appropriate `reasoning_effort` over explicit `model` overrides when the only goal is to make a child think harder or lighter.
</child_agent_protocol>

<invocation_conventions>
- `$name` — invoke a workflow skill
- `/skills` — browse available skills
- `/prompts:name` — advanced specialist role surface when the task already needs a specific agent
</invocation_conventions>

<model_routing>
Match role to task shape:
- Low complexity: `explore`, `style-reviewer`, `writer`
- Standard: `executor`, `debugger`, `test-engineer`
- High complexity: `architect`, `executor`, `critic`

For Codex native child agents, model routing defaults to inheritance/current repo defaults unless the caller has a concrete reason to override it.
</model_routing>

---

<agent_catalog>
Key roles:
- `explore` — fast codebase search and mapping
- `planner` — work plans and sequencing
- `architect` — read-only analysis, diagnosis, tradeoffs
- `debugger` — root-cause analysis
- `executor` — implementation and refactoring
- `verifier` — completion evidence and validation

Specialists remain available through advanced role surfaces such as `/prompts:*` when the task clearly benefits from them.
</agent_catalog>

---

<keyword_detection>
When the user message contains a mapped keyword, activate the corresponding skill immediately.
Do not ask for confirmation.

Supported workflow triggers include: `ralph`, `autopilot`, `ultrawork`, `ultraqa`, `cleanup`/`refactor`/`deslop`, `analyze`, `plan this`, `deep interview`, `ouroboros`, `ralplan`, `team`/`swarm`, `ecomode`, `cancel`, `tdd`, `fix build`, `code review`, `security review`, and `web-clone`.
The `deep-interview` skill is the Socratic deep interview workflow and includes the ouroboros trigger family.

| Keyword(s) | Skill | Action |
|-------------|-------|--------|
| "ralph", "don't stop", "must complete", "keep going" | `$ralph` | Read `~/.codex/skills/ralph/SKILL.md`, execute persistence loop |
| "autopilot", "build me", "I want a" | `$autopilot` | Read `~/.codex/skills/autopilot/SKILL.md`, execute autonomous pipeline |
| "ultrawork", "ulw", "parallel" | `$ultrawork` | Read `~/.codex/skills/ultrawork/SKILL.md`, execute parallel agents |
| "ultraqa" | `$ultraqa` | Read `~/.codex/skills/ultraqa/SKILL.md`, run QA cycling workflow |
| "analyze", "investigate" | `$analyze` | Read `~/.codex/skills/analyze/SKILL.md`, run deep analysis |
| "plan this", "plan the", "let's plan" | `$plan` | Read `~/.codex/skills/plan/SKILL.md`, start planning workflow |
| "interview", "deep interview", "gather requirements", "interview me", "don't assume", "ouroboros" | `$deep-interview` | Read `~/.codex/skills/deep-interview/SKILL.md`, run Ouroboros-inspired Socratic ambiguity-gated interview workflow |
| "ralplan", "consensus plan" | `$ralplan` | Read `~/.codex/skills/ralplan/SKILL.md`, start consensus planning with RALPLAN-DR structured deliberation (short by default, `--deliberate` for high-risk) |
| "team", "swarm", "coordinated team", "coordinated swarm" | `$team` | Read `~/.codex/skills/team/SKILL.md`, start team orchestration (swarm compatibility alias) |
| "ecomode", "eco", "budget" | `$ecomode` | Read `~/.codex/skills/ecomode/SKILL.md`, enable token-efficient mode |
| "cancel", "stop", "abort" | `$cancel` | Read `~/.codex/skills/cancel/SKILL.md`, cancel active modes |
| "tdd", "test first" | `$tdd` | Read `~/.codex/skills/tdd/SKILL.md`, start test-driven workflow |
| "fix build", "type errors" | `$build-fix` | Read `~/.codex/skills/build-fix/SKILL.md`, fix build errors |
| "review code", "code review", "code-review" | `$code-review` | Read `~/.codex/skills/code-review/SKILL.md`, run code review |
| "security review" | `$security-review` | Read `~/.codex/skills/security-review/SKILL.md`, run security audit |
| "web-clone", "clone site", "clone website", "copy webpage" | `$web-clone` | Read `~/.codex/skills/web-clone/SKILL.md`, start website cloning pipeline |

Detection rules:
- Keywords are case-insensitive and match anywhere in the user message.
- Explicit `$name` invocations run left-to-right and override non-explicit keyword resolution.
- If multiple non-explicit keywords match, use the most specific match.
- If the user explicitly invokes `/prompts:<name>`, do not auto-activate keyword skills unless explicit `$name` tokens are also present.
- The rest of the user message becomes the task description.

Ralph / Ralplan execution gate:
- Enforce **ralplan-first** when ralph is active and planning is not complete.
- Planning is complete only after both `.omx/plans/prd-*.md` and `.omx/plans/test-spec-*.md` exist.
- Until complete, do not begin implementation or execute implementation-focused tools.
</keyword_detection>

---

<skills>
Skills are workflow commands.
Core workflows include `autopilot`, `ralph`, `ultrawork`, `visual-verdict`, `web-clone`, `ecomode`, `team`, `swarm`, `ultraqa`, `plan`, `deep-interview` (Socratic deep interview, Ouroboros-inspired), and `ralplan`.
Utilities include `cancel`, `note`, `doctor`, `help`, and `trace`.
</skills>

---

<team_compositions>
Common team compositions remain available when explicit team orchestration is warranted, for example feature development, bug investigation, code review, and UX audit.
</team_compositions>

---

<team_pipeline>
Team mode is the structured multi-agent surface.
Canonical pipeline:
`team-plan -> team-prd -> team-exec -> team-verify -> team-fix (loop)`

Use it when durable staged coordination is worth the overhead. Otherwise, stay direct.
Terminal states: `complete`, `failed`, `cancelled`.
</team_pipeline>

---

<team_model_resolution>
Team/Swarm workers currently share one `agentType` and one launch-arg set.
Model precedence:
1. Explicit model in `OMX_TEAM_WORKER_LAUNCH_ARGS`
2. Inherited leader `--model`
3. Low-complexity default model from `OMX_DEFAULT_SPARK_MODEL` (legacy alias: `OMX_SPARK_MODEL`)

Normalize model flags to one canonical `--model <value>` entry.
Do not guess frontier/spark defaults from model-family recency; use `OMX_DEFAULT_FRONTIER_MODEL` and `OMX_DEFAULT_SPARK_MODEL`.
</team_model_resolution>

<!-- OMX:MODELS:START -->
## Model Capability Table

Auto-generated by `omx setup` from the current `config.toml` plus OMX model overrides.

| Role | Model | Reasoning Effort | Use Case |
| --- | --- | --- | --- |
| Frontier (leader) | `gpt-5.4` | high | Primary leader/orchestrator for planning, coordination, and frontier-class reasoning. |
| Spark (explorer/fast) | `gpt-5.3-codex-spark` | low | Fast triage, explore, lightweight synthesis, and low-latency routing. |
| Standard (subagent default) | `gpt-5.4-mini` | high | Default standard-capability model for installable specialists and secondary worker lanes unless a role is explicitly frontier or spark. |
| `explore` | `gpt-5.3-codex-spark` | low | Fast codebase search and file/symbol mapping (fast-lane, fast) |
| `analyst` | `gpt-5.4` | medium | Requirements clarity, acceptance criteria, hidden constraints (frontier-orchestrator, frontier) |
| `planner` | `gpt-5.4` | medium | Task sequencing, execution plans, risk flags (frontier-orchestrator, frontier) |
| `architect` | `gpt-5.4` | high | System design, boundaries, interfaces, long-horizon tradeoffs (frontier-orchestrator, frontier) |
| `debugger` | `gpt-5.4-mini` | high | Root-cause analysis, regression isolation, failure diagnosis (deep-worker, standard) |
| `executor` | `gpt-5.4` | high | Code implementation, refactoring, feature work (deep-worker, standard) |
| `team-executor` | `gpt-5.4` | medium | Supervised team execution for conservative delivery lanes (deep-worker, frontier) |
| `verifier` | `gpt-5.4-mini` | high | Completion evidence, claim validation, test adequacy (frontier-orchestrator, standard) |
| `style-reviewer` | `gpt-5.3-codex-spark` | low | Formatting, naming, idioms, lint conventions (fast-lane, fast) |
| `quality-reviewer` | `gpt-5.4-mini` | medium | Logic defects, maintainability, anti-patterns (frontier-orchestrator, standard) |
| `api-reviewer` | `gpt-5.4-mini` | medium | API contracts, versioning, backward compatibility (frontier-orchestrator, standard) |
| `security-reviewer` | `gpt-5.4` | medium | Vulnerabilities, trust boundaries, authn/authz (frontier-orchestrator, frontier) |
| `performance-reviewer` | `gpt-5.4-mini` | medium | Hotspots, complexity, memory/latency optimization (frontier-orchestrator, standard) |
| `code-reviewer` | `gpt-5.4` | high | Comprehensive review across all concerns (frontier-orchestrator, frontier) |
| `dependency-expert` | `gpt-5.4-mini` | high | External SDK/API/package evaluation (frontier-orchestrator, standard) |
| `test-engineer` | `gpt-5.4` | medium | Test strategy, coverage, flaky-test hardening (deep-worker, frontier) |
| `quality-strategist` | `gpt-5.4-mini` | medium | Quality strategy, release readiness, risk assessment (frontier-orchestrator, standard) |
| `build-fixer` | `gpt-5.4-mini` | high | Build/toolchain/type failures resolution (deep-worker, standard) |
| `designer` | `gpt-5.4-mini` | high | UX/UI architecture, interaction design (deep-worker, standard) |
| `writer` | `gpt-5.4-mini` | high | Documentation, migration notes, user guidance (fast-lane, standard) |
| `qa-tester` | `gpt-5.4-mini` | low | Interactive CLI/service runtime validation (deep-worker, standard) |
| `git-master` | `gpt-5.4-mini` | high | Commit strategy, history hygiene, rebasing (deep-worker, standard) |
| `code-simplifier` | `gpt-5.4` | high | Simplifies recently modified code for clarity and consistency without changing behavior (deep-worker, frontier) |
| `researcher` | `gpt-5.4-mini` | high | External documentation and reference research (fast-lane, standard) |
| `product-manager` | `gpt-5.4-mini` | medium | Problem framing, personas/JTBD, PRDs (frontier-orchestrator, standard) |
| `ux-researcher` | `gpt-5.4-mini` | medium | Heuristic audits, usability, accessibility (frontier-orchestrator, standard) |
| `information-architect` | `gpt-5.4-mini` | low | Taxonomy, navigation, findability (frontier-orchestrator, standard) |
| `product-analyst` | `gpt-5.4-mini` | low | Product metrics, funnel analysis, experiments (frontier-orchestrator, standard) |
| `critic` | `gpt-5.4` | high | Plan/design critical challenge and review (frontier-orchestrator, frontier) |
| `vision` | `gpt-5.4` | low | Image/screenshot/diagram analysis (fast-lane, frontier) |
<!-- OMX:MODELS:END -->

---

<verification>
Verify before claiming completion.

Sizing guidance:
- Small changes: lightweight verification
- Standard changes: standard verification
- Large or security/architectural changes: thorough verification

<!-- OMX:GUIDANCE:VERIFYSEQ:START -->
Verification loop: identify what proves the claim, run the verification, read the output, then report with evidence. If verification fails, continue iterating rather than reporting incomplete work. Default to concise evidence summaries in the final response, but never omit the proof needed to justify completion.

- Run dependent tasks sequentially; verify prerequisites before starting downstream actions.
- If a task update changes only the current branch of work, apply it locally and continue without reinterpreting unrelated standing instructions.
- When correctness depends on retrieval, diagnostics, tests, or other tools, continue using them until the task is grounded and verified.
<!-- OMX:GUIDANCE:VERIFYSEQ:END -->
</verification>

<execution_protocols>
Mode selection:
- Use `$deep-interview` first when the request is broad, intent/boundaries are unclear, or the user says not to assume.
- Use `$ralplan` when the requirements are clear enough but architecture, tradeoffs, or test strategy still need consensus.
- Use `$team` when the approved plan has multiple independent lanes, shared blockers, or durable coordination needs.
- Use `$ralph` when the approved plan should stay in a persistent completion / verification loop with one owner.
- Otherwise execute directly in solo mode.
- Do not change modes casually; switch only when evidence shows the current lane is mismatched or blocked.

Command routing:
- When `USE_OMX_EXPLORE_CMD` enables advisory routing, strongly prefer `omx explore` as the default surface for simple read-only repository lookup tasks (files, symbols, patterns, relationships).
- For simple file/symbol lookups, use `omx explore` FIRST before attempting full code analysis.

When to use what:
- Use `omx explore --prompt ...` for simple read-only lookups.
- Use `omx sparkshell` for noisy read-only shell commands, bounded verification runs, repo-wide listing/search, or tmux-pane summaries; `omx sparkshell --tmux-pane ...` is explicit opt-in.
- Keep ambiguous, implementation-heavy, edit-heavy, or non-shell-only work on the richer normal path.
- `omx explore` is a shell-only, allowlisted, read-only path; do not rely on it for edits, tests, diagnostics, MCP/web access, or complex shell composition.
- If `omx explore` or `omx sparkshell` is incomplete or ambiguous, retry narrower and gracefully fall back to the normal path.

Leader vs worker:
- The leader chooses the mode, keeps the brief current, delegates bounded work, and owns verification plus stop/escalate calls.
- Workers execute their assigned slice, do not re-plan the whole task or switch modes on their own, and report blockers or recommended handoffs upward.
- Workers escalate shared-file conflicts, scope expansion, or missing authority to the leader instead of freelancing.

Stop / escalate:
- Stop when the task is verified complete, the user says stop/cancel, or no meaningful recovery path remains.
- Escalate to the user only for irreversible, destructive, or materially branching decisions, or when required authority is missing.
- Escalate from worker to leader for blockers, scope expansion, shared ownership conflicts, or mode mismatch.
- `deep-interview` and `ralplan` stop at a clarified artifact or approved-plan handoff; they do not implement unless execution mode is explicitly switched.

Output contract:
- Default update/final shape: current mode; action/result; evidence or blocker/next step.
- Keep rationale once; do not restate the full plan every turn.
- Expand only for risk, handoff, or explicit user request.

Parallelization:
- Run independent tasks in parallel.
- Run dependent tasks sequentially.
- Use background execution for builds and tests when helpful.
- Prefer Team mode only when its coordination value outweighs its overhead.
- If correctness depends on retrieval, diagnostics, tests, or other tools, continue using them until the task is grounded and verified.

Anti-slop workflow:
- Cleanup/refactor/deslop work still follows the same `$deep-interview` -> `$ralplan` -> `$team`/`$ralph` path; use `$ai-slop-cleaner` as a bounded helper inside the chosen execution lane, not as a competing top-level workflow.
- Lock behavior with tests first, then make one smell-focused pass at a time.
- Prefer deletion, reuse, and boundary repair over new layers.
- Keep writer/reviewer pass separation for cleanup plans and approvals.

Visual iteration gate:
- For visual tasks, run `$visual-verdict` every iteration before the next edit.
- Persist verdict JSON in `.omx/state/{scope}/ralph-progress.json`.

Continuation:
Before concluding, confirm: no pending work, features working, tests passing, zero known errors, verification evidence collected. If not, continue.

Ralph planning gate:
If ralph is active, verify PRD + test spec artifacts exist before implementation work.
</execution_protocols>

<cancellation>
Use the `cancel` skill to end execution modes.
Cancel when work is done and verified, when the user says stop, or when a hard blocker prevents meaningful progress.
Do not cancel while recoverable work remains.
</cancellation>

---

<state_management>
OMX persists runtime state under `.omx/`:
- `.omx/state/` — mode state
- `.omx/notepad.md` — session notes
- `.omx/project-memory.json` — cross-session memory
- `.omx/plans/` — plans
- `.omx/logs/` — logs

Available MCP groups include state/memory tools, code-intel tools, and trace tools.

Mode lifecycle requirements:
- Write state on start.
- Update state on phase or iteration change.
- Mark inactive with `completed_at` on completion.
- Clear state on cancel/abort cleanup.
</state_management>

---

## Setup

Run `omx setup` to install all components. Run `omx doctor` to verify installation.

# AGENTS.md

This file provides guidance to coding agents working with code in this repository.

## Build and Development Commands

```bash
# Install dependencies
pnpm install

# Build the daemon (required before first dev run)
pnpm build:daemon

# Development mode (starts Vite + Tauri)
pnpm tauri dev

# Production build (daemon must be built first)
pnpm build:daemon:release
pnpm tauri build

# TypeScript check only
pnpm exec tsc --noEmit

# Rust check only (changed crate — NOT --workspace)
cd src-tauri && cargo check -p <crate-you-modified>

# Run Rust tests (all workspace members, requires cargo-nextest)
cd src-tauri && cargo nextest run --workspace

# Run Rust tests (fast profile — skip stress/perf tests)
cd src-tauri && cargo nextest run --workspace --profile fast

# Smart test runner (only affected crates based on git diff)
pnpm test:smart

# Run TypeScript unit tests
pnpm test

# Run browser tests (real Chromium via Playwright)
pnpm test:browser

# Run browser tests with visible browser window
pnpm test:browser:headed

# Run integration tests (daemon required — spawns isolated daemon per suite)
pnpm build:daemon && pnpm test:integration
```

## Git Workflow

Always commit all staged and unstaged changes when making a commit. Do not leave uncommitted changes behind.

Never add "Generated with Claude Code" or any similar attribution message to commits, PRs, or any other output.

### Changelog Fragments

Every `feat:` and `fix:` commit must include a changelog fragment file in `changelog/unreleased/`.

- **Naming**: `<PR-number>-<short-description>.md` (e.g., `425-fix-scroll-snap.md`). If no PR yet, use the branch name.
- **Format**: One or more [Keep a Changelog](https://keepachangelog.com/) sections (`### Added`, `### Fixed`, `### Changed`, `### Removed`, `### Tests`).
- **Content**: Bold title + dash + description + PR reference. See `changelog/TEMPLATE.md`.
- **When**: Create the fragment as part of the same commit that introduces the change.
- **Who collects**: `/bump-version` merges fragments into `CHANGELOG.md` and deletes them at release time.
- `chore:`, `docs:`, `style:`, `refactor:`, `test:` commits do NOT need fragments unless they represent user-facing changes.

### PR Policy

- **Features (`feat:`) and bug fixes (`fix:`)**: Create a feature branch, open a PR to master, and wait for merge.
- **Documentation (`docs:`), chores (`chore:`), style (`style:`), and other minor changes**: Commit and push directly to master — no PR needed.

## Debugging Principles

- **Never mask errors.** Don't add retry loops, fallback handlers, or auto-recovery that hides the root cause of a crash or failure. If something crashes, the priority is understanding WHY — not papering over it so the user doesn't notice.
- **Preserve crash evidence.** Logs must survive process restarts. Never truncate logs on startup. Use append mode and rotate old logs so the previous run's crash info is always available for post-mortem.

## Issue Investigation Tracking

Track all bugs and investigations as **GitHub Issues**, not local docs.

### When starting a bug investigation:
1. Search existing issues: `gh issue list --search "<keywords>" --state all --limit 10`
2. If a matching closed issue exists, read it (`gh issue view N`) — the bug may have regressed
3. Create a new issue or reopen the existing one with appropriate labels (`bug`, `performance`, `daemon`, `frontend`, `mcp`, `ux`)
4. Comment on the issue with each approach tried, including what failed and why

### During investigation:
- Add a comment for each significant attempt (what you tried, result, why it failed/succeeded)
- Include relevant code snippets, test commands, and root cause analysis in comments
- Use the issue body for the canonical summary (symptom, root cause, fix)

### When resolved:
- Reference the issue in the PR description with `fixes #N` (GitHub auto-closes on merge)
- Add a final comment with regression risk assessment and relevant test commands

### Reference docs
Architecture docs, design specs, and testing guides stay in `docs/` — only investigation/bug tracking uses GitHub Issues.

## Test Frameworks

Six test tiers, each targeting a different layer of the stack. When reproducing a bug, pick the tier that exercises the real failure point — not the one that's easiest to write.

### Quick Reference

| Tier | Naming | Command | Environment | Mocks | Best For |
|------|--------|---------|-------------|-------|----------|
| **Unit** | `*.test.ts` | `pnpm test` | Node/jsdom | Tauri APIs | Store logic, services, pure functions, keyboard routing |
| **Browser** | `*.browser.test.ts` | `pnpm test:browser` | Real Chromium | Tauri APIs | Canvas2D rendering, pixel correctness, real layout, pointer events |
| **Integration** | `*.integration.test.ts` | `pnpm test:integration` | Node + spawned daemon | Nothing | Daemon protocol, session lifecycle, Quick Claude flow, IPC correctness |
| **E2E** | `e2e/specs/*.e2e.ts` | `pnpm test:e2e` | Full Tauri app + WebdriverIO | Nothing | Full user workflows, persistence across restarts, input latency |
| **Daemon** | `daemon/tests/*.rs` | `cargo nextest run -p godly-daemon` | Isolated daemon process | Nothing | Concurrency, lock contention, memory leaks, pipe saturation, handler starvation |
| **Crate** | `#[test]` in `*.rs` | `cargo nextest run -p <crate>` | Rust unit | — | Parser correctness, serialization, data structures |

### Tier Details

#### 1. Unit Tests (`pnpm test`)
- **Location**: `src/**/*.test.ts`
- **Environment**: Vitest + jsdom (Node.js DOM simulator)
- **What's real**: JavaScript logic, state machines, event bus
- **What's mocked**: All Tauri APIs (invoke, listen, Store, dialogs)
- **Catches**: State management bugs, event routing errors, keyboard shortcut conflicts, service logic regressions, plugin system errors
- **Cannot catch**: Canvas rendering bugs, real DOM layout, real CSS flexbox, pointer events (jsdom returns zeros for `getBoundingClientRect`)
- **Examples**: `src/state/store.split-navigation.test.ts`, `src/services/workspace-service.test.ts`

#### 2. Browser Tests (`pnpm test:browser`)
- **Location**: `src/**/*.browser.test.ts`
- **Environment**: Vitest Browser Mode + real Chromium via Playwright
- **What's real**: DOM, CSS flexbox, Canvas2D, `measureText()`, `getImageData()`, pointer events
- **What's mocked**: Tauri APIs (via `src/test-utils/browser-setup.ts`)
- **Catches**: Canvas paint order bugs, font metric errors, pixel color correctness, flexbox layout regressions, split pane sizing bugs, divider positioning errors
- **Cannot catch**: Daemon interaction, session lifecycle, persistence
- **Use `pnpm test:browser:headed`** to see the Chromium window during tests
- **Examples**: `Canvas2DGridRenderer.browser.test.ts` (pixel inspection), `SplitContainer.browser.test.ts` (real layout)

#### 3. Integration Tests (`pnpm test:integration`)
- **Location**: `integration/tests/**/*.integration.test.ts`
- **Environment**: Node.js + real spawned daemon (isolated per suite via `DaemonFixture`)
- **What's real**: Daemon binary, named pipe IPC, PTY sessions, shell processes, binary frame protocol
- **What's mocked**: Nothing — exercises the real daemon
- **Catches**: Protocol correctness (binary frames, JSON messages), session create/attach/detach lifecycle, IPC pipe saturation, command execution + output parsing, Quick Claude flow (trust prompt, incremental echo)
- **Cannot catch**: Frontend rendering, Tauri app lifecycle, persistence across restarts
- **Key infrastructure**: `DaemonFixture` (spawns isolated daemon), `DaemonClient` (TypeScript wire protocol), `SessionHandle` (high-level session API)
- **Examples**: `smoke.integration.test.ts`, `quick-claude.integration.test.ts`

#### 4. E2E Tests (`pnpm test:e2e`)
- **Location**: `e2e/specs/**/*.e2e.ts`
- **Environment**: Full Tauri debug binary + WebdriverIO + tauri-driver + WebView2
- **What's real**: Everything — full app, daemon, renderer, persistence, IPC
- **What's mocked**: Nothing
- **Catches**: Session persistence across app restart, layout/scrollback/CWD persistence, keyboard shortcut routing (app vs terminal), tab drag-and-drop, input latency (key-to-grid, key-to-pixel), full user workflows end-to-end
- **Cannot catch**: Isolated component bugs (too high-level to pinpoint)
- **Gotchas**: Use `browser.execute()` for DOM queries (not `browser.$()`), use `invoke('write_to_terminal')` for input (not `browser.keys()`)
- **Examples**: `session-persistence.e2e.ts`, `input-latency.e2e.ts`, `keyboard-shortcuts.e2e.ts`

#### 5. Daemon Tests (`cargo nextest run -p godly-daemon`)
- **Location**: `src-tauri/daemon/tests/**/*.rs`
- **Environment**: Isolated daemon process per test (unique pipe, unique instance, non-detached)
- **What's real**: Daemon binary, PTY sessions, ring buffers, godly-vt parser, named pipe IPC
- **What's mocked**: Nothing
- **Catches**: Mutex deadlocks, handler thread starvation, memory leaks (RSS monitoring), input latency under load, resize during output, adaptive batching behavior, pause/resume state, Ctrl+C signal handling
- **Cannot catch**: Frontend rendering, Tauri app integration
- **CRITICAL isolation rules**: unique `GODLY_PIPE_NAME` + `GODLY_INSTANCE` + `GODLY_NO_DETACH=1` + kill by PID (never `taskkill /IM`). See `DaemonFixture` pattern in `handler_starvation.rs`.
- **Examples**: `handler_starvation.rs` (lock contention), `input_latency.rs` (I/O bottleneck), `memory_stress.rs` (RSS tracking)

#### 6. Crate Tests (`cargo nextest run -p <crate>`)
- **Location**: Inline `#[test]` blocks in crate source + `tests/` dirs
- **Environment**: Standard Rust unit tests
- **Catches**: VT parser state machine bugs, ANSI sequence handling, grid/cursor operations, binary frame serialization, image protocol (Kitty/iTerm2/Sixel) decoding
- **Key crates**: `godly-vt` (100+ tests), `godly-protocol` (message serialization)

### Bug → Test Tier Decision Tree

Use this to pick the right test framework when reproducing a bug:

| Bug symptom | Test tier | Why |
|-------------|-----------|-----|
| Rendering glitch, wrong colors, garbled text on screen | **Browser** | Needs real Canvas2D + pixel inspection |
| Layout broken, panes wrong size, divider misplaced | **Browser** | Needs real CSS flexbox + `getBoundingClientRect` |
| Keyboard shortcut doesn't work or conflicts | **Unit** | Shortcut routing is pure logic (keybinding-store) |
| Terminal output missing, wrong, or delayed | **Integration** | Needs real daemon + shell process |
| Session lost after app restart | **E2E** | Needs full app lifecycle with persistence |
| Daemon freezes, all terminals unresponsive | **Daemon** | Lock contention / handler starvation |
| High input latency, slow typing | **Daemon** or **E2E** | Daemon for I/O bottleneck, E2E for full pipeline measurement |
| Memory leak over time | **Daemon** | RSS monitoring with `GetProcessMemoryInfo` |
| Workspace/tab state bug | **Unit** or **E2E** | Unit for store logic, E2E for persistence |
| Quick Claude flow broken | **Integration** | DaemonFixture + SessionHandle exercises real CLI |
| Protocol parsing error | **Crate** | godly-protocol unit tests |
| VT escape sequence mishandled | **Crate** | godly-vt parser tests |
| Drag-and-drop, pointer interaction broken | **Browser** or **E2E** | Browser for component, E2E for full workflow |

### Project-Specific Workflow Notes

- **Bug fixes**: Write a full test **suite** (not a single test) to reproduce the bug. Pick the tier from the decision tree above.
- **Features**: Write **E2E tests** (`pnpm test:e2e`), not just unit tests. For Canvas2D/layout features, also write **browser tests** (`*.browser.test.ts`).
- **Performance issues**: Always write automated reproducible tests that demonstrate the problem under realistic conditions. Isolated component benchmarks are useful but insufficient — the test must exercise the real bottleneck (e.g., concurrent I/O, lock contention, IPC round-trips). See `daemon/tests/input_latency.rs` and `daemon/tests/handler_starvation.rs` for patterns.

## User-Like Testing (Post-Implementation)

After completing any feature or bug fix that has a visual/UX component, **ask the user** if they'd like you to run user-like testing via `/manual-testing <feature>`.

The testing framework combines:
- **godly-terminal MCP** — `execute_js` (DOM/store inspection), `capture_screenshot` (canvas PNG), split view control
- **pyautogui-mcp** — Real OS-level mouse/keyboard/screenshot for drag-and-drop, divider resize, keyboard shortcuts

See `.claude/skills/manual-testing.md` for the full testing procedure.

## Output Hygiene

- **Run targeted tests first**: `cargo nextest run -p godly-daemon` before the full suite.
- **Summarize failures**: State the root cause concisely, don't paste full stack traces.
- **Avoid verbose flags**: No `--verbose` or `--nocapture` unless actively debugging a specific test.
- **Incremental verification**: `cargo check` before `cargo nextest run`. One crate before all crates.

## Verification Requirements

**IMPORTANT**: CI runs full builds and tests on every PR. Locally, run only lightweight checks:


### What CI handles (so you don't have to):
- `cargo check --workspace` (cross-crate type checking)
- `cargo nextest run --workspace` (full test suite, 3 daemon partitions)
- `tsc --noEmit` (TypeScript strict check)
- `pnpm build` (production Vite build)
- Full release build of daemon/mcp/notify binaries

Do NOT run `cargo check --workspace`, `pnpm build`, or `cargo nextest run --workspace` locally unless debugging a CI failure. Let CI catch cross-crate breakage — local checks are for fast feedback only.

## Product Vision

Godly Terminal is built for **AI-assisted development workflows**. The primary use case is running multiple workspaces, each containing 2+ Claude Code instances (or other AI tools) working in parallel. A typical session has 10-20 concurrent terminal sessions, with only 1-2 visible at any time.

This means the critical performance axis is **not** single-terminal rendering speed — it's **multi-session efficiency**: low memory per session, fast workspace switching, intelligent resource allocation between visible and background terminals, and robust session persistence for long-running AI processes.

### Design Priorities (in order)
1. **Session persistence** — AI tool sessions are long-running and valuable; never lose them
2. **Multi-session scalability** — 20+ concurrent sessions without degradation
3. **Workspace switching speed** — instant context switch between groups of terminals
4. **Background efficiency** — minimize resources for terminals the user isn't looking at
5. **Visible terminal responsiveness** — low latency for the 1-2 terminals currently on screen

## Architecture Overview

Godly Terminal is a Windows terminal application built with Tauri 2.0, featuring workspaces and tmux-style session persistence via a background daemon.

### Stack
- **Frontend**: TypeScript + vanilla DOM + Canvas2D renderer (backed by godly-vt)
- **Backend**: Rust + Tauri 2.0 (GUI client) + godly-daemon (background PTY manager)
- **Terminal engine**: godly-vt (forked from vt100-rust, SIMD VT parser with scrollback)
- **Build**: Vite (frontend) + Cargo workspace (backend)

### Rendering Pipeline

The daemon owns all terminal state via godly-vt parsers. The frontend is a pure display layer:

```
Shell output → daemon PTY reader → ring buffer + godly-vt parser
                                          │
                              ┌───────────┘
                              ▼
Frontend: terminal-output event → fetch RichGridData snapshot via IPC
                                          │
                              ┌───────────┘
                              ▼
              TerminalRenderer.render(snapshot) → Canvas2D paint
```

Key design: **no terminal parsing happens in the frontend**. The daemon's godly-vt parser is the single source of truth for grid state, cursor position, colors, scrollback, etc.

### Daemon Architecture

```
┌─────────────┐     Named Pipe IPC      ┌─────────────────┐
│  Tauri App   │◄──────────────────────►│  godly-daemon    │
│  (GUI client)│  connect/disconnect     │  (background)    │
│              │  at will                │                  │
│  DaemonClient│                        │  PTY Sessions    │
│  Bridge      │                        │  Ring Buffers    │
└─────────────┘                         │  godly-vt Parsers│
     │                                  └─────────────────┘
     │ Tauri events                           │
     ▼                                        │ portable-pty
  Frontend                                    ▼
  (Canvas2D renderer)                    Shell processes
                                         (survive app close)
```

### Workspace Crate Structure

```
src-tauri/
  Cargo.toml              ← workspace root
  protocol/               ← shared message types (godly-protocol)
    src/lib.rs, messages.rs, frame.rs, types.rs
  daemon/                 ← background daemon binary (godly-daemon)
    src/main.rs, server.rs, session.rs, pid.rs
  godly-vt/               ← terminal state engine (forked from vt100-rust)
    src/lib.rs, grid.rs, screen.rs, parser.rs
  src/                    ← Tauri app
    daemon_client/        ← IPC client + event bridge
      mod.rs, client.rs, bridge.rs
    commands/             ← Tauri IPC command handlers
    state/                ← App state (workspaces, terminals, session metadata)
    persistence/          ← Layout, scrollback, autosave
    pty/                  ← Process monitor (queries daemon for PIDs)
```

### Frontend-Backend Communication

All terminal and workspace operations use Tauri IPC commands defined in `src-tauri/src/commands/`. Frontend services (`src/services/`) wrap `invoke()` calls. Terminal commands proxy through the daemon via named pipe IPC.

Key IPC commands:
- `create_terminal` / `close_terminal` - Creates/closes daemon session + attaches
- `write_to_terminal` / `resize_terminal` - Proxied to daemon session
- `get_grid_snapshot` - Fetch RichGridData from daemon's godly-vt parser
- `get_grid_dimensions` / `get_grid_text` - Query grid state
- `set_scrollback` - Set scrollback viewport offset
- `reconnect_sessions` / `attach_session` - Reconnect to live daemon sessions on restart
- `detach_all_sessions` - Detach on window close (sessions keep running)
- `create_workspace` / `delete_workspace` - Workspace management
- `save_layout` / `load_layout` - Persistence
- `save_scrollback` / `load_scrollback` - Terminal history

Backend emits events to frontend (via DaemonBridge):
- `terminal-output` - PTY output data (triggers grid snapshot fetch)
- `terminal-closed` - Process exit
- `process-changed` - Shell process name updates

### State Management

**Frontend** (`src/state/store.ts`): Observable store with `subscribe()` pattern. Components call store methods, store notifies all subscribers.

**Backend** (`src-tauri/src/state/`): Thread-safe state using `RwLock<HashMap>`. Holds workspaces, terminals, and session metadata (shell_type, cwd for persistence).

### Session Lifecycle

1. **Create**: App sends `CreateSession` + `Attach` to daemon via named pipe
2. **Running**: Daemon owns PTY + godly-vt parser, streams output events to attached client
3. **App close**: App sends `Detach` for all sessions, saves layout
4. **App reopen**: Loads layout, checks daemon for live sessions via `ListSessions`
5. **Reattach**: If session alive → `Attach` (ring buffer replays missed output into godly-vt)
6. **Fallback**: If session dead → create fresh terminal with saved CWD + load scrollback
7. **Idle**: Daemon self-terminates after 5min with no sessions and no clients

### Persistence

Three persistence mechanisms in `src-tauri/src/persistence/`:
- **layout.rs** - Workspace/terminal metadata saved on exit (reads from session_metadata)
- **scrollback.rs** - Terminal buffer content per-session (5MB limit)
- **autosave.rs** - Background thread saves every 30s if dirty

Data stored via `tauri-plugin-store` in app data directory.

### Component Structure

```
App.ts           - Root: manages layout, keyboard shortcuts, reconnection logic
├── WorkspaceSidebar.ts  - Workspace list, new workspace dialog, drop target
├── TabBar.ts            - Terminal tabs with drag-drop reordering
└── TerminalPane.ts      - Canvas2D terminal pane (delegates to TerminalRenderer)
    └── TerminalRenderer.ts - Canvas2D rendering of godly-vt grid snapshots
```

### Shell Types

`ShellType` enum supports:
- `Windows` - PowerShell with `-NoLogo`
- `Wsl { distribution }` - WSL with optional distro selection

## Daemon Test Isolation (CRITICAL)

**Tests must NEVER interfere with the production daemon.** A test that kills or connects to the production daemon will freeze all live terminal sessions.

### Required isolation rules for `daemon/tests/*.rs`:

1. **Use isolated pipe names** — every test must create its own unique pipe via `GODLY_PIPE_NAME` env var or `--instance` CLI arg. NEVER import or use the production `PIPE_NAME` constant from `godly_protocol`.
2. **Use `GODLY_INSTANCE`** — every test that sets `GODLY_PIPE_NAME` must also set `GODLY_INSTANCE` to isolate the shim metadata directory. Without it, the test daemon reads the production metadata dir and kills live shim processes. Use: `.env("GODLY_INSTANCE", pipe_name.trim_start_matches(r"\\.\pipe\"))`.
3. **Kill by PID, not by name** — NEVER use `taskkill /F /IM godly-daemon.exe` (kills ALL daemon processes). Use `child.kill()` for child-process daemons or `taskkill /F /PID <pid>` for detached daemons.
4. **Use `GODLY_NO_DETACH=1`** — keeps the test daemon as a child process so `child.kill()` works for cleanup.
5. **Pattern to follow** — see `handler_starvation.rs` or `memory_stress.rs` for the `DaemonFixture` pattern with proper isolation.

### Guardrail test

`daemon/tests/test_isolation_guardrail.rs` automatically scans all daemon test files for violations of these rules. It runs as part of the normal test suite and will fail if any test file:
- Uses `taskkill /IM` (process-name kill)
- Imports the production `PIPE_NAME` constant
- Spawns a daemon without `GODLY_PIPE_NAME` or `--instance` isolation
- Spawns a daemon without `GODLY_INSTANCE` (metadata directory isolation)

## Keyboard Shortcuts

All keyboard shortcuts defined in `DEFAULT_SHORTCUTS` (`src/state/keybinding-store.ts`) must be displayed in the Settings dialog (`src/components/SettingsDialog.ts`). When adding a new shortcut category, add it to the `categories` array in `renderShortcuts()` so it appears in the UI.

## Key Patterns

### Adding a new Tauri command

1. Add function in `src-tauri/src/commands/` with `#[tauri::command]`
2. Register in `lib.rs` `invoke_handler`
3. Add TypeScript wrapper in `src/services/`

### Adding a new daemon command

1. Add variant to `Request` and `Response` in `protocol/src/messages.rs`
2. Handle in `daemon/src/server.rs` `handle_request()`
3. Add client method in `src/daemon_client/client.rs`
4. Add Tauri command wrapper in `src/commands/terminal.rs`

### Crate Dependency Graph

```
godly-protocol (hub) → daemon, mcp, notify, whisper, remote
godly-vt (leaf) → daemon
Independent: godly-llm, godly-renderer, godly-pty-shim
```

### Parallel Agent Rules

- **Protocol changes**: Serialize — one agent at a time, merge before others rebase
- **lib.rs / App.ts**: After decomposition, each agent works in its domain module — conflicts rare
- **Independent crates** (vt, llm, renderer): Fully parallelizable
- **Different frontend controllers**: Fully parallelizable
- **Different store domains**: Fully parallelizable

### Conventions for New Code

- **New Tauri command**: Add to the relevant domain section in `lib.rs` invoke_handler, implement in the appropriate `commands/` submodule
- **New App.ts feature**: Create a controller in `src/controllers/`, import from `App.ts`
- **New store operation**: Add to the relevant domain module (`store-workspace.ts`, `store-terminal.ts`, or `store-layout.ts`), add delegation method in `store.ts`
- **New shared type (Rust↔TS)**: Add to protocol crate with `#[derive(ts_rs::TS)]`, run `pnpm generate-types`

### Modifying godly-mcp

When changing any code in `src-tauri/mcp/`, bump the `BUILD` constant in `src-tauri/mcp/src/main.rs` so the log shows which binary is running. The log line `=== godly-mcp starting === build=N` makes it easy to confirm a rebuilt binary is actually in use.

### Adding auto-save triggers

Inject `State<Arc<AutoSaveManager>>` and call `auto_save.mark_dirty()` after state mutations.

### Terminal state flow

User input → `terminalService.writeToTerminal()` → IPC → DaemonClient → named pipe → daemon → PTY
Shell output → daemon reader thread → ring buffer + godly-vt parser → named pipe → DaemonBridge → `terminal-output` event → `TerminalPane.fetchAndRenderSnapshot()` → Canvas2D paint

## Log File Locations

| Component | File | Location |
|-----------|------|----------|
| Daemon | `godly-daemon-debug.log` | `%APPDATA%/com.godly.terminal/` |
| Bridge | `godly-bridge-debug.log` | `%APPDATA%/com.godly.terminal/` |
| Whisper | `godly-whisper-debug.log` | `%APPDATA%/com.godly.terminal/` |
| MCP | `godly-mcp.log` | Next to `godly-mcp.exe` binary |
| Frontend | `frontend.log` | `%APPDATA%/com.godly.terminal/logs/` |

All rotate to `.prev.log` at 2MB. Append-mode, survive restarts.

## MCP Testing

See [docs/mcp-testing.md](docs/mcp-testing.md) for the full MCP test procedure and known gaps.

<!-- OMX:RUNTIME:START -->
<session_context>
**Session:** omx-1775154465837-j2nz5w | 2026-04-02T18:27:45.925Z

**Codebase Map:**
  scripts/: before-build, bump-version, demo-acts, demo-calibrate, dispatch-agent-prompts, install-whisper-locally, mcp-client, orchestrate-codex-lanes, orchestrate-quick-claude, phone-qa
  godly-test/: godly-test, assertions, cleanup, discovery, mcp-client, reporter, runner, step-executor, yaml-parser
  src-tauri/: CursorShape, CursorState, DaemonMessage, Event, GridData, GridDimensions, Request, Response, RichGridCell, RichGridData
  web/: vite.config

**Active Modes:**
- ralph: iteration 1/999, phase: starting
- skill-active: active

**Explore Command Preference:** enabled via `USE_OMX_EXPLORE_CMD` (default-on; opt out with `0`, `false`, `no`, or `off`)
- Advisory steering only: agents SHOULD treat `omx explore` as the default first stop for direct inspection and SHOULD reserve `omx sparkshell` for qualifying read-only shell-native tasks.
- For simple file/symbol lookups, use `omx explore` FIRST before attempting full code analysis.
- When the user asks for a simple read-only exploration task (file/symbol/pattern/relationship lookup), strongly prefer `omx explore` as the default surface.
- Explore examples: `omx explore...

**Ralph Ralplan-First Gate:** BLOCKED
- Requirement: complete planning artifacts before implementation/tool execution.
- Missing: `prd-*.md`, `test-spec-*.md`
- Path: `.omx/plans/`

**Compaction Protocol:**
Before context compaction, preserve critical state:
1. Write progress checkpoint via state_write MCP tool
2. Save key decisions to notepad via notepad_write_working
3. If context is >80% full, proactively checkpoint state
</session_context>
<!-- OMX:RUNTIME:END -->
