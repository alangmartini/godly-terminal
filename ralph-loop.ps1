<#
.SYNOPSIS
  Ralph Loop - iterative rendering quality refinement for godly-shell.
  Focuses on the GRAPHICAL QUALITY of UI elements (SDF rounded rects,
  shadows, borders, anti-aliasing) — not layout or placement.

.PARAMETER MaxIterations
  Maximum number of iterations (0 = unlimited). Default: 20.

.PARAMETER ReferenceImage
  Path to the reference image showing target rendering quality.
#>

param(
    [int]$MaxIterations = 40,
    [string]$ReferenceImage = "$env:USERPROFILE\Documents\ShareX\Screenshots\2026-03\brave_CMuHy1dZCO.png"
)

$ErrorActionPreference = "Continue"
$ProjectRoot = "C:\Users\alanm\Documents\dev\godly-claude\godly-terminal"

# Write the prompt to a temp file to avoid all quoting issues
$promptFile = Join-Path $env:TEMP "ralph-loop-prompt.md"

function Write-PromptFile {
    param([int]$Iteration)

    $content = @"
USE AGENT TEAMS LIBERALLY. You are running in an autonomous iteration loop (iteration $Iteration) to improve the RENDERING QUALITY of godly-terminal's native shell (godly-shell).

## Critical Focus: RENDERING QUALITY, NOT LAYOUT

This loop is NOT about placing elements in the right position or getting layout dimensions right.
It IS about making every rendered element look as polished as a professional desktop app (Zed, VS Code, WezTerm).

The gap is: our UI elements are flat colored rectangles. They need to look like professional app chrome.

## Reference Image
Read this reference image to see the TARGET RENDERING QUALITY:
$ReferenceImage

Study these specific quality attributes in the reference:
- **Rounded corners** on tabs, panels, buttons — smooth anti-aliased curves, not jagged pixels
- **Subtle borders** — thin 1px borders between panels with slight color differentiation
- **Depth and separation** — elements feel layered via subtle color shifts, not drop shadows necessarily
- **Smooth text rendering** — ClearType subpixel quality, proper spacing, no jagged edges
- **Color transitions** — active vs inactive tabs differ subtly, hover states are smooth
- **Professional polish** — elements feel "solid" and well-defined, not like colored rectangles

## Current Architecture
The app uses raw winit + wgpu. Key rendering code:
- ``src-tauri/native/godly-shell/src/ui/quad_renderer.rs`` — THE MAIN TARGET. Currently draws FLAT solid-color rectangles. The WGSL shader just does ``pow(c.rgb, vec3<f32>(2.2))`` sRGB conversion. No rounded corners, no borders, no shadows, no gradients.
- ``src-tauri/native/godly-shell/src/ui/builder.rs`` — UI builder that emits quads
- ``src-tauri/native/godly-shell/src/ui/sidebar.rs`` — sidebar rendering
- ``src-tauri/native/godly-shell/src/ui/tab_bar.rs`` — tab bar rendering
- ``src-tauri/native/godly-shell/src/ui/title_bar.rs`` — title bar
- ``src-tauri/native/godly-shell/src/ui/status_bar.rs`` — status bar
- ``src-tauri/native/godly-shell/src/ui/text_renderer.rs`` — UI text
- ``src-tauri/native/godly-shell/src/main.rs`` — App struct, render pipeline
- ``src-tauri/native/godly-shell/src/terminal_renderer.rs`` — terminal content

## What Needs to Change (Rendering Quality Roadmap)

### Phase 1: SDF Rounded Rectangle Shader
Replace the flat quad shader with a Signed Distance Field (SDF) based rounded rectangle shader.
The vertex/fragment shader should support:
- **Corner radius** (per-vertex or uniform) — smooth anti-aliased rounded corners
- **Border width + border color** — thin borders around elements
- **Anti-aliasing** via SDF smoothstep at the edges (1-2px feather)

The QuadVertex struct needs new fields: rect bounds (center + half-extents), corner_radius, border_width, border_color.
The fragment shader computes distance to the rounded rect and uses smoothstep for AA.

Reference approach (WGSL pseudocode):
``
fn sd_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2(radius);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}
// In fragment shader:
let dist = sd_rounded_rect(local_pos, half_extents, corner_radius);
let aa = 1.0 - smoothstep(-1.0, 1.0, dist);  // anti-aliased edge
let border_mask = smoothstep(-border_width - 1.0, -border_width + 1.0, dist);
let final_color = mix(border_color, fill_color, border_mask);
output = vec4(final_color.rgb, final_color.a * aa);
``

