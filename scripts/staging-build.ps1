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

# ── Read version from package.json ───────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$packageJson = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$version = $packageJson.version

Write-Host "Godly Terminal (Staging) build  v$version" -ForegroundColor Magenta

# ── Install dependencies ──────────────────────────────────────────────────

Write-Step "Installing dependencies..."
Push-Location $repoRoot
pnpm install
Assert-ExitCode

# ── Unlock binaries ──────────────────────────────────────────────────────

Write-Step "Unlocking release binaries..."
pnpm run unlock -- --release
Assert-ExitCode

# ── Build Tauri staging bundle ───────────────────────────────────────────
# Uses --features staging to bake GODLY_INSTANCE=staging into the binary,
# and --config to override identifier/productName/title for full isolation.

Write-Step "Building Tauri staging bundle..."
Write-Host "   Features: staging (isolated pipes, metadata, app data)" -ForegroundColor DarkGray
Write-Host "   Config:   tauri.conf.staging.json (separate identity)" -ForegroundColor DarkGray

$env:GODLY_INSTANCE = "staging"
pnpm exec tauri build --features staging --config src-tauri/tauri.conf.staging.json
Assert-ExitCode
Remove-Item Env:\GODLY_INSTANCE -ErrorAction SilentlyContinue

# ── Copy artifacts to installations/staging/ ─────────────────────────────

$bundleDir = Join-Path $repoRoot "src-tauri\target\release\bundle"
$outDir = Join-Path $repoRoot "installations\staging"

if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

Write-Step "Copying artifacts to installations\staging\..."

$nsisExe = Get-ChildItem "$bundleDir\nsis\*.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
$msiFile = Get-ChildItem "$bundleDir\msi\*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($nsisExe) {
    Copy-Item $nsisExe.FullName $outDir -Force
    Write-Ok "NSIS: $($nsisExe.Name)  ($([math]::Round($nsisExe.Length / 1MB, 1)) MB)"
}
if ($msiFile) {
    Copy-Item $msiFile.FullName $outDir -Force
    Write-Ok "MSI:  $($msiFile.Name)  ($([math]::Round($msiFile.Length / 1MB, 1)) MB)"
}

if (-not $nsisExe -and -not $msiFile) {
    Write-Host "   No installers found in $bundleDir" -ForegroundColor Yellow
}

Pop-Location

Write-Host "`nStaging build complete. Artifacts in: $outDir" -ForegroundColor Green
