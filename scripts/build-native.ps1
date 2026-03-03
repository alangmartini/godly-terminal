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

# Build the native binary
$buildArgs = @("build", "-p", "godly-iced-shell")
if ($profileArg) { $buildArgs += $profileArg }

Write-Host "Running: cargo $($buildArgs -join ' ')" -ForegroundColor Gray
Push-Location "$PSScriptRoot\..\src-tauri"
try {
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

$binaryPath = "$PSScriptRoot\..\src-tauri\target\$profileName\godly-native.exe"
if (Test-Path $binaryPath) {
    $size = (Get-Item $binaryPath).Length / 1MB
    Write-Host "`nBuild complete: $binaryPath ($([math]::Round($size, 1)) MB)" -ForegroundColor Green
} else {
    Write-Host "`nBuild complete (binary at: src-tauri/target/$profileName/godly-native)" -ForegroundColor Green
}