### Phase 2: Apply to UI Elements
Update sidebar, tab_bar, title_bar, status_bar to use the new SDF quad capabilities:
- Tabs get rounded top corners (radius ~4-6px)
- Sidebar items get subtle rounded rects for hover/active states
- Panel borders become part of the SDF quad (not separate quads)
- Active tab gets a subtle bottom highlight or distinct background

### Phase 3: Shadows and Depth
Add optional box shadow support to the SDF shader:
- Gaussian-approximated shadow via expanded SDF
- Very subtle — shadows should create depth, not be visually loud
- Used sparingly: dropdown menus, floating panels, modal dialogs

### Phase 4: Polish
- Gradient fills (top-to-bottom subtle gradients on title bar / tab bar)
- Smooth alpha blending for overlapping elements
- Consistent anti-aliasing quality across all UI chrome

## Your Task This Iteration

1. Read the reference image to keep the quality target fresh
2. Read the current quad_renderer.rs and other relevant UI files
3. Implement the NEXT phase of rendering quality improvements
4. Build: ``cd C:/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri && cargo build -p godly-shell 2>&1``
   If MSVC is not found, set up the environment first:
   ``export MSVC_VER="14.44.35207" && export VS_BASE="/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools" && export MSVC_BASE="`$VS_BASE/VC/Tools/MSVC/`$MSVC_VER" && export SDK_BASE="/c/Program Files (x86)/Windows Kits/10" && export SDK_VER="10.0.26100.0" && export PATH="`$MSVC_BASE/bin/Hostx64/x64:`$SDK_BASE/bin/`$SDK_VER/x64:`$PATH" && export LIB="`$MSVC_BASE/lib/x64;`$SDK_BASE/Lib/`$SDK_VER/ucrt/x64;`$SDK_BASE/Lib/`$SDK_VER/um/x64" && export INCLUDE="`$MSVC_BASE/include;`$SDK_BASE/Include/`$SDK_VER/ucrt;`$SDK_BASE/Include/`$SDK_VER/um;`$SDK_BASE/Include/`$SDK_VER/shared"``
   Then: ``cd /c/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri && cargo build -p godly-shell 2>&1``
5. Kill any running instance, launch, screenshot, and compare:
   ``cmd //C "taskkill /IM godly-native.exe /F" 2>/dev/null; cmd //C "taskkill /IM godly-daemon.exe /F" 2>/dev/null; cmd //C "taskkill /IM godly-pty-shim.exe /F" 2>/dev/null; sleep 2``
   ``C:/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri/target/debug/godly-native.exe &``
   ``sleep 5``
   Then use the chrome-devtools MCP or PyAutoGUI to take a screenshot. Then Read the screenshot file.
6. Compare your screenshot against the reference — focus on ELEMENT QUALITY not layout:
   - Are edges smooth or jagged?
   - Do elements have rounded corners?
   - Are borders visible and crisp?
   - Does the UI feel "solid" or like flat colored blocks?
7. Log progress in tasks/rendering-quality-iterations.md
8. Commit successful changes with descriptive message (feat: or fix: prefix)
9. Decide: if rendering quality of UI elements approaches the reference, say RALPH_DONE. Otherwise continue.

## Important Rules
- Focus ONLY on rendering quality. Do NOT rearrange layout, change dimensions, or restructure the UI.
- The SDF shader is the foundation — get it right first, then apply to individual elements.
- Always build and visually verify with a screenshot before declaring done.
- If a build fails, fix the compile error immediately.
- Commit working improvements frequently.
- The binary is godly-shell (package name), producing godly-native.exe.
- Alpha blending must be enabled in the pipeline for AA to work.
- Don't forget sRGB gamma correction — SDF anti-aliasing must happen in linear space.
- Study how Zed's GPUI renders rounded rects with SDF — it's open source at ``crates/gpui/src/``.

If you believe the rendering quality now approaches the reference closely enough, include the word RALPH_DONE in your response.
"@

    Set-Content -Path $promptFile -Value $content -Encoding UTF8
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Ralph Loop - Rendering Quality" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Focus:     ELEMENT RENDERING QUALITY (SDF, AA, borders, shadows)" -ForegroundColor Yellow
Write-Host "NOT:       layout, dimensions, placement" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Reference: $ReferenceImage"
Write-Host "Max iterations: $(if ($MaxIterations -eq 0) { 'unlimited' } else { $MaxIterations })"
Write-Host "Project:   $ProjectRoot"
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

# Cleanup
Remove-Item $promptFile -ErrorAction SilentlyContinue
