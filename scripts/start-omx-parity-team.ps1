#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Launch the OMX team workflow for native visual parity.

.DESCRIPTION
  Wraps `omx team` with the repo's parity task contract and a default
  three-lane worker split focused on transcript typography/compositing,
  sidebar or retained-layout migration, and parity measurement/evidence.

  Supports PowerShell's native `-WhatIf`/`ShouldProcess` preview mode plus an
  explicit `-Preview` switch for a no-op command summary.
#>

[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [ValidateRange(1, 32)]
    [int]$WorkerCount = 3,

    [ValidateNotNullOrEmpty()]
    [string]$AgentType = 'executor',

    [string]$Model = '',

    [string]$ReasoningEffort = '',

    [string]$OmxCommandName = 'omx',

    [string]$TaskContractPath = (Join-Path $PSScriptRoot '..\docs\omx-visual-parity-task.md'),

    [switch]$Preview
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Test-CommandExists {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Resolve-OmxCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Required command '$Name' was not found on PATH. Install oh-my-codex before launching the parity team."
    }

    return $command
}

function Test-IsWindowsHost {
    return $env:OS -eq 'Windows_NT'
}

function Enable-CodexCmdShim {
    if (-not (Test-IsWindowsHost)) {
        return $null
    }

    $codexCommand = Get-Command 'codex' -ErrorAction SilentlyContinue
    if (-not $codexCommand) {
        return $null
    }

    $codexSource = [string]$codexCommand.Source
    if ([string]::IsNullOrWhiteSpace($codexSource)) {
        return $null
    }

    if (-not $codexSource.EndsWith('.ps1', [System.StringComparison]::OrdinalIgnoreCase)) {
        return $null
    }

    $baseDir = Split-Path -Parent $codexSource
    $nodeExe = Join-Path $baseDir 'node.exe'
    $codexJs = Join-Path $baseDir 'node_modules\@openai\codex\bin\codex.js'

    if (-not (Test-Path $nodeExe) -or -not (Test-Path $codexJs)) {
        return $null
    }

    $shimDir = Join-Path ([System.IO.Path]::GetTempPath()) 'omx-codex-cmd-shim'
    $shimPath = Join-Path $shimDir 'codex.cmd'
    New-Item -ItemType Directory -Force -Path $shimDir | Out-Null

    $shimContent = @"
@echo off
"$nodeExe" "$codexJs" %*
"@

    Set-Content -Path $shimPath -Value $shimContent -Encoding ASCII

    $previousPath = $env:PATH
    $env:PATH = "$shimDir;$previousPath"

    return [pscustomobject]@{
        previous_path = $previousPath
        shim_dir = $shimDir
        shim_path = $shimPath
    }
}

function Assert-TeamBackend {
    $tmuxAvailable = Test-CommandExists -Name 'tmux'
    $psmuxAvailable = Test-CommandExists -Name 'psmux'

    if (-not $tmuxAvailable -and -not $psmuxAvailable) {
        throw "No tmux-compatible backend found. Install tmux or psmux before running `omx team`."
    }
}

function Read-TaskContract {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    if (-not (Test-Path -LiteralPath $resolvedPath)) {
        throw "Task contract not found: $resolvedPath"
    }

    return [System.IO.File]::ReadAllText($resolvedPath)
}

