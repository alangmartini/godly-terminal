$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

# ── Read version from package.json ───────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$packageJson = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$version = $packageJson.version

Write-Host "Godly Terminal installer  v$version" -ForegroundColor Magenta

# ── Locate production installer ──────────────────────────────────────────

$outDir = Join-Path $repoRoot "installations\production"

$nsisExe = Get-ChildItem "$outDir\*setup*.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

$msiFile = Get-ChildItem "$outDir\*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($nsisExe) {
    $installerPath = $nsisExe.FullName
    $installerType = "NSIS"
} elseif ($msiFile) {
    $installerPath = $msiFile.FullName
    $installerType = "MSI"
} else {
    Write-Host "`nNo production installer found in: $outDir" -ForegroundColor Red
    Write-Host "Run 'scripts/production-build.ps1' first." -ForegroundColor Yellow
    exit 1
}

Write-Ok "Found $installerType installer: $installerPath"

# ── Run the installer (silent) ───────────────────────────────────────────

Write-Step "Installing Godly Terminal v$version ($installerType) silently..."

if ($installerType -eq "NSIS") {
    Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait
} else {
    Start-Process msiexec.exe -ArgumentList "/i", "`"$installerPath`"", "/quiet" -Wait
}

Write-Host "`nGodly Terminal v$version installed." -ForegroundColor Green
