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
    "godly-iced-shell"
)

foreach ($crate in $crates) {
    Write-Host "   Building $crate..." -ForegroundColor DarkGray
    cargo build --release -p $crate
    Assert-ExitCode
}

Pop-Location

# ── Copy artifacts to installations/production/ ────────────────────────

$targetDir = Join-Path $repoRoot "src-tauri\target\release"
$outDir = Join-Path $repoRoot "installations\production"

if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

Write-Step "Copying binaries to installations\production\..."

$binaries = @("godly-native.exe", "godly-daemon.exe", "godly-mcp.exe", "godly-notify.exe", "godly-pty-shim.exe", "godly-remote.exe")

foreach ($bin in $binaries) {
    $src = Join-Path $targetDir $bin
    if (Test-Path $src) {
        Copy-Item $src $outDir -Force
        $size = [math]::Round((Get-Item $src).Length / 1MB, 1)
        Write-Ok "$bin ($size MB)"
    } else {
        Write-Host "   $bin not found (skipped)" -ForegroundColor Yellow
    }
}

Pop-Location

Write-Host "`nProduction build complete. Binaries in: $outDir" -ForegroundColor Green
