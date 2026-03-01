# Build the standalone Godly Whisper NSIS installer (CPU-only).
# Output: installations/whisper/godly-whisper-setup.exe
#
# Prerequisites: cargo, makensis (NSIS) in PATH
# Usage: pwsh scripts/build-whisper-installer.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
Set-Location $root

Write-Host "=== Building godly-whisper (CPU-only, release) ===" -ForegroundColor Cyan
Push-Location src-tauri
cargo build -p godly-whisper --release
Pop-Location

$binary = "src-tauri\target\release\godly-whisper.exe"
if (-not (Test-Path $binary)) {
    Write-Error "Build failed: $binary not found"
    exit 1
}

# Create staging directory for NSIS
$staging = "installers\whisper\staging"
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Copy-Item $binary "$staging\godly-whisper.exe" -Force

# Generate version.json from the binary
$versionJson = & "$staging\godly-whisper.exe" --version 2>$null
if ($LASTEXITCODE -eq 0 -and $versionJson) {
    Set-Content "$staging\version.json" $versionJson
    Write-Host "version.json: $versionJson"
} else {
    Write-Warning "Could not generate version.json"
    Set-Content "$staging\version.json" '{"version":"unknown","build":0,"cuda":false}'
}

# Create output directory
New-Item -ItemType Directory -Force -Path "installations\whisper" | Out-Null

# Build NSIS installer
Write-Host "=== Running makensis ===" -ForegroundColor Cyan
makensis "installers\whisper\whisper-installer.nsi"

if ($LASTEXITCODE -ne 0) {
    Write-Error "makensis failed"
    exit 1
}

# Cleanup staging
Remove-Item -Recurse -Force $staging

Write-Host "`n=== Done ===" -ForegroundColor Green
Write-Host "Installer: installations\whisper\godly-whisper-setup.exe"
