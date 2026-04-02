<#
.SYNOPSIS
  Ralph Loop - autonomous native visual-parity loop for godly-shell.

.DESCRIPTION
  Repeatedly invokes the configured coding agent with a repo-specific prompt that
  prioritizes the highest-leverage parity work until the agent emits RALPH_DONE
  or the loop is stopped.

  The loop defaults to unlimited iterations. Create the STOP file shown in the
  banner to stop gracefully after the current iteration.

.PARAMETER MaxIterations
  Maximum number of iterations. Use 0 for unlimited. Default: 0.
#>

param(
    [int]$MaxIterations = 0,
    [int]$IterationDelaySeconds = 5,
    [int]$RetryDelaySeconds = 10,
    [string]$AgentCommand = "claude",
    [string]$ProjectRoot = $PSScriptRoot,
    [string]$ArtifactsRoot = (Join-Path $PSScriptRoot ".ralph-loop"),
    [string]$WebRefImage = (Join-Path $PSScriptRoot "docs\references\web-reference.png"),
    [string]$NativeImage = (Join-Path $PSScriptRoot "docs\references\current-godly-shell.png"),
    [switch]$NoCleanup
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($PSVersionTable.PSVersion.Major -ge 7) {
    $PSNativeCommandUseErrorActionPreference = $true
}

$ProjectRoot = [System.IO.Path]::GetFullPath($ProjectRoot)
$ArtifactsRoot = [System.IO.Path]::GetFullPath($ArtifactsRoot)
$WebRefImage = [System.IO.Path]::GetFullPath($WebRefImage)
$NativeImage = [System.IO.Path]::GetFullPath($NativeImage)

$RunId = Get-Date -Format "yyyyMMdd-HHmmss"
$RunDir = Join-Path $ArtifactsRoot $RunId
$StopFile = Join-Path $ArtifactsRoot "STOP"
$LatestPromptPath = Join-Path $ArtifactsRoot "latest-prompt.md"
$LatestOutputPath = Join-Path $ArtifactsRoot "latest-output.txt"
$RunManifestPath = Join-Path $RunDir "run-manifest.json"

New-Item -ItemType Directory -Force -Path $ArtifactsRoot | Out-Null
New-Item -ItemType Directory -Force -Path $RunDir | Out-Null

function Test-CommandExists {
    param([Parameter(Mandatory = $true)][string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Write-RunManifest {
    $manifest = [ordered]@{
        run_id = $RunId
        started_at = (Get-Date).ToString("o")
        project_root = $ProjectRoot
        artifacts_root = $ArtifactsRoot
        run_dir = $RunDir
        stop_file = $StopFile
        agent_command = $AgentCommand
        web_reference_image = $WebRefImage
        native_image = $NativeImage
        max_iterations = $MaxIterations
        iteration_delay_seconds = $IterationDelaySeconds
        retry_delay_seconds = $RetryDelaySeconds
        no_cleanup = [bool]$NoCleanup
    }

    $manifest | ConvertTo-Json -Depth 4 | Set-Content -Path $RunManifestPath -Encoding UTF8
}

function Write-PromptFile {
    param(
        [Parameter(Mandatory = $true)][int]$Iteration,
        [Parameter(Mandatory = $true)][string]$PromptPath,
        [string]$PreviousOutputPath
    )

    $previousOutputNote = if ($PreviousOutputPath -and (Test-Path $PreviousOutputPath)) {
        "Previous iteration transcript: $PreviousOutputPath"
    }
    else {
        "No previous iteration transcript exists yet."
    }

    $content = @"
Ralph Loop iteration $Iteration.

Repository root: $ProjectRoot
Run artifacts directory: $RunDir
Web reference screenshot target: $WebRefImage
Native screenshot target: $NativeImage
$previousOutputNote

You are in an autonomous long-horizon parity loop for godly-shell.

The target is not "pretty enough". The target is real parity with the visual quality of web/godly-terminal.jsx and the screenshot in docs/references/web-reference.png.

The loop is effectively unlimited. Optimize for the final native result, not for making tiny isolated edits per iteration.

Read first:
- AGENTS.md
- docs/references/gaps.md
- tasks/rendering-quality-iterations.md
- docs/superpowers/plans/2026-03-29-drop-iced-migration.md
- docs/superpowers/plans/2026-03-27-directwrite-cleartype-rendering.md
- src-tauri/native/godly-shell/src/main.rs
- src-tauri/native/godly-shell/src/ui/layout.rs
- src-tauri/native/godly-shell/src/ui/builder.rs
- src-tauri/native/godly-shell/src/terminal_renderer.rs

Non-negotiable parity gates:
1. Windows presentation must be physical-pixel sharp, or you must implement and verify an objectively equivalent path.
2. UI chrome text must use a real typography/layout path. Do not treat synthetic italic, hand-tuned advance hacks, or pseudo-layout as the finished state.
3. Chrome text compositing must be background-aware so labels can achieve terminal-grade sharpness where technically valid.
4. Shell chrome layout must move toward a retained flex/layout layer (taffy or equivalent) instead of accumulating more manual rectangle math.
5. The repo must have a screenshot-diff or measurable visual-parity harness. Do not rely purely on vibes.

You must not emit RALPH_DONE while any of the above remain materially unresolved.

Current strategy priority:
1. Close the highest-leverage architectural blocker first.
2. Then close measurable visual gaps.
3. Only after those are solid should you spend full iterations on tiny pixel nudges.

Important guidance:
- Do not artificially limit yourself to one tiny visual tweak if a deeper blocker spans multiple files.
- If a needed helper script or harness is missing, build it as part of the iteration.
- If docs/references/gaps.md understates architectural gaps, correct it.
- Use parallel sub-agents when that shortens the critical path, but do not delegate immediate blockers blindly.

Preferred repo-native tooling:
- Native screenshot helper: scripts/take-screenshot-now.ps1
- Pixel inspection helper: scripts/check-pixels.ps1
- Iteration log: tasks/rendering-quality-iterations.md
- Gap tracker: docs/references/gaps.md

Iteration workflow:
1. Audit current state and choose the highest-leverage next task.
2. Build or improve the missing automation/harness you need if quality work is under-instrumented.
3. Refresh the web reference screenshot if needed:
   - Use the Vite app in web/
   - Prefer pnpm over npm
   - Use Chrome DevTools / browser tooling to capture the real reference at 1920x1080
4. Build and run godly-shell, capture the native screenshot, and compare against the web reference.
5. Implement the code changes.
6. Run lightweight verification only. Prefer targeted cargo check / targeted tests / script verification.
7. Rebuild and re-capture screenshots after the fix.
8. Update docs/references/gaps.md and tasks/rendering-quality-iterations.md with precise technical findings and remaining gaps.
9. Commit only if the result is meaningfully better and working. Follow AGENTS.md exactly:
   - commit all staged and unstaged changes in that commit
   - never leave partially related local changes behind
   - if the commit is feat: or fix:, include a changelog fragment in changelog/unreleased/
10. Leave the repo buildable before ending the iteration.

Hard rules:
- Do not settle for close enough.
- Do not spend the whole iteration on tiny color nudges while a higher-leverage parity blocker remains open.
- Do not revert unrelated user changes.
- Do not hide uncertainty in docs. Be explicit about what is still wrong.
- Do not claim parity just because the current screenshot looks better than the previous one.

Done criteria:
- The native shell is visually at parity with the web reference across text sharpness, layout discipline, spacing, chrome hierarchy, and presentation quality.
- docs/references/gaps.md no longer lists any material parity gap.
- The parity harness/checking path is good enough that future regressions are detectable.

At the end of your response:
- Output RALPH_DONE only if the done criteria are genuinely met.
- Otherwise end with a line starting exactly with: RALPH_CONTINUE:
  The value after the colon must be the next highest-leverage task.
"@

    Set-Content -Path $PromptPath -Value $content -Encoding UTF8
    Copy-Item -Path $PromptPath -Destination $LatestPromptPath -Force
}

function Invoke-AgentIteration {
    param([Parameter(Mandatory = $true)][string]$PromptPath)

    $promptText = Get-Content -Path $PromptPath -Raw
    $env:CLAUDE_CODE_ENTRYPOINT = "cli"
    return & $AgentCommand --dangerously-skip-permissions --print --output-format text -p $promptText 2>&1
}

function Stop-ViteProcesses {
    $viteProcs = Get-CimInstance Win32_Process -Filter "Name = 'node.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -match "vite" -and $_.CommandLine -match "5199" }

    foreach ($proc in $viteProcs) {
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
    }
}

if (-not (Test-CommandExists -Name $AgentCommand)) {
    throw "Agent command '$AgentCommand' was not found on PATH."
}

Write-RunManifest

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Ralph Loop - Native Visual Parity" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Goal:        finish native parity work, not just screenshot polish" -ForegroundColor Yellow
Write-Host "Agent:       $AgentCommand" -ForegroundColor Green
Write-Host "Project:     $ProjectRoot" -ForegroundColor Green
Write-Host "Artifacts:   $RunDir" -ForegroundColor Green
Write-Host "Stop file:   $StopFile" -ForegroundColor Green
Write-Host ""
Write-Host "  Web ref:   $WebRefImage"
Write-Host "  Native:    $NativeImage"
Write-Host "Iterations:  $(if ($MaxIterations -eq 0) { 'unlimited' } else { $MaxIterations })"
Write-Host ""

$iteration = 0
$previousOutputPath = $null

while ($true) {
    if (Test-Path $StopFile) {
        Write-Host ""
        Write-Host "[Ralph Loop] Stop file detected. Exiting before next iteration." -ForegroundColor Yellow
        break
    }

    $iteration++
    if ($MaxIterations -gt 0 -and $iteration -gt $MaxIterations) {
        Write-Host ""
        Write-Host "[Ralph Loop] Reached max iterations ($MaxIterations). Stopping." -ForegroundColor Yellow
        break
    }

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $iterationTag = "{0:D4}" -f $iteration
    $promptPath = Join-Path $RunDir "iteration-$iterationTag-prompt.md"
    $outputPath = Join-Path $RunDir "iteration-$iterationTag-output.txt"

    Write-Host ""
    Write-Host "[$timestamp] === Iteration $iteration ===" -ForegroundColor Green
    Write-Host "[Ralph Loop] Prompt: $promptPath" -ForegroundColor DarkGray
    Write-Host "[Ralph Loop] Output: $outputPath" -ForegroundColor DarkGray

    Write-PromptFile -Iteration $iteration -PromptPath $promptPath -PreviousOutputPath $previousOutputPath

    try {
        $output = Invoke-AgentIteration -PromptPath $promptPath
        $outputStr = ($output | Out-String).TrimEnd()

        $outputStr | Set-Content -Path $outputPath -Encoding UTF8
        $outputStr | Set-Content -Path $LatestOutputPath -Encoding UTF8

        if ($outputStr) {
            $outputStr | Write-Host
        }

        if ($outputStr -match "(?m)^RALPH_DONE\b") {
            Write-Host ""
            Write-Host "[Ralph Loop] Parity achieved. Stopping." -ForegroundColor Green
            break
        }

        $previousOutputPath = $outputPath
    }
    catch {
        $errorText = $_ | Out-String
        $errorText | Set-Content -Path $outputPath -Encoding UTF8
        $errorText | Set-Content -Path $LatestOutputPath -Encoding UTF8

        Write-Host "[Ralph Loop] Error in iteration ${iteration}: $_" -ForegroundColor Red
        Write-Host "[Ralph Loop] Retrying in $RetryDelaySeconds seconds..." -ForegroundColor Yellow
        Start-Sleep -Seconds $RetryDelaySeconds
        continue
    }

    if (Test-Path $StopFile) {
        Write-Host ""
        Write-Host "[Ralph Loop] Stop file detected. Exiting after current iteration." -ForegroundColor Yellow
        break
    }

    Write-Host ""
    Write-Host "[Ralph Loop] Iteration $iteration complete. Starting next in $IterationDelaySeconds seconds..." -ForegroundColor Cyan
    Start-Sleep -Seconds $IterationDelaySeconds
}

if (-not $NoCleanup) {
    Write-Host "[Ralph Loop] Cleaning up..." -ForegroundColor DarkGray
    Stop-ViteProcesses
}

Write-Host "[Ralph Loop] Finished after $iteration iterations." -ForegroundColor Green
Write-Host "[Ralph Loop] Run artifacts saved to $RunDir" -ForegroundColor Green
