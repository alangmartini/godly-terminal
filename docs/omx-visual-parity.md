# OMX Visual Parity Workflow

This repo now has an additive OMX workflow for the existing godly-shell visual
parity harness. It does **not** replace the current parity scripts. It wraps
them with oh-my-codex-friendly operator surfaces so the same measurable parity
work can run under `omx exec`, `omx ralph`, or `omx team`.

## Files

- `omx-visual-parity-loop.ps1`
  - outer unattended loop
  - uses `omx exec`
  - keeps transcript/event artifacts under `.omx-visual-parity-loop/`
  - attaches `web-reference.png`, `current-godly-shell.png`, and the diff image
    when available
- `scripts/launch-godly-omx-parity-team.ps1`
  - thin wrapper around `omx team`
  - supplies the repo-specific parity contract and recommended lane split
- Existing parity harness:
  - `scripts/capture-web-reference.ps1`
  - `scripts/measure-godly-shell-parity.ps1`
  - `scripts/check-pixels.ps1`
  - `scripts/take-screenshot-now.ps1`

## When To Use Which Surface

### 1. Use `omx-visual-parity-loop.ps1` for unattended iteration

This is the closest analogue to `codex-ralph-loop.ps1`.

Use it when you want:
- repeated non-interactive iterations
- a `STOP` file
- per-iteration prompts, transcripts, events, and final-message artifacts
- OMX guidance without turning the whole flow into a manual interactive session

Example:

```powershell
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1
```

Useful variants:

```powershell
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -MaxIterations 5
powershell -ExecutionPolicy Bypass -File .\omx-visual-parity-loop.ps1 -Ephemeral
```

## 2. Use `omx team` for durable parallel lanes

Use team mode when the current highest-leverage parity work splits cleanly into
independent lanes:
- transcript typography/compositing
- retained sidebar/session layout
- measurement, screenshot refresh, and parity-ledger updates

The wrapper script just standardizes that task:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\launch-godly-omx-parity-team.ps1
```

Preview the task without launching workers:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\launch-godly-omx-parity-team.ps1 -PrintOnly
```

Notes:
- `omx team` expects a tmux-compatible shell context.
- On Windows that usually means `psmux` or a WSL/tmux flow.

## 3. Use `omx ralph` for supervised single-owner sessions

If you want an interactive persistent session instead of an outer PowerShell
loop, use `omx ralph` directly:

```bash
omx ralph "Drive godly-shell to web visual parity. Use scripts/measure-godly-shell-parity.ps1 after each meaningful change. Do not claim done while docs/references/gaps.md still lists critical or major gaps."
```

Do **not** nest `omx ralph` inside `omx-visual-parity-loop.ps1`. The script is
already the persistence layer.

## Repo-Specific Contract

The OMX parity surfaces in this repo all assume the same contract:

1. Measure first with `scripts/measure-godly-shell-parity.ps1`.
2. Use `scripts/check-pixels.ps1` as the numeric truth.
3. Use `$visual-verdict` as the structured qualitative guide when screenshots are fresh.
4. Update `docs/references/gaps.md` honestly.
5. Update `tasks/rendering-quality-iterations.md` with concrete measurements and
   the next highest-leverage blocker.
6. Do not use `$web-clone` for this task. The target is a repo-local reference
   capture and JSX/layout intent, not a live external URL.

## Current Highest-Leverage OMX Lanes

As of the current gap log, the most useful autonomous split is:

1. Transcript typography/compositing
   - `src-tauri/native/godly-shell/src/ui/reference_pane.rs`
   - `src-tauri/native/godly-shell/src/terminal_renderer.rs`
2. Sidebar/session retained layout
   - `src-tauri/native/godly-shell/src/ui/sidebar.rs`
   - supporting retained layout code
3. Measurement and verification
   - `scripts/measure-godly-shell-parity.ps1`
   - `scripts/capture-web-reference.ps1`
   - `docs/references/gaps.md`
   - `tasks/rendering-quality-iterations.md`

That split keeps one lane measuring and one lane writing docs/evidence instead
of letting every worker freestyle on the same rendering files.
