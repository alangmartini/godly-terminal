<#
.SYNOPSIS
    Run /test-hygiene skill via Claude Code on logical groups of test files.

.DESCRIPTION
    Launches headless Claude Code sessions (--dangerously-skip-permissions) to
    analyze test quality, validity, output hygiene, and test smells across the
    project's test suite. Tests are grouped by feature domain so each session
    gets a focused, coherent set of files.

.PARAMETER Group
    Run only a specific group (or comma-separated list). Tab-completes.
    Example: -Group "terminal-pane"
    Example: -Group "terminal-pane,tab-bar"
    Omit to run all groups.

.PARAMETER List
    List available groups without running anything.

.PARAMETER OutputDir
    Directory for analysis reports. Default: ./test-hygiene-reports

.PARAMETER MaxParallel
    Max concurrent Claude sessions. Default: 3

.EXAMPLE
    .\scripts\hygienize_tests.ps1
    .\scripts\hygienize_tests.ps1 -Group daemon
    .\scripts\hygienize_tests.ps1 -List
    .\scripts\hygienize_tests.ps1 -Group "godly-vt,daemon" -MaxParallel 2
#>

param(
    [string]$Group = "",
    [switch]$List,
    [string]$OutputDir = "./test-hygiene-reports",
    [int]$MaxParallel = 3
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$ProjectRoot/package.json")) {
    $ProjectRoot = (Get-Location).Path
}

# ── Test Groups ──────────────────────────────────────────────────────────────
# Each group: name, description, list of file globs relative to project root.

$TestGroups = [ordered]@{
    "terminal-pane" = @{
        Desc  = "TerminalPane component (scroll, keyboard, focus, rendering)"
        Files = @(
            "src/components/TerminalPane.scroll.test.ts"
            "src/components/TerminalPane.scroll-preservation.test.ts"
            "src/components/TerminalPane.scroll-regression.test.ts"
            "src/components/TerminalPane.dead-keys.test.ts"
            "src/components/TerminalPane.keyboard.test.ts"
            "src/components/TerminalPane.home-end-key.test.ts"
            "src/components/TerminalPane.ctrl-arrow.test.ts"
            "src/components/TerminalPane.focus-recovery.test.ts"
            "src/components/TerminalPane.dialog-focus-steal.test.ts"
            "src/components/TerminalPane.zoom-flash.test.ts"
            "src/components/TerminalPane.tab-switch.test.ts"
            "src/components/TerminalPane.output-buffer.test.ts"
            "src/components/TerminalPane.activation.test.ts"
        )
    }
    "tab-bar" = @{
        Desc  = "TabBar component (drag, rename, overflow, process titles)"
        Files = @(
            "src/components/TabBar.test.ts"
            "src/components/TabBar.drag.test.ts"
            "src/components/TabBar.rename.test.ts"
            "src/components/TabBar.rename-focus-steal.test.ts"
            "src/components/TabBar.overflow.test.ts"
            "src/components/TabBar.process-title.test.ts"
        )
    }
    "state" = @{
        Desc  = "App state, store, keybindings, and drag state"
        Files = @(
            "src/state/store.test.ts"
            "src/state/store.split-navigation.test.ts"
            "src/state/store.mcp-window.test.ts"
            "src/state/keybinding-store.test.ts"
            "src/state/terminal-settings-store.test.ts"
            "src/state/settings-tab-store.test.ts"
            "src/state/drag-state.test.ts"
        )
    }
    "app" = @{
        Desc  = "App component and notifications"
        Files = @(
            "src/components/App.test.ts"
            "src/components/App.notification.test.ts"
        )
    }
    "services" = @{
        Desc  = "Terminal, workspace, and notification services"
        Files = @(
            "src/services/workspace-service.test.ts"
            "src/services/terminal-service.test.ts"
            "src/services/terminal-service.wsl-bug.test.ts"
            "src/services/idle-notification-service.test.ts"
        )
    }
    "renderer" = @{
        Desc  = "Canvas2D renderer (color cache, cell encoder, glyph atlas)"
        Files = @(
            "src/components/renderer/ColorCache.test.ts"
            "src/components/renderer/CellDataEncoder.test.ts"
            "src/components/renderer/GlyphAtlas.test.ts"
        )
    }
    "plugins" = @{
        Desc  = "Plugin system, SmolLM2, event bus"
        Files = @(
            "src/plugins/smollm2/smollm2.test.ts"
            "src/plugins/smollm2/llm-service.test.ts"
            "src/plugins/smollm2/download-retry.test.ts"
            "src/plugins/plugin-registry.test.ts"
            "src/plugins/plugin-store.test.ts"
            "src/plugins/event-bus.test.ts"
            "src/plugins/peon-ping/peon-ping.test.ts"
        )
    }
    "ui-misc" = @{
        Desc  = "Dialogs, OSC titles, perf tracer, utilities"
        Files = @(
            "src/components/CopyDialog.test.ts"
            "src/components/FileEditorDialog.test.ts"
            "src/components/osc-title.integration.test.ts"
            "src/utils/PerfTracer.test.ts"
            "src/utils/quote-path.test.ts"
        )
    }
    "daemon" = @{
        Desc  = "Daemon integration tests (persistence, stability, performance)"
        Files = @(
            "src-tauri/daemon/tests/read_grid.rs"
            "src-tauri/daemon/tests/ctrl_c_interrupt.rs"
            "src-tauri/daemon/tests/input_latency.rs"
            "src-tauri/daemon/tests/input_latency_full_path.rs"
            "src-tauri/daemon/tests/memory_stress.rs"
            "src-tauri/daemon/tests/paste_image_freeze.rs"
            "src-tauri/daemon/tests/session_persistence.rs"
            "src-tauri/daemon/tests/zombie_tabs.rs"
            "src-tauri/daemon/tests/scroll_position_preservation.rs"
            "src-tauri/daemon/tests/single_instance.rs"
            "src-tauri/daemon/tests/handler_starvation.rs"
            "src-tauri/daemon/tests/test_isolation_guardrail.rs"
        )
    }
    "godly-vt" = @{
        Desc  = "Terminal state engine (VT compliance, scrolling, dirty tracking)"
        Files = @(
            "src-tauri/godly-vt/tests/vt_compliance.rs"
            "src-tauri/godly-vt/tests/scroll.rs"
            "src-tauri/godly-vt/tests/dirty_tracking.rs"
            "src-tauri/godly-vt/tests/escape.rs"
            "src-tauri/godly-vt/tests/basic.rs"
            "src-tauri/godly-vt/tests/tab_switch_resize.rs"
            "src-tauri/godly-vt/tests/osc.rs"
            "src-tauri/godly-vt/tests/csi.rs"
            "src-tauri/godly-vt/tests/processing.rs"
            "src-tauri/godly-vt/tests/mode.rs"
        )
    }
    "rust-misc" = @{
        Desc  = "Protocol, LLM, and benchmark tests"
        Files = @(
            "src-tauri/llm/tests/download_url_parsing.rs"
            "src-tauri/llm/tests/download_error_quality.rs"
            "src-tauri/protocol/benches/snapshot_serialization.rs"
            "src-tauri/godly-vt/benches/throughput.rs"
        )
    }
}

