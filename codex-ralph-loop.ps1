<#
.SYNOPSIS
  Codex Ralph Loop - autonomous native visual-parity loop for godly-shell.

.DESCRIPTION
  Repeatedly invokes Codex non-interactively with a repo-specific parity prompt
  until the agent emits RALPH_DONE or the loop is stopped.

  The loop defaults to unlimited iterations. Create the STOP file shown in the
  banner to stop gracefully after the current iteration.

.PARAMETER MaxIterations
  Maximum number of iterations. Use 0 for unlimited. Default: 0.
#>

param(
    [int]$MaxIterations = 0,
    [int]$IterationDelaySeconds = 5,
    [int]$RetryDelaySeconds = 10,
    [string]$AgentCommand = "codex",
    [string]$ProjectRoot = $PSScriptRoot,
    [string]$ArtifactsRoot = (Join-Path $PSScriptRoot ".codex-ralph-loop"),
    [string]$WebRefImage = (Join-Path $PSScriptRoot "docs\references\web-reference.png"),
    [string]$NativeImage = (Join-Path $PSScriptRoot "docs\references\current-godly-shell.png"),
    [string]$Model = "gpt-5.4",
    [string]$ReasoningEffort = "xhigh",
    [string]$Profile = "",
    [switch]$EnableSearch,
    [switch]$Ephemeral,
    [switch]$NoCleanup,
    [switch]$ShowRawJsonEvents
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
$LatestTranscriptPath = Join-Path $ArtifactsRoot "latest-transcript.txt"
$LatestLastMessagePath = Join-Path $ArtifactsRoot "latest-last-message.txt"
$LatestEventsPath = Join-Path $ArtifactsRoot "latest-events.jsonl"
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
        model = $Model
        reasoning_effort = $ReasoningEffort
        profile = $Profile
        enable_search = [bool]$EnableSearch
        ephemeral = [bool]$Ephemeral
        show_raw_json_events = [bool]$ShowRawJsonEvents
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
        [string]$PreviousLastMessagePath
    )

    $previousOutputNote = if ($PreviousLastMessagePath -and (Test-Path $PreviousLastMessagePath)) {
        "Previous iteration final message: $PreviousLastMessagePath"
    }
    else {
        "No previous iteration final message exists yet."
    }

    $content = @"
Codex Ralph Loop iteration $Iteration.

Repository root: $ProjectRoot
Run artifacts directory: $RunDir
Web reference screenshot target: $WebRefImage
Native screenshot target: $NativeImage
$previousOutputNote

You are Codex running in an autonomous long-horizon parity loop for godly-shell.

The target is not "good enough". The target is real parity with the visual quality of web/godly-terminal.jsx and the screenshot in docs/references/web-reference.png.

The loop is effectively unlimited. Optimize for the final native result, not for tiny isolated edits that leave the real blockers untouched.

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
2. UI chrome text must use a real typography/layout path. Do not treat synthetic italic, hand-tuned advance hacks, or pseudo-layout as finished.
3. Chrome text compositing must be background-aware so labels can achieve terminal-grade sharpness where technically valid.
4. Shell chrome layout must move toward a retained flex/layout layer (taffy or equivalent) instead of accumulating more manual rectangle math.
5. The repo must have a screenshot-diff or measurable visual-parity harness. Do not rely purely on vibes.

You must not emit RALPH_DONE while any of the above remain materially unresolved.

Current strategy priority:
1. Close the highest-leverage architectural blocker first.
2. Then close measurable visual gaps.
3. Only after those are solid should you spend iterations on micro-polish.

Important guidance:
- Do not artificially limit yourself to one tiny visual tweak if a deeper blocker spans multiple files.
- If a needed helper script or harness is missing, build it as part of the iteration.
- If docs/references/gaps.md understates architectural gaps, correct it.
- Use parallel agents if your environment supports them and it shortens the critical path, but do not delegate immediate blockers blindly.

