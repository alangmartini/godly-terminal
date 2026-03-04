# Build the native Godly Terminal for production distribution.
# Produces: godly-native.exe + godly-daemon.exe (both release builds)
param([switch]$SkipDaemon)

$ErrorActionPreference = "Stop"
Write-Host "=== Production Build: Godly Terminal (Native) ===" -ForegroundColor Cyan

Push-Location "$PSScriptRoot\..\src-tauri"
try {
    if (-not $SkipDaemon) {
        Write-Host "Building daemon (release)..." -ForegroundColor Gray
        cargo build -p godly-daemon --release
        if ($LASTEXITCODE -ne 0) { throw "Daemon build failed" }
    }

    Write-Host "Building native shell (release)..." -ForegroundColor Gray
    cargo build -p godly-iced-shell --release
    if ($LASTEXITCODE -ne 0) { throw "Native shell build failed" }
} finally {
    Pop-Location
}

$targetDir = "$PSScriptRoot\..\src-tauri\target\release"
Write-Host "`nBuild complete:" -ForegroundColor Green
foreach ($bin in @("godly-native.exe", "godly-daemon.exe")) {
    $path = Join-Path $targetDir $bin
    if (Test-Path $path) {
        $size = (Get-Item $path).Length / 1MB
        Write-Host "  $bin ($([math]::Round($size, 1)) MB)" -ForegroundColor Green
    }
}