# ── List mode ────────────────────────────────────────────────────────────────

if ($List) {
    Write-Host "`nAvailable test groups:`n" -ForegroundColor Cyan
    foreach ($name in $TestGroups.Keys) {
        $g = $TestGroups[$name]
        $count = $g.Files.Count
        Write-Host "  $($name.PadRight(16))" -ForegroundColor Yellow -NoNewline
        Write-Host " ($count files) " -ForegroundColor DarkGray -NoNewline
        Write-Host $g.Desc
    }
    $total = ($TestGroups.Values | ForEach-Object { $_.Files.Count } | Measure-Object -Sum).Sum
    Write-Host "`n  Total: $total test files in $($TestGroups.Count) groups`n" -ForegroundColor DarkGray
    exit 0
}

# ── Resolve which groups to run ──────────────────────────────────────────────

if ($Group) {
    $selectedNames = $Group -split "," | ForEach-Object { $_.Trim() }
    foreach ($name in $selectedNames) {
        if (-not $TestGroups.Contains($name)) {
            Write-Host "Unknown group: $name" -ForegroundColor Red
            Write-Host "Use -List to see available groups." -ForegroundColor DarkGray
            exit 1
        }
    }
} else {
    $selectedNames = @($TestGroups.Keys)
}

# ── Validate files exist ────────────────────────────────────────────────────

foreach ($name in $selectedNames) {
    $g = $TestGroups[$name]
    $missing = @()
    foreach ($f in $g.Files) {
        $full = Join-Path $ProjectRoot $f
        if (-not (Test-Path $full)) {
            $missing += $f
        }
    }
    if ($missing.Count -gt 0) {
        Write-Host "Warning: $($missing.Count) file(s) not found in group '$name':" -ForegroundColor Yellow
        $missing | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
        # Filter to only existing files
        $TestGroups[$name].Files = $g.Files | Where-Object { Test-Path (Join-Path $ProjectRoot $_) }
        if ($TestGroups[$name].Files.Count -eq 0) {
            Write-Host "  Skipping group '$name' (no valid files)" -ForegroundColor Yellow
            $selectedNames = $selectedNames | Where-Object { $_ -ne $name }
        }
    }
}

if ($selectedNames.Count -eq 0) {
    Write-Host "No groups to run." -ForegroundColor Red
    exit 1
}

