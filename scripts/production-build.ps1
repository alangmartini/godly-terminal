$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

function Assert-ExitCode {
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`nProduction build failed (exit code $LASTEXITCODE)." -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

# ── Read version from version.txt ──────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$version = (Get-Content (Join-Path $repoRoot "version.txt") -Raw).Trim()

Write-Host "Godly Terminal build  v$version" -ForegroundColor Magenta

# ── Unlock binaries ────────────────────────────────────────────────────

Write-Step "Unlocking release binaries..."
Push-Location $repoRoot
node scripts/unlock-binaries.js --release
Assert-ExitCode

# ── Build all release binaries ─────────────────────────────────────────

Write-Step "Building native release binaries..."

Push-Location (Join-Path $repoRoot "src-tauri")

$crates = @(
    "godly-daemon",
    "godly-pty-shim",
    "godly-mcp",
    "godly-notify",
    "godly-remote",
    "godly-iced-shell",
    "godly-tmux-shim"
)

foreach ($crate in $crates) {
    Write-Host "   Building $crate..." -ForegroundColor DarkGray
    cargo build --release -p $crate
    Assert-ExitCode
}

Pop-Location

# ── Generate MSI installer ─────────────────────────────────────────────

Write-Step "Generating MSI installer..."

$outDir = Join-Path $repoRoot "installations\production"
if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

# Locate WiX tools: WIX env var, then ~/.wix/, then PATH
$wixBin = if ($env:WIX) { Join-Path $env:WIX "bin" }
           elseif (Test-Path "$env:USERPROFILE\.wix\candle.exe") { "$env:USERPROFILE\.wix" }
           else { $null }

$candle = if ($wixBin) { Join-Path $wixBin "candle.exe" } else { "candle.exe" }
$light  = if ($wixBin) { Join-Path $wixBin "light.exe" }  else { "light.exe" }

if (-not (Get-Command $candle -ErrorAction SilentlyContinue)) {
    Write-Host "`nWiX Toolset not found. Install WiX 3.x or download binaries to ~/.wix/" -ForegroundColor Red
    Write-Host "   curl -sL https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip -o wix3.zip" -ForegroundColor Yellow
    Write-Host "   unzip wix3.zip -d `$env:USERPROFILE\.wix" -ForegroundColor Yellow
    exit 1
}

$wixObj = Join-Path $outDir "main.wixobj"
$msiPath = Join-Path $outDir "Godly Terminal_${version}_x64.msi"

Write-Host "   Compiling WiX manifest..." -ForegroundColor DarkGray
& $candle "-dVersion=$version" -o $wixObj (Join-Path $repoRoot "wix\main.wxs")
Assert-ExitCode

Write-Host "   Linking MSI..." -ForegroundColor DarkGray
& $light -sice:ICE38 -sice:ICE64 -sice:ICE91 -o $msiPath $wixObj
Assert-ExitCode

# Clean up intermediate files
Remove-Item $wixObj -ErrorAction SilentlyContinue
Remove-Item (Join-Path $outDir "*.wixpdb") -ErrorAction SilentlyContinue

$msiFile = Get-Item $msiPath -ErrorAction SilentlyContinue
if ($msiFile) {
    $size = [math]::Round($msiFile.Length / 1MB, 1)
    Write-Ok "MSI: $($msiFile.Name) ($size MB)"
} else {
    Write-Host "   MSI generation failed — check output above" -ForegroundColor Yellow
}

Pop-Location

Write-Host "`nProduction build complete. Installer in: $outDir" -ForegroundColor Green