Preferred repo-native tooling:
- Native screenshot helper: scripts/take-screenshot-now.ps1
- Pixel inspection helper: scripts/check-pixels.ps1
- Iteration log: tasks/rendering-quality-iterations.md
- Gap tracker: docs/references/gaps.md

Iteration workflow:
1. Audit current state and choose the highest-leverage next task.
2. Build or improve the missing automation or harness you need if quality work is under-instrumented.
3. Refresh the web reference screenshot if needed:
   - Use the Vite app in web/
   - Prefer pnpm over npm
   - Use browser tooling to capture the real reference at 1920x1080
4. Build and run godly-shell, capture the native screenshot, and compare against the web reference.
5. Implement the code changes.
6. Run lightweight verification only. Prefer targeted cargo check, targeted tests, or script verification.
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
- The parity harness or checking path is good enough that future regressions are detectable.

At the end of your response:
- Output RALPH_DONE only if the done criteria are genuinely met.
- Otherwise end with a line starting exactly with: RALPH_CONTINUE:
  The value after the colon must be the next highest-leverage task.
"@

    Set-Content -Path $PromptPath -Value $content -Encoding UTF8
    Copy-Item -Path $PromptPath -Destination $LatestPromptPath -Force
}

function Invoke-AgentIteration {
    param(
        [Parameter(Mandatory = $true)][string]$PromptPath,
        [Parameter(Mandatory = $true)][string]$TranscriptPath,
        [Parameter(Mandatory = $true)][string]$EventsPath,
        [Parameter(Mandatory = $true)][string]$LastMessagePath
    )

    $promptText = Get-Content -Path $PromptPath -Raw

    $topLevelArgs = @()
    if ($EnableSearch) {
        $topLevelArgs += "--search"
    }

    $args = @(
        "exec",
        "--dangerously-bypass-approvals-and-sandbox",
        "--json",
        "--color", "never",
        "-C", $ProjectRoot,
        "-o", $LastMessagePath,
        "-c", "model_reasoning_effort=`"$ReasoningEffort`""
    )

    if ($Profile) {
        $args += @("-p", $Profile)
    }
    if ($Model) {
        $args += @("-m", $Model)
    }
    if ($Ephemeral) {
        $args += "--ephemeral"
    }

    $args += "-"

    Set-Content -Path $TranscriptPath -Value $null -Encoding UTF8
    Set-Content -Path $LatestTranscriptPath -Value $null -Encoding UTF8
    Set-Content -Path $EventsPath -Value $null -Encoding UTF8
    Set-Content -Path $LatestEventsPath -Value $null -Encoding UTF8

    $state = @{
        last_message = ""
    }

    function Append-SharedLine {
        param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][string]$Line)
        Add-Content -Path $Path -Value $Line -Encoding UTF8
    }

    function Write-TranscriptLine {
        param([Parameter(Mandatory = $true)][string]$Line)
        Write-Host $Line
        Append-SharedLine -Path $TranscriptPath -Line $Line
        Append-SharedLine -Path $LatestTranscriptPath -Line $Line
    }

    function Write-TranscriptBlock {
        param(
            [Parameter(Mandatory = $true)][string]$Header,
            [AllowEmptyString()][string]$Text
        )

        Write-TranscriptLine -Line $Header
        if ([string]::IsNullOrWhiteSpace($Text)) {
            return
        }

        foreach ($line in ($Text -split "\r?\n")) {
            Write-TranscriptLine -Line ("  {0}" -f $line)
        }
    }

    function Handle-JsonItem {
        param(
            [Parameter(Mandatory = $true)]$Item,
            [Parameter(Mandatory = $true)][string]$EventType
        )

        $itemType = [string]$Item.type

        switch ($itemType) {
            "agent_message" {
                $text = [string]$Item.text
                if ([string]::IsNullOrWhiteSpace($text)) {
                    return
                }

                if ($EventType -eq "item.completed") {
                    $state.last_message = $text
                }

                Write-TranscriptBlock -Header "[agent]" -Text $text
            }
            "command_execution" {
                if ($EventType -eq "item.started") {
                    Write-TranscriptLine -Line ("[command:start] {0}" -f $Item.command)
                    return
                }

                $exitCode = if ($null -eq $Item.exit_code) { "?" } else { [string]$Item.exit_code }
                Write-TranscriptLine -Line ("[command:done exit={0}] {1}" -f $exitCode, $Item.command)

                $output = [string]$Item.aggregated_output
                if (-not [string]::IsNullOrWhiteSpace($output)) {
                    Write-TranscriptBlock -Header "[command output]" -Text $output.TrimEnd("`r", "`n")
                }
            }
            default {
                Write-TranscriptLine -Line ("[{0}] {1}" -f $EventType, $itemType)
            }
        }
    }

    function Handle-JsonEventLine {
        param([Parameter(Mandatory = $true)][string]$Line)

        Append-SharedLine -Path $EventsPath -Line $Line
        Append-SharedLine -Path $LatestEventsPath -Line $Line

        if ($ShowRawJsonEvents) {
            Write-Host $Line
        }

        try {
            $event = $Line | ConvertFrom-Json
        }
        catch {
            Write-TranscriptLine -Line ("[raw] {0}" -f $Line)
            return
        }

        switch ([string]$event.type) {
            "thread.started" {
                Write-TranscriptLine -Line ("[thread] {0}" -f $event.thread_id)
            }
            "turn.started" {
                Write-TranscriptLine -Line "[turn] started"
            }
            "item.started" {
                Handle-JsonItem -Item $event.item -EventType "item.started"
            }
            "item.completed" {
                Handle-JsonItem -Item $event.item -EventType "item.completed"
            }
            "turn.completed" {
                if ($null -ne $event.usage) {
                    Write-TranscriptLine -Line ("[usage] input={0} cached={1} output={2}" -f $event.usage.input_tokens, $event.usage.cached_input_tokens, $event.usage.output_tokens)
                }
                else {
                    Write-TranscriptLine -Line "[turn] completed"
                }
            }
            default {
                Write-TranscriptLine -Line ("[event] {0}" -f $event.type)
            }
        }
    }

    $promptText | & $AgentCommand @topLevelArgs @args 2>&1 | ForEach-Object {
        $line = [string]$_
        if (-not [string]::IsNullOrWhiteSpace($line)) {
            Handle-JsonEventLine -Line $line
        }
    }

    if (-not (Test-Path $LastMessagePath) -and -not [string]::IsNullOrWhiteSpace($state.last_message)) {
        Set-Content -Path $LastMessagePath -Value $state.last_message -Encoding UTF8
    }

    return [pscustomobject]@{
        last_message = $state.last_message
    }
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
Write-Host "  Codex Ralph Loop - Visual Parity" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Goal:        finish native parity work, not just screenshot polish" -ForegroundColor Yellow
Write-Host "Agent:       $AgentCommand" -ForegroundColor Green
Write-Host "Project:     $ProjectRoot" -ForegroundColor Green
Write-Host "Artifacts:   $RunDir" -ForegroundColor Green
Write-Host "Stop file:   $StopFile" -ForegroundColor Green
Write-Host "Streaming:   visible agent/tool events in real time" -ForegroundColor Green
if ($Model) {
    Write-Host "Model:       $Model" -ForegroundColor Green
}
if ($ReasoningEffort) {
    Write-Host "Reasoning:   $ReasoningEffort" -ForegroundColor Green
}
if ($Profile) {
    Write-Host "Profile:     $Profile" -ForegroundColor Green
}
Write-Host ""
Write-Host "  Web ref:   $WebRefImage"
Write-Host "  Native:    $NativeImage"
Write-Host "Iterations:  $(if ($MaxIterations -eq 0) { 'unlimited' } else { $MaxIterations })"
Write-Host ""

$iteration = 0
$previousLastMessagePath = $null

while ($true) {
    if (Test-Path $StopFile) {
        Write-Host ""
        Write-Host "[Codex Ralph Loop] Stop file detected. Exiting before next iteration." -ForegroundColor Yellow
        break
    }

    $iteration++
    if ($MaxIterations -gt 0 -and $iteration -gt $MaxIterations) {
        Write-Host ""
        Write-Host "[Codex Ralph Loop] Reached max iterations ($MaxIterations). Stopping." -ForegroundColor Yellow
        break
    }

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $iterationTag = "{0:D4}" -f $iteration
    $promptPath = Join-Path $RunDir "iteration-$iterationTag-prompt.md"
    $transcriptPath = Join-Path $RunDir "iteration-$iterationTag-transcript.txt"
    $eventsPath = Join-Path $RunDir "iteration-$iterationTag-events.jsonl"
    $lastMessagePath = Join-Path $RunDir "iteration-$iterationTag-last-message.txt"

    Write-Host ""
    Write-Host "[$timestamp] === Iteration $iteration ===" -ForegroundColor Green
    Write-Host "[Codex Ralph Loop] Prompt:      $promptPath" -ForegroundColor DarkGray
    Write-Host "[Codex Ralph Loop] Transcript: $transcriptPath" -ForegroundColor DarkGray
    Write-Host "[Codex Ralph Loop] Events:     $eventsPath" -ForegroundColor DarkGray
    Write-Host "[Codex Ralph Loop] Final msg:  $lastMessagePath" -ForegroundColor DarkGray

    Write-PromptFile -Iteration $iteration -PromptPath $promptPath -PreviousLastMessagePath $previousLastMessagePath

    try {
        $result = Invoke-AgentIteration -PromptPath $promptPath -TranscriptPath $transcriptPath -EventsPath $eventsPath -LastMessagePath $lastMessagePath
        $lastMessage = ""
        if (Test-Path $lastMessagePath) {
            $lastMessage = (Get-Content -Path $lastMessagePath -Raw).TrimEnd()
            $lastMessage | Set-Content -Path $LatestLastMessagePath -Encoding UTF8
        }
        elseif (-not [string]::IsNullOrWhiteSpace($result.last_message)) {
            $lastMessage = $result.last_message.TrimEnd()
            $lastMessage | Set-Content -Path $lastMessagePath -Encoding UTF8
            $lastMessage | Set-Content -Path $LatestLastMessagePath -Encoding UTF8
        }

        $doneText = if ($lastMessage) { $lastMessage } else { (Get-Content -Path $transcriptPath -Raw).TrimEnd() }
        if ($doneText -match "(?m)^RALPH_DONE\b") {
            Write-Host ""
            Write-Host "[Codex Ralph Loop] Parity achieved. Stopping." -ForegroundColor Green
            break
        }

        if (Test-Path $lastMessagePath) {
            $previousLastMessagePath = $lastMessagePath
        }
    }
    catch {
        $errorText = $_ | Out-String
        Add-Content -Path $transcriptPath -Value $errorText -Encoding UTF8
        Add-Content -Path $LatestTranscriptPath -Value $errorText -Encoding UTF8

        Write-Host "[Codex Ralph Loop] Error in iteration ${iteration}: $_" -ForegroundColor Red
        Write-Host "[Codex Ralph Loop] Retrying in $RetryDelaySeconds seconds..." -ForegroundColor Yellow
        Start-Sleep -Seconds $RetryDelaySeconds
        continue
    }

    if (Test-Path $StopFile) {
        Write-Host ""
        Write-Host "[Codex Ralph Loop] Stop file detected. Exiting after current iteration." -ForegroundColor Yellow
        break
    }

    Write-Host ""
    Write-Host "[Codex Ralph Loop] Iteration $iteration complete. Starting next in $IterationDelaySeconds seconds..." -ForegroundColor Cyan
    Start-Sleep -Seconds $IterationDelaySeconds
}

if (-not $NoCleanup) {
    Write-Host "[Codex Ralph Loop] Cleaning up..." -ForegroundColor DarkGray
    Stop-ViteProcesses
}

Write-Host "[Codex Ralph Loop] Finished after $iteration iterations." -ForegroundColor Green
Write-Host "[Codex Ralph Loop] Run artifacts saved to $RunDir" -ForegroundColor Green
