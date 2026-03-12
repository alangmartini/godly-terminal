$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

# ── Read version from version.txt ──────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$version = (Get-Content (Join-Path $repoRoot "version.txt") -Raw).Trim()

Write-Host "Godly Terminal (Staging) installer  v$version" -ForegroundColor Magenta

# ── Locate staging MSI ─────────────────────────────────────────────────

$outDir = Join-Path $repoRoot "installations\staging"

$msiFile = Get-ChildItem "$outDir\*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if (-not $msiFile) {
    Write-Host "`nNo staging MSI found in: $outDir" -ForegroundColor Red
    Write-Host "Run 'pwsh scripts/staging-build.ps1' first." -ForegroundColor Yellow
    exit 1
}

Write-Ok "Found MSI: $($msiFile.Name)"

# ── Run the installer (silent) ─────────────────────────────────────────

Write-Step "Installing Godly Terminal (Staging) v$version..."

Start-Process msiexec.exe -ArgumentList "/i", "`"$($msiFile.FullName)`"", "/quiet" -Wait

Write-Host "`nGodly Terminal (Staging) v$version installed." -ForegroundColor Green
Write-Host "Launch from Start Menu or: `"$env:LOCALAPPDATA\Godly Terminal (Staging)\godly-native.exe`"" -ForegroundColor DarkGray