function Build-TeamPrompt {
    param(
        [Parameter(Mandatory = $true)]
        [string]$TaskContract,

        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $webReference = Join-Path $RepoRoot 'docs\references\web-reference.png'
    $nativeCapture = Join-Path $RepoRoot 'docs\references\current-godly-shell.png'
    $nativeDiff = Join-Path $RepoRoot 'docs\references\current-godly-shell.diff.png'
    $harness = Join-Path $RepoRoot 'scripts\measure-godly-shell-parity.ps1'

    $template = @'
{0}

Run context:
- Repository root: {1}
- Web reference screenshot: {2}
- Native screenshot: {3}
- Native diff image: {4}
- Parity harness: {5}

Current assignment:
1. Lane 1 owns transcript typography, compositing, and text-rendering correctness.
2. Lane 2 owns sidebar spacing, retained-layout migration, and chrome geometry cleanup.
3. Lane 3 owns parity measurement, screenshot evidence, and gap-log updates.

Execution rules:
- Prefer the smallest change that closes the highest-leverage parity blocker.
- Run the repo-native parity harness after meaningful changes.
- If screenshot evidence is available, use it to guide the next edit.
- Keep `docs/references/gaps.md` and `tasks/rendering-quality-iterations.md` honest.
- Do not declare completion while material parity gaps remain.
'@

    return [string]::Format(
        $template,
        $TaskContract,
        $RepoRoot,
        $webReference,
        $nativeCapture,
        $nativeDiff,
        $harness
    )
}

function Build-WorkerLaunchArgs {
    param(
        [string]$ModelValue,
        [string]$ReasoningValue
    )

    $parts = @()
    if (-not [string]::IsNullOrWhiteSpace($ModelValue)) {
        $parts += @('--model', $ModelValue)
    }
    if (-not [string]::IsNullOrWhiteSpace($ReasoningValue)) {
        $parts += @('-c', "model_reasoning_effort=`"$ReasoningValue`"")
    }

    if ($parts.Count -eq 0) {
        return $null
    }

    return ($parts -join ' ')
}

function Format-CommandPreview {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $escaped = @()
    foreach ($arg in $Arguments) {
        if ($arg -match '\s' -or $arg -match '"') {
            $escaped += ('"' + ($arg -replace '"', '\"') + '"')
        }
        else {
            $escaped += $arg
        }
    }

    return ($Command + ' ' + ($escaped -join ' '))
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$omxCommand = Resolve-OmxCommand -Name $OmxCommandName

$taskContract = Read-TaskContract -Path $TaskContractPath
$taskText = Build-TeamPrompt -TaskContract $taskContract -RepoRoot $repoRoot
$teamSpec = "$WorkerCount`:$AgentType"
$workerLaunchArgs = Build-WorkerLaunchArgs -ModelValue $Model -ReasoningValue $ReasoningEffort

$previousWorkerLaunchArgs = $env:OMX_TEAM_WORKER_LAUNCH_ARGS
$codexShim = Enable-CodexCmdShim
try {
    if (-not [string]::IsNullOrWhiteSpace($workerLaunchArgs)) {
        if ([string]::IsNullOrWhiteSpace($previousWorkerLaunchArgs)) {
            $env:OMX_TEAM_WORKER_LAUNCH_ARGS = $workerLaunchArgs
        }
        else {
            $env:OMX_TEAM_WORKER_LAUNCH_ARGS = "$previousWorkerLaunchArgs $workerLaunchArgs"
        }
    }

    $arguments = @('team', $teamSpec, $taskText)
    $previewCommand = Format-CommandPreview -Command $omxCommand.Source -Arguments $arguments

    if ($Preview) {
        Write-Host 'OMX parity team preview' -ForegroundColor Cyan
        Write-Host ('Repository: {0}' -f $repoRoot)
        Write-Host ('Worker spec: {0}' -f $teamSpec)
        if (-not [string]::IsNullOrWhiteSpace($workerLaunchArgs)) {
            Write-Host ('Worker launch args: {0}' -f $workerLaunchArgs)
        }
        if (-not (Test-CommandExists -Name 'tmux') -and -not (Test-CommandExists -Name 'psmux')) {
            Write-Warning 'No tmux-compatible backend found. This preview does not launch a team.'
        }
        Write-Host ''
        Write-Host $previewCommand
        return
    }

    if ($PSCmdlet.ShouldProcess($repoRoot, "Launch OMX parity team with $teamSpec")) {
        Assert-TeamBackend
        Write-Host ('Launching OMX parity team: {0}' -f $previewCommand) -ForegroundColor Green
        & $omxCommand @arguments
    }
}
finally {
    if ($null -ne $previousWorkerLaunchArgs) {
        $env:OMX_TEAM_WORKER_LAUNCH_ARGS = $previousWorkerLaunchArgs
    }
    else {
        Remove-Item Env:OMX_TEAM_WORKER_LAUNCH_ARGS -ErrorAction SilentlyContinue
    }

    if ($codexShim) {
        $env:PATH = $codexShim.previous_path
    }
}
