$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

# ── Read version from package.json ───────────────────────────────────────

$packageJson = Get-Content (Join-Path $PSScriptRoot "package.json") -Raw | ConvertFrom-Json
$version = $packageJson.version
$productName = "Godly Terminal"

Write-Host "Godly Terminal installer  v$version" -ForegroundColor Cyan

# ── Locate installer (prefer NSIS .exe, fall back to MSI) ────────────────

$bundleDir = Join-Path $PSScriptRoot "src-tauri\target\release\bundle"

# Find the most recent NSIS setup exe matching current version
$nsisExe = Get-ChildItem "$bundleDir\nsis\*${version}*setup.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

# Find the most recent MSI matching current version
$msiFile = Get-ChildItem "$bundleDir\msi\*${version}*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($nsisExe) {
    $installerPath = $nsisExe.FullName
    $installerType = "NSIS"
} elseif ($msiFile) {
    $installerPath = $msiFile.FullName
    $installerType = "MSI"
} else {
    Write-Host "`nNo installer found for v$version in:" -ForegroundColor Red
    Write-Host "  $bundleDir\nsis\" -ForegroundColor Yellow
    Write-Host "  $bundleDir\msi\" -ForegroundColor Yellow
    Write-Host "`nRun .\production_build.ps1 first to build the installer." -ForegroundColor Yellow
    exit 1
}

Write-Ok "Found $installerType installer: $installerPath"

# ── Stop running daemon processes ────────────────────────────────────────

Write-Step "Stopping running godly-daemon instances..."
$daemon = Get-Process -Name "godly-daemon" -ErrorAction SilentlyContinue
if ($daemon) {
    $daemon | Stop-Process -Force
    Start-Sleep -Seconds 1
    Write-Ok "Stopped $($daemon.Count) daemon process(es)"
} else {
    Write-Ok "No running daemon found"
}

# ── Run the installer ────────────────────────────────────────────────────

Write-Step "Installing $productName v$version ($installerType)..."

if ($installerType -eq "NSIS") {
    # NSIS exe installer — run directly
    Start-Process -FilePath $installerPath -Wait
} else {
    # MSI installer — use msiexec
    Start-Process msiexec.exe -ArgumentList "/i", "`"$installerPath`"" -Wait
}

Write-Host "`n$productName v$version installed." -ForegroundColor Green
