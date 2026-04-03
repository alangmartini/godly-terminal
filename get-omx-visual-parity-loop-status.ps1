<#
.SYNOPSIS
  Show status for the latest OMX visual parity loop run.

.DESCRIPTION
  Summarizes the latest run directory, file activity, relevant processes, and a
  simple stalled-run heuristic so it is easy to tell whether the loop is still
  making progress or is hung after a completed model turn.
#>

param(
    [string]$ArtifactsRoot = (Join-Path $PSScriptRoot ".omx-visual-parity-loop"),
    [string]$RunId = "",
    [int]$TailLines = 40,
    [int]$StallMinutes = 10
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-RunDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [string]$RequestedRunId
    )

    $resolvedRoot = [System.IO.Path]::GetFullPath($Root)
    if (-not (Test-Path -LiteralPath $resolvedRoot)) {
        throw "Artifacts root not found: $resolvedRoot"
    }

    if (-not [string]::IsNullOrWhiteSpace($RequestedRunId)) {
        $requestedDir = Join-Path $resolvedRoot $RequestedRunId
        if (-not (Test-Path -LiteralPath $requestedDir)) {
            throw "Run directory not found: $requestedDir"
        }
        return [System.IO.Path]::GetFullPath($requestedDir)
    }

    $latestRun = Get-ChildItem -LiteralPath $resolvedRoot -Directory |
        Where-Object { $_.Name -match '^\d{8}-\d{6}$' } |
        Sort-Object Name -Descending |
        Select-Object -First 1

    if (-not $latestRun) {
        throw "No run directories found in $resolvedRoot"
    }

    return $latestRun.FullName
}

function Get-LatestIterationTag {
    param([Parameter(Mandatory = $true)][string]$RunDir)

    $match = Get-ChildItem -LiteralPath $RunDir -File |
        Where-Object { $_.Name -match '^iteration-(\d{4})-' } |
        ForEach-Object {
            [pscustomobject]@{
                tag = $Matches[1]
                name = $_.Name
            }
        } |
        Sort-Object tag -Descending |
        Select-Object -First 1

    if (-not $match) {
        return $null
    }

    return [string]$match.tag
}

function Get-FileInfoOrNull {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $null
    }

    return Get-Item -LiteralPath $Path
}

function Get-OptionalContent {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [int]$Tail = 40
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return $null
    }

    $content = Get-Content -LiteralPath $Path -Tail $Tail -ErrorAction Stop
    if ($null -eq $content) {
        return ""
    }

    return ($content -join [Environment]::NewLine)
}

function Get-RelatedProcesses {
    param(
        [Parameter(Mandatory = $true)][string]$RunDir
    )

    $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue

    $runProcesses = @(
        $all | Where-Object {
            $commandLine = [string]$_.CommandLine
            -not [string]::IsNullOrWhiteSpace($commandLine) -and $commandLine.Contains($RunDir)
        }
    )

    $loopProcesses = @(
        $all | Where-Object {
            $commandLine = [string]$_.CommandLine
            -not [string]::IsNullOrWhiteSpace($commandLine) -and $commandLine.Contains("omx-visual-parity-loop.ps1")
        }
    )

    return ($runProcesses + $loopProcesses | Sort-Object ProcessId -Unique)
}

function Format-Age {
    param([datetime]$Timestamp)

    if ($null -eq $Timestamp) {
        return "n/a"
    }

    $age = (Get-Date) - $Timestamp
    return ("{0:N1} min" -f $age.TotalMinutes)
}

$ArtifactsRoot = [System.IO.Path]::GetFullPath($ArtifactsRoot)
$runDir = Get-RunDirectory -Root $ArtifactsRoot -RequestedRunId $RunId
$resolvedRunId = Split-Path -Leaf $runDir
$iterationTag = Get-LatestIterationTag -RunDir $runDir

if (-not $iterationTag) {
    throw "No iteration files found in $runDir"
}

$promptPath = Join-Path $runDir ("iteration-{0}-prompt.md" -f $iterationTag)
$transcriptPath = Join-Path $runDir ("iteration-{0}-transcript.txt" -f $iterationTag)
$eventsPath = Join-Path $runDir ("iteration-{0}-events.jsonl" -f $iterationTag)
$lastMessagePath = Join-Path $runDir ("iteration-{0}-last-message.txt" -f $iterationTag)

