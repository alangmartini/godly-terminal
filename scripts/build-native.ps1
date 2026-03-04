# Build the native Iced+wgpu frontend binary.
# Usage:
#   pwsh scripts/build-native.ps1            # debug build
#   pwsh scripts/build-native.ps1 --release  # release build

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"

Write-Host "=== Building Godly Terminal (Native) ===" -ForegroundColor Cyan

$profileArg = if ($Release) { "--release" } else { $null }
$profileName = if ($Release) { "release" } else { "debug" }

Push-Location "$PSScriptRoot\..\src-tauri"
try {
    # Build the daemon first
    $daemonArgs = @("build", "-p", "godly-daemon")
    if ($profileArg) { $daemonArgs += $profileArg }

    Write-Host "Running: cargo $($daemonArgs -join ' ')" -ForegroundColor Gray
    & cargo @daemonArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Daemon build failed with exit code $LASTEXITCODE"
    }

    # Build the native shell
    $buildArgs = @("build", "-p", "godly-iced-shell")
    if ($profileArg) { $buildArgs += $profileArg }

    Write-Host "Running: cargo $($buildArgs -join ' ')" -ForegroundColor Gray
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Native shell build failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

$targetDir = "$PSScriptRoot\..\src-tauri\target\$profileName"
Write-Host "`nBuild complete:" -ForegroundColor Green
foreach ($bin in @("godly-native.exe", "godly-daemon.exe")) {
    $path = Join-Path $targetDir $bin
    if (Test-Path $path) {
        $size = (Get-Item $path).Length / 1MB
        Write-Host "  $bin ($([math]::Round($size, 1)) MB)" -ForegroundColor Green
    }
}
