<#
.SYNOPSIS
  Ralph Loop - iterative rendering quality refinement for godly-shell.
  Claude Code handles building, launching, screenshotting, and analysis.

.PARAMETER MaxIterations
  Maximum number of iterations (0 = unlimited). Default: 40.

.PARAMETER ReferenceImages
  Paths to reference images showing target rendering quality.
#>

param(
    [int]$MaxIterations = 40,
    [string[]]$ReferenceImages = @(
        "$PSScriptRoot\docs\references\reference-zed.png",
        "$PSScriptRoot\docs\references\reference-opensessions.png"
    ),
    [string]$CurrentStateImage = "$PSScriptRoot\docs\references\current-godly-shell.png"
)

$ErrorActionPreference = "Continue"
$ProjectRoot = "C:\Users\alanm\Documents\dev\godly-claude\godly-terminal"
$promptFile = Join-Path $env:TEMP "ralph-loop-prompt.md"

function Write-PromptFile {
    param([int]$Iteration)

    $refImagesBlock = ($ReferenceImages | ForEach-Object { "- $_" }) -join "`n"

    $content = @"
USE AGENT TEAMS LIBERALLY. You are running in an autonomous iteration loop (iteration $Iteration) to improve the RENDERING QUALITY of godly-terminal's native shell (godly-shell).

## Critical Focus: RENDERING QUALITY, THEN PLACEMENT, THEN LAYOUT

This loop is also about placing elements in the right position or getting layout dimensions right.
But mainly It IS about making every rendered element look as polished as a professional desktop app (Zed, VS Code, WezTerm).

The gap is: our UI elements are flat colored rectangles. They need to look like professional app chrome.

## Reference Images
Read ALL of these images every iteration:

**Target quality** (what we want to match):
$refImagesBlock

Study the DIFFERENCE between current state and targets. Then update docs/reference/gaps.md with our current finds to iterate on it.

## Your Task

1. Read ALL reference images (both targets) to keep the quality gap fresh in mind
2. Implement the NEXT phase of rendering quality improvements
3. Build godly-shell:
   ``cd C:/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri && cargo build -p godly-shell 2>&1``
   If MSVC is not found, set up the environment first:
   ``export MSVC_VER="14.44.35207" && export VS_BASE="/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools" && export MSVC_BASE="`$VS_BASE/VC/Tools/MSVC/`$MSVC_VER" && export SDK_BASE="/c/Program Files (x86)/Windows Kits/10" && export SDK_VER="10.0.26100.0" && export PATH="`$MSVC_BASE/bin/Hostx64/x64:`$SDK_BASE/bin/`$SDK_VER/x64:`$PATH" && export LIB="`$MSVC_BASE/lib/x64;`$SDK_BASE/Lib/`$SDK_VER/ucrt/x64;`$SDK_BASE/Lib/`$SDK_VER/um/x64" && export INCLUDE="`$MSVC_BASE/include;`$SDK_BASE/Include/`$SDK_VER/ucrt;`$SDK_BASE/Include/`$SDK_VER/um;`$SDK_BASE/Include/`$SDK_VER/shared"``
   Then: ``cd /c/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri && cargo build -p godly-shell 2>&1``
   If the build fails, fix the compile error immediately. Keep going until it compiles.
4. After a successful build, launch the binary, take a screenshot, and kill it:
   ``taskkill //IM godly-native.exe //F 2>/dev/null; taskkill //IM godly-daemon.exe //F 2>/dev/null; taskkill //IM godly-pty-shim.exe //F 2>/dev/null; sleep 2``
   ``C:/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri/target/debug/godly-native.exe &``
   ``sleep 5``
   Then take a screenshot using PowerShell:
   ``powershell -Command "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; \`$s = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; \`$b = New-Object System.Drawing.Bitmap(\`$s.Width, \`$s.Height); \`$g = [System.Drawing.Graphics]::FromImage(\`$b); \`$g.CopyFromScreen(\`$s.Location, [System.Drawing.Point]::Empty, \`$s.Size); \`$b.Save('$($CurrentStateImage -replace '\\','/')', [System.Drawing.Imaging.ImageFormat]::Png); \`$g.Dispose(); \`$b.Dispose()"``
   Then kill the process:
   ``taskkill //IM godly-native.exe //F 2>/dev/null``
5. Read the screenshot you just took at: $CurrentStateImage
   Compare it against the reference images. Assess what improved and what still needs work.
6. Log progress in tasks/rendering-quality-iterations.md
7. Commit successful changes with descriptive message (feat: or fix: prefix)

## Important Rules
- YOU handle building, launching, screenshotting, and analysis. The loop script just calls you.
- Always build AND take a screenshot before declaring done.
- Commit working improvements frequently.
- The binary is godly-shell (package name), producing godly-native.exe.
- Study how Zed's GPUI renders things.

"@

    Set-Content -Path $promptFile -Value $content -Encoding UTF8
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Ralph Loop - Rendering Quality" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Focus:     ELEMENT RENDERING QUALITY (SDF, AA, borders, shadows)" -ForegroundColor Yellow
Write-Host "Claude Code handles: build, launch, screenshot, analysis" -ForegroundColor Green
Write-Host ""
Write-Host "References:" -ForegroundColor White
foreach ($ref in $ReferenceImages) {
    Write-Host "  Target:  $ref"
}
Write-Host "  Current: $CurrentStateImage"
Write-Host "Max iterations: $(if ($MaxIterations -eq 0) { 'unlimited' } else { $MaxIterations })"
Write-Host ""

$iteration = 0
while ($true) {
    $iteration++
    if ($MaxIterations -gt 0 -and $iteration -gt $MaxIterations) {
        Write-Host ""
        Write-Host "[Ralph Loop] Reached max iterations ($MaxIterations). Stopping." -ForegroundColor Yellow
        break
    }

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host ""
    Write-Host "[$timestamp] === Iteration $iteration ===" -ForegroundColor Green

    Write-PromptFile -Iteration $iteration

    try {
        $env:CLAUDE_CODE_ENTRYPOINT = "cli"
        $output = & claude --dangerously-skip-permissions --print --output-format text -p (Get-Content $promptFile -Raw) 2>&1
        $output | Write-Host

        $outputStr = $output -join "`n"
        if ($outputStr -match "RALPH_DONE") {
            Write-Host ""
            Write-Host "[Ralph Loop] Rendering quality target reached! Stopping." -ForegroundColor Green
            break
        }
    }
    catch {
        Write-Host "[Ralph Loop] Error in iteration ${iteration}: $_" -ForegroundColor Red
        Write-Host "Retrying in 10 seconds..." -ForegroundColor Yellow
        Start-Sleep -Seconds 10
    }

    Write-Host ""
    Write-Host "[Ralph Loop] Iteration $iteration complete. Starting next in 5s..." -ForegroundColor Cyan
    Start-Sleep -Seconds 5
}

Write-Host ""
Write-Host "[Ralph Loop] Finished after $iteration iterations." -ForegroundColor Green

Remove-Item $promptFile -ErrorAction SilentlyContinue
