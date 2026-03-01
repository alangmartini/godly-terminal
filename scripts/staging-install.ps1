$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

# ── Read version from package.json ───────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$packageJson = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$version = $packageJson.version

Write-Host "Godly Terminal (Staging) installer  v$version" -ForegroundColor Magenta

# ── Locate staging installer ─────────────────────────────────────────────

$outDir = Join-Path $repoRoot "installations\staging"

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
    Write-Host "`nNo staging installer found in: $outDir" -ForegroundColor Red
    Write-Host "Run 'npm run staging:build' first." -ForegroundColor Yellow
    exit 1
}

Write-Ok "Found $installerType installer: $installerPath"

# ── Stop staging daemon (uses staging pipe name) ─────────────────────────
# The staging daemon uses a different pipe name, but the process name is
# still godly-daemon.exe. We can't kill by name without also killing
# production. Instead, check if a staging daemon is listening and send a
# shutdown request, or just let the installer handle it.

Write-Step "Note: Staging daemon (if running) uses isolated pipes."
Write-Host "   The installer will handle binary replacement via NSIS hooks." -ForegroundColor DarkGray

# ── Run the installer ────────────────────────────────────────────────────

Write-Step "Installing Godly Terminal (Staging) v$version ($installerType)..."

if ($installerType -eq "NSIS") {
    Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait
} else {
    Start-Process msiexec.exe -ArgumentList "/i", "`"$installerPath`"", "/quiet" -Wait
}

Write-Host "`nGodly Terminal (Staging) v$version installed." -ForegroundColor Green
Write-Host "It runs with isolated pipes (GODLY_INSTANCE=staging) and separate app data." -ForegroundColor DarkGray
