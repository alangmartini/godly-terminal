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

# ── Read version from package.json ───────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$packageJson = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$version = $packageJson.version

Write-Host "Godly Terminal build  v$version" -ForegroundColor Magenta

# ── Install npm dependencies ─────────────────────────────────────────────

Write-Step "Installing npm dependencies..."
Push-Location $repoRoot
npm install
Assert-ExitCode

# ── Unlock binaries ──────────────────────────────────────────────────────

Write-Step "Unlocking release binaries..."
npm run unlock -- --release
Assert-ExitCode

# ── Build Tauri production bundle ─────────────────────────────────────────

Write-Step "Building Tauri production bundle..."

npx tauri build
Assert-ExitCode

# ── Copy artifacts to installations/production/ ──────────────────────────

$bundleDir = Join-Path $repoRoot "src-tauri\target\release\bundle"
$outDir = Join-Path $repoRoot "installations\production"

if (-not (Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

Write-Step "Copying artifacts to installations\production\..."

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

Write-Host "`nProduction build complete. Artifacts in: $outDir" -ForegroundColor Green
