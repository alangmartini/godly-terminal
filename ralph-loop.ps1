<#
.SYNOPSIS
  Ralph Loop - iterative visual quality refinement for godly-shell.
  Runs Claude Code in dangerously-skip-permissions mode to build,
  screenshot, compare, and improve the UI until it matches the reference.

.PARAMETER MaxIterations
  Maximum number of iterations (0 = unlimited). Default: 20.

.PARAMETER ReferenceImage
  Path to the reference image to match.
#>

param(
    [int]$MaxIterations = 20,
    [string]$ReferenceImage = "$env:USERPROFILE\Downloads\570343101-2caaee1a-b3f5-4041-aa3c-5b3668aa1912.png"
)

$ErrorActionPreference = "Continue"
$ProjectRoot = "C:\Users\User\godly-terminal"

# Write the prompt to a temp file to avoid all quoting issues
$promptFile = Join-Path $env:TEMP "ralph-loop-prompt.md"

function Write-PromptFile {
    param([int]$Iteration)

    $content = @"
You are running in an autonomous iteration loop (iteration $Iteration) to improve the visual rendering quality of godly-terminal native shell (godly-shell).

## Reference Image
Read this reference image to understand the target visual quality:
$ReferenceImage

The reference shows a polished terminal multiplexer (tmux in a styled terminal) with:
- Deep dark background (~#1e1e2e / Catppuccin Mocha style)
- Left sidebar (~200px wide) with Sessions header, numbered session list, active item highlighted with a colored left border
- Top tab bar with numbered tabs (1, 2, 3, 4, 5), each with colored indicators, active tab visually distinct
- Main content area with terminal output (styled text, markdown, code blocks)
- Right pane showing rendered text content - demonstrates split pane capability
- Bottom status bar with working directory path, git branch info
- Bottom-left session manager panel showing running agents/processes
- Crisp ClearType subpixel text rendering throughout
- Subtle thin borders between panes, muted UI chrome colors
- Professional minimal aesthetic - similar to Zed, WezTerm, or modern tmux with catppuccin theme

## Current State (what we have now)
The app currently looks very bare:
- Gray title bar with Godly Terminal text
- Tiny sidebar on left (~15px wide) with just a small colored square and + button
- Terminal area takes up most of the window with medium-gray background
- No tab bar visible (tabs exist in code but are tiny/invisible)
- No status bar visible at bottom
- No split panes visible
- Working prompt (PS C:\Users\User>) with text rendering functional
- The gap is HUGE - we need to go from bare functional to polished production UI

## Your Task
Make multiple improvements per iteration to close the visual gap fast. Each iteration:

1. Read the reference image (the path above) to keep the target fresh
2. Read the current UI code - key files in src-tauri/native/godly-shell/src/:
   - ui/layout.rs (layout dimensions and regions)
   - ui/sidebar.rs (sidebar rendering)
   - ui/tab_bar.rs (tab bar rendering)
   - ui/title_bar.rs (title bar)
   - ui/status_bar.rs (status bar)
   - ui/text_renderer.rs (UI text)
   - ui/quad_renderer.rs (rectangles/backgrounds)
   - main.rs (App struct, render pipeline)
   - terminal_renderer.rs (terminal content)
3. Make changes - you can tackle multiple related improvements at once
4. Build the project using:
   export MSVC_VER="14.40.33807" && export VS_BASE="/c/Program Files/Microsoft Visual Studio/2022/Community" && export MSVC_BASE="`$VS_BASE/VC/Tools/MSVC/`$MSVC_VER" && export SDK_BASE="/c/Program Files (x86)/Windows Kits/10" && export SDK_VER="10.0.22621.0" && export PATH="`$MSVC_BASE/bin/Hostx64/x64:`$SDK_BASE/bin/`$SDK_VER/x64:`$PATH" && export LIB="`$MSVC_BASE/lib/x64;`$SDK_BASE/Lib/`$SDK_VER/ucrt/x64;`$SDK_BASE/Lib/`$SDK_VER/um/x64" && export INCLUDE="`$MSVC_BASE/include;`$SDK_BASE/Include/`$SDK_VER/ucrt;`$SDK_BASE/Include/`$SDK_VER/um;`$SDK_BASE/Include/`$SDK_VER/shared" && cd /c/Users/User/godly-terminal/src-tauri && cargo build -p godly-shell 2>&1
5. Kill any running instance, launch, screenshot, and compare:
   cmd //C "taskkill /IM godly-native.exe /F" 2>/dev/null; cmd //C "taskkill /IM godly-daemon.exe /F" 2>/dev/null; cmd //C "taskkill /IM godly-pty-shim.exe /F" 2>/dev/null; sleep 2; /c/Users/User/godly-terminal/src-tauri/target/debug/godly-native.exe & sleep 5
   Then use PyAutoGUI to maximize, focus, and screenshot. Then Read the screenshot file.
6. Log progress in tasks/rendering-quality-iterations.md
7. Commit successful changes with a descriptive message (feat: or fix: prefix)
8. Decide: if visual quality closely matches the reference, say RALPH_DONE. Otherwise continue.

## Visual Priorities (tackle in this order)
1. Layout dimensions - sidebar width ~200-220px, proper tab bar height ~35px, status bar height ~25px
2. Color scheme - deep dark background (#1e1e2e), sidebar slightly lighter (#252535), tab bar dark, borders subtle (#313145)
3. Sidebar content - Sessions header text, session list items with names, active indicator (colored left border), proper padding/spacing
4. Tab bar - numbered tabs, active tab highlight, + button for new tab, proper styling
5. Status bar - working directory, branch info, subtle dark background
6. Terminal area - darker background matching the deep dark theme (#1e1e2e for terminal bg)
7. Polish - consistent spacing, proper font sizes, subtle hover effects, thin pane borders

## Important Rules
- You CAN make multiple related changes per iteration
- Always build and visually verify with a screenshot before declaring done
- If a build fails, fix the compile error immediately
- Commit working improvements frequently
- The binary is godly-shell (package name), producing godly-native.exe
- Build from src-tauri/: cargo build -p godly-shell

If you believe the visual quality now matches the reference closely enough, include the word RALPH_DONE in your response.
"@

    Set-Content -Path $promptFile -Value $content -Encoding UTF8
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Ralph Loop - Visual Quality Iteration" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Reference: $ReferenceImage"
Write-Host "Max iterations: $(if ($MaxIterations -eq 0) { 'unlimited' } else { $MaxIterations })"
Write-Host "Project: $ProjectRoot"
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

    # Write prompt to temp file (avoids all quoting/escaping issues)
    Write-PromptFile -Iteration $iteration

    try {
        $env:CLAUDE_CODE_ENTRYPOINT = "cli"
        $output = & claude --dangerously-skip-permissions --print --output-format text -p (Get-Content $promptFile -Raw) 2>&1
        $output | Write-Host

        $outputStr = $output -join "`n"
        if ($outputStr -match "RALPH_DONE") {
            Write-Host ""
            Write-Host "[Ralph Loop] Claude declared visual parity reached! Stopping." -ForegroundColor Green
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

# Cleanup
Remove-Item $promptFile -ErrorAction SilentlyContinue