$promptFile = Get-FileInfoOrNull -Path $promptPath
$transcriptFile = Get-FileInfoOrNull -Path $transcriptPath
$eventsFile = Get-FileInfoOrNull -Path $eventsPath
$lastMessageFile = Get-FileInfoOrNull -Path $lastMessagePath

$relatedProcesses = Get-RelatedProcesses -RunDir $runDir
$codexProcesses = @($relatedProcesses | Where-Object { $_.Name -match '^codex(\.exe)?$|^node(\.exe)?$' })
$loopProcesses = @($relatedProcesses | Where-Object { $_.Name -match '^powershell(\.exe)?$|^pwsh(\.exe)?$' })

$now = Get-Date
$mostRecentFileTime = @($promptFile, $transcriptFile, $eventsFile, $lastMessageFile) |
    Where-Object { $null -ne $_ } |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

$status = "UNKNOWN"
$reason = ""

if ($lastMessageFile -and $codexProcesses.Count -gt 0) {
    $staleMinutes = ($now - $lastMessageFile.LastWriteTime).TotalMinutes
    if ($staleMinutes -ge $StallMinutes) {
        $status = "LIKELY_STUCK_AFTER_FINAL_MESSAGE"
        $reason = "last-message exists, but codex is still running {0:N1} minutes later" -f $staleMinutes
    }
}

if ($status -eq "UNKNOWN") {
    if ($codexProcesses.Count -gt 0 -or $loopProcesses.Count -gt 0) {
        if ($mostRecentFileTime) {
            $idleMinutes = ($now - $mostRecentFileTime.LastWriteTime).TotalMinutes
            if ($idleMinutes -ge $StallMinutes) {
                $status = "LIKELY_STUCK"
                $reason = "no run artifact changed for {0:N1} minutes while loop processes are still alive" -f $idleMinutes
            }
            else {
                $status = "RUNNING"
                $reason = "run artifacts changed {0:N1} minutes ago" -f $idleMinutes
            }
        }
        else {
            $status = "RUNNING"
            $reason = "loop-related processes are active"
        }
    }
    elseif ($lastMessageFile) {
        $status = "EXITED_WITH_LAST_MESSAGE"
        $reason = "no active loop process, but the latest iteration produced a last-message file"
    }
    else {
        $status = "NO_ACTIVE_PROCESS"
        $reason = "no active loop process found for this run"
    }
}

Write-Host ("Run ID:     {0}" -f $resolvedRunId) -ForegroundColor Cyan
Write-Host ("Run dir:    {0}" -f $runDir) -ForegroundColor Cyan
Write-Host ("Iteration:  {0}" -f $iterationTag) -ForegroundColor Cyan
Write-Host ("Status:     {0}" -f $status) -ForegroundColor Yellow
Write-Host ("Why:        {0}" -f $reason) -ForegroundColor Yellow
Write-Host ""

foreach ($entry in @(
    @{ label = "Prompt"; file = $promptFile },
    @{ label = "Transcript"; file = $transcriptFile },
    @{ label = "Events"; file = $eventsFile },
    @{ label = "Last msg"; file = $lastMessageFile }
)) {
    if ($null -eq $entry.file) {
        Write-Host ("{0,-10} missing" -f $entry.label)
    }
    else {
        Write-Host ("{0,-10} {1,8} bytes  updated {2}  at {3}" -f $entry.label, $entry.file.Length, (Format-Age -Timestamp $entry.file.LastWriteTime), $entry.file.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"))
    }
}

Write-Host ""
Write-Host ("Processes:  {0}" -f $relatedProcesses.Count) -ForegroundColor Cyan
foreach ($proc in $relatedProcesses | Sort-Object CreationDate,ProcessId) {
    $commandLine = [string]$proc.CommandLine
    if ($commandLine.Length -gt 180) {
        $commandLine = $commandLine.Substring(0, 180) + "..."
    }
    Write-Host ("  PID {0} PPID {1} {2}  started {3}" -f $proc.ProcessId, $proc.ParentProcessId, $proc.Name, ([datetime]$proc.CreationDate).ToString("yyyy-MM-dd HH:mm:ss"))
    Write-Host ("    {0}" -f $commandLine)
}

if ($lastMessageFile) {
    $tail = Get-OptionalContent -Path $lastMessagePath -Tail $TailLines
    if (-not [string]::IsNullOrWhiteSpace($tail)) {
        Write-Host ""
        Write-Host "Last message tail:" -ForegroundColor Cyan
        Write-Host $tail
    }
}