# ── Prepare output dir ───────────────────────────────────────────────────────

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$timestamp = Get-Date -Format "yyyy-MM-dd_HH-mm"
$runDir = Join-Path $OutputDir $timestamp
New-Item -ItemType Directory -Path $runDir -Force | Out-Null

Write-Host "`n=== Test Hygiene Analysis ===" -ForegroundColor Cyan
Write-Host "Groups:    $($selectedNames -join ', ')" -ForegroundColor White
Write-Host "Parallel:  $MaxParallel" -ForegroundColor White
Write-Host "Output:    $runDir" -ForegroundColor White
Write-Host ""

# ── Run groups ───────────────────────────────────────────────────────────────

$jobs = @()
$completed = 0
$total = $selectedNames.Count

foreach ($name in $selectedNames) {
    $g = $TestGroups[$name]
    $fileList = ($g.Files | ForEach-Object { "`"$_`"" }) -join " "
    $outFile = Join-Path $runDir "$name.md"

    # Build the prompt for Claude
    $prompt = @"
/test-hygiene

Analyze the following test files for quality, validity, output hygiene, and test smells.
Group: $name - $($g.Desc)

Files to analyze:
$($g.Files | ForEach-Object { "- $_" } | Out-String)

Write your full analysis report. Be specific about file names and line numbers.
Focus on: redundant tests, weak assertions, excessive mocking, output noise,
tests that don't actually test anything meaningful, and missing edge cases.
"@

    # Throttle parallel jobs
    while (($jobs | Where-Object { $_.State -eq 'Running' }).Count -ge $MaxParallel) {
        Start-Sleep -Milliseconds 500
    }

    Write-Host "  [$($completed + 1)/$total] Starting: " -NoNewline -ForegroundColor DarkGray
    Write-Host $name -ForegroundColor Yellow -NoNewline
    Write-Host " ($($g.Files.Count) files)" -ForegroundColor DarkGray

    $job = Start-Job -ScriptBlock {
        param($Prompt, $OutFile, $ProjectRoot)
        Set-Location $ProjectRoot
        $result = & claude --dangerously-skip-permissions -p $Prompt --output-format text 2>&1
        $result | Out-File -FilePath $OutFile -Encoding utf8
        return @{ ExitCode = $LASTEXITCODE; OutputFile = $OutFile }
    } -ArgumentList $prompt, $outFile, $ProjectRoot

    $jobs += @{ Name = $name; Job = $job; OutFile = $outFile }
    $completed++
}

# ── Wait for all jobs ────────────────────────────────────────────────────────

Write-Host "`nWaiting for all sessions to complete..." -ForegroundColor DarkGray

$results = @()
foreach ($j in $jobs) {
    $result = Receive-Job -Job $j.Job -Wait
    Remove-Job -Job $j.Job

    $hasOutput = (Test-Path $j.OutFile) -and ((Get-Item $j.OutFile).Length -gt 0)

    if ($hasOutput) {
        Write-Host "  Done: " -NoNewline -ForegroundColor DarkGray
        Write-Host $j.Name -ForegroundColor Green
        $results += @{ Name = $j.Name; Status = "ok"; File = $j.OutFile }
    } else {
        Write-Host "  FAIL: " -NoNewline -ForegroundColor DarkGray
        Write-Host $j.Name -ForegroundColor Red
        $results += @{ Name = $j.Name; Status = "fail"; File = $j.OutFile }
    }
}

# ── Summary ──────────────────────────────────────────────────────────────────

$ok = ($results | Where-Object { $_.Status -eq "ok" }).Count
$fail = ($results | Where-Object { $_.Status -eq "fail" }).Count

Write-Host "`n=== Summary ===" -ForegroundColor Cyan
Write-Host "  Completed: $ok / $total" -ForegroundColor $(if ($fail -eq 0) { "Green" } else { "Yellow" })
if ($fail -gt 0) {
    Write-Host "  Failed:    $fail" -ForegroundColor Red
}
Write-Host "  Reports:   $runDir" -ForegroundColor White
Write-Host ""

# ── Generate index ───────────────────────────────────────────────────────────

$indexFile = Join-Path $runDir "INDEX.md"
$indexContent = @"
# Test Hygiene Report - $timestamp

| Group | Files | Status | Report |
|-------|-------|--------|--------|
"@

foreach ($r in $results) {
    $g = $TestGroups[$r.Name]
    $count = $g.Files.Count
    $status = if ($r.Status -eq "ok") { "Done" } else { "Failed" }
    $indexContent += "`n| $($r.Name) | $count | $status | [$($r.Name).md]($($r.Name).md) |"
}

$indexContent | Out-File -FilePath $indexFile -Encoding utf8
Write-Host "Index written to: $indexFile" -ForegroundColor DarkGray
