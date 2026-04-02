# OMX Visual Parity Workflow

This repo already has the real parity harness:

- `scripts/capture-web-reference.ps1`
- `scripts/take-screenshot-now.ps1`
- `scripts/check-pixels.ps1`
- `scripts/measure-godly-shell-parity.ps1`

OMX does not replace that harness. It sits on top of it and helps keep the loop moving when the remaining work is iterative, multi-lane, or long-running.

Use OMX for orchestration. Use the harness for evidence.

## When To Use OMX

Use the OMX workflow when visual parity work needs one or more of these:

- unattended iteration over a stubborn screenshot diff
- a prompt contract that keeps the agent anchored on the real target
- parallel lanes for typography, layout, and measurement work
- persistent notes and artifacts between runs

Do not use OMX as a substitute for the screenshot harness. If a change is not backed by `scripts/measure-godly-shell-parity.ps1`, it is not a parity result.

## Prerequisites

Before using this workflow:

1. Install oh-my-codex and confirm `omx` is on `PATH`.
2. Run `omx setup` in this machine once so the prompts, skills, and config are installed.
3. Make sure the Windows team backend exists if you plan to use parallel lanes. `omx team` expects a tmux-compatible backend such as `psmux` on Windows or `tmux` in WSL.
4. Expect a dirty worktree in this repo. The parity work already has unrelated local changes, so do not use a destructive cleanup command to "reset" the repo before running the loop.

## Shared Prompt Contract

The shared task contract is `docs/omx-visual-parity-task.md`.

That file is the anchor for both operator paths:

- the unattended loop reads it before each iteration
- the team launcher uses it as the base instructions for every lane

It exists so the loop and team workers do not drift into different ideas of what parity means.

## Main Paths

### 1. Unattended Loop

Use `omx-visual-parity-loop.ps1` when you want a single long-running supervisor to keep feeding OMX the next parity iteration.

Typical command:

```powershell
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1
```

Useful options:

```powershell
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -MaxIterations 1
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -EnableSearch
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -ShowRawJsonEvents
```

This loop should:

- write a run manifest and per-iteration prompt/transcript/event files
- reuse the screenshot harness commands in the generated prompt
- stop only when the prompt contract says `RALPH_DONE`, or when the operator creates the `STOP` file in the artifacts directory

### 2. Parallel Lanes

Use `scripts/start-omx-parity-team.ps1` when the work is naturally split across independent lanes.

Typical command:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-omx-parity-team.ps1
```

Suggested lane split:

- transcript typography and compositing
- sidebar or retained-layout migration
- parity measurement and gap-log updates

If you need a preview without launching a real team, run the script with `-WhatIf` or its equivalent no-op mode.

## Recommended Operator Flow

1. Refresh the web reference if needed:
   `powershell -ExecutionPolicy Bypass -File .\scripts\capture-web-reference.ps1`
2. Measure the current state:
   `powershell -ExecutionPolicy Bypass -File .\scripts\measure-godly-shell-parity.ps1`
3. Start the unattended OMX loop or the parallel team lanes.
4. After every meaningful edit, re-run the parity harness and compare the new screenshot to the web reference.
5. Keep `docs/references/gaps.md` and `tasks/rendering-quality-iterations.md` honest about what is still unresolved.

## Example Commands

Unattended loop:

```powershell
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -MaxIterations 0
```

Single-iteration smoke test:

```powershell
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -MaxIterations 1
```

Parallel lanes:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-omx-parity-team.ps1
```

Harness-only refresh:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\measure-godly-shell-parity.ps1
```

## Loop Artifacts

The unattended loop should leave behind:

- a run manifest
- the latest generated prompt
- the latest transcript
- the latest JSONL events
- the last agent message
- the screenshot artifacts used for comparison

These artifacts are the audit trail for why the next edit happened.

## Stop And Exit Semantics

The loop should stop in one of three ways:

1. The agent emits `RALPH_DONE`.
2. The operator creates the `STOP` file in the loop artifacts directory.
3. The loop hits an explicit `-MaxIterations` limit.

If the loop exits because of a retryable error, the next iteration should continue from the latest saved artifacts instead of silently discarding them.

## Practical Rule

If the screenshot diff got better but the harness did not run, the work is incomplete.

If the harness ran but the visual verdict did not justify the next edit, the work is also incomplete.

Use OMX to keep the iteration moving. Use the parity harness to decide whether the move was real.
