# check-memory-leaks.ps1
# Automated memory leak detection script for Godly Terminal daemon.
#
# Usage: .\scripts\check-memory-leaks.ps1
#
# This script:
# 1. Runs RSS stress tests (create/destroy, attach/detach, heavy output)
# 2. Optionally runs DHAT profiling for detailed heap analysis

param(
    [switch]$DhatProfile,
    [int]$DhatSessionCount = 50,
    [int]$DhatExerciseDurationSec = 10
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$SrcTauri = Join-Path $ProjectRoot "src-tauri"

Write-Host "======================================" -ForegroundColor Cyan
Write-Host "  Godly Terminal Memory Leak Checker  " -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""

# ------------------------------------------------------------------
# Step 1: RSS Stress Tests
# ------------------------------------------------------------------
Write-Host "[1/2] Running RSS stress tests..." -ForegroundColor Yellow
Write-Host ""

Push-Location $SrcTauri
try {
    $testResult = & cargo test -p godly-daemon --test memory_stress -- --nocapture 2>&1
    $testExitCode = $LASTEXITCODE

    # Print all output
    $testResult | ForEach-Object { Write-Host $_ }

    if ($testExitCode -eq 0) {
        Write-Host ""
        Write-Host "[PASS] RSS stress tests passed" -ForegroundColor Green
    } else {
        Write-Host ""
        Write-Host "[FAIL] RSS stress tests failed (exit code: $testExitCode)" -ForegroundColor Red
        Write-Host "Memory leak detected! Review the output above for details." -ForegroundColor Red
        Pop-Location
        exit 1
    }
} finally {
    Pop-Location
}

Write-Host ""

# ------------------------------------------------------------------
# Step 2: DHAT Profiling (optional)
# ------------------------------------------------------------------
if ($DhatProfile) {
    Write-Host "[2/2] Running DHAT profiling..." -ForegroundColor Yellow
    Write-Host ""

    Push-Location $SrcTauri
    try {
        # Build daemon with DHAT
        Write-Host "  Building daemon with leak-check feature..."
        & cargo build -p godly-daemon --features leak-check
        if ($LASTEXITCODE -ne 0) {
            Write-Host "[FAIL] Failed to build daemon with leak-check" -ForegroundColor Red
            Pop-Location
            exit 1
        }

        $daemonExe = Join-Path $SrcTauri "target\debug\godly-daemon.exe"
        $pipeName = "\\.\pipe\godly-dhat-check-$$"

        # Spawn daemon
        Write-Host "  Spawning daemon for DHAT profiling (pipe: $pipeName)..."
        $env:GODLY_PIPE_NAME = $pipeName
        $env:GODLY_NO_DETACH = "1"
        $daemonProcess = Start-Process -FilePath $daemonExe -PassThru -NoNewWindow -RedirectStandardError (Join-Path $SrcTauri "dhat-daemon-stderr.log")

        Start-Sleep -Seconds 2

        Write-Host "  Daemon running (PID: $($daemonProcess.Id))"
        Write-Host "  Let it run for $DhatExerciseDurationSec seconds..."
        Write-Host "  (For better results, connect the app to this daemon and use it.)"

        Start-Sleep -Seconds $DhatExerciseDurationSec

        # Stop daemon
        Write-Host "  Stopping daemon..."
        Stop-Process -Id $daemonProcess.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2

        # Clean up env vars
        Remove-Item Env:\GODLY_PIPE_NAME -ErrorAction SilentlyContinue
        Remove-Item Env:\GODLY_NO_DETACH -ErrorAction SilentlyContinue

        # Check for DHAT output
        $dhatFile = Join-Path $SrcTauri "dhat-heap.json"
        if (Test-Path $dhatFile) {
            $size = (Get-Item $dhatFile).Length
            Write-Host ""
            Write-Host "[PASS] DHAT profile generated: $dhatFile ($size bytes)" -ForegroundColor Green
            Write-Host "  Open in: https://nnethercote.github.io/dh_view/dh_view.html" -ForegroundColor Cyan
        } else {
            Write-Host ""
            Write-Host "[WARN] No dhat-heap.json found. The daemon may have exited before writing it." -ForegroundColor Yellow
            Write-Host "  DHAT writes on graceful exit. Try running the daemon manually:" -ForegroundColor Yellow
            Write-Host '  $env:GODLY_NO_DETACH="1"; cargo run -p godly-daemon --features leak-check' -ForegroundColor Yellow
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "[2/2] DHAT profiling skipped (use -DhatProfile to enable)" -ForegroundColor DarkGray
}

Write-Host ""
Write-Host "======================================" -ForegroundColor Cyan
Write-Host "  Memory leak check complete          " -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
