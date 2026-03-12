$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

function Assert-ExitCode {
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`nStaging build failed (exit code $LASTEXITCODE)." -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

# ── Read version from version.txt ──────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$version = (Get-Content (Join-Path $repoRoot "version.txt") -Raw).Trim()

Write-Host "Godly Terminal (Staging) build  v$version" -ForegroundColor Magenta

# ── Unlock binaries ────────────────────────────────────────────────────

Write-Step "Unlocking release binaries..."
Push-Location $repoRoot
node scripts/unlock-binaries.js --release
Assert-ExitCode

# ── Build all release binaries with staging feature ────────────────────

Write-Step "Building native staging binaries..."
Write-Host "   Features: staging (isolated pipes, metadata, app data)" -ForegroundColor DarkGray

$env:GODLY_INSTANCE = "staging"

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
    if ($crate -eq "godly-iced-shell") {
        cargo build --release -p $crate --features staging
    } else {
        cargo build --release -p $crate
    }
    Assert-ExitCode
}

Pop-Location
Remove-Item Env:\GODLY_INSTANCE -ErrorAction SilentlyContinue

# ── Copy artifacts to installations/staging/ ───────────────────────────

$targetDir = Join-Path $repoRoot "src-tauri\target\release"
$outDir = Join-Path $repoRoot "installations\staging"

if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

Write-Step "Copying binaries to installations\staging\..."

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

Write-Host "`nStaging build complete. Binaries in: $outDir" -ForegroundColor Green
Write-Host "Run 'pwsh scripts/staging-install.ps1' to install." -ForegroundColor DarkGray
