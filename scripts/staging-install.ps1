$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

# ── Read version from version.txt ──────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$version = (Get-Content (Join-Path $repoRoot "version.txt") -Raw).Trim()

Write-Host "Godly Terminal (Staging) installer  v$version" -ForegroundColor Magenta

# ── Locate staging binaries ────────────────────────────────────────────

$srcDir = Join-Path $repoRoot "installations\staging"

if (-not (Test-Path (Join-Path $srcDir "godly-native.exe"))) {
    Write-Host "`nNo staging binaries found in: $srcDir" -ForegroundColor Red
    Write-Host "Run 'pwsh scripts/staging-build.ps1' first." -ForegroundColor Yellow
    exit 1
}

# ── Install to %LOCALAPPDATA%/godly-terminal-staging/ ──────────────────

$installDir = Join-Path $env:LOCALAPPDATA "godly-terminal-staging"

if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

Write-Step "Installing to $installDir..."

$binaries = @("godly-native.exe", "godly-daemon.exe", "godly-mcp.exe", "godly-notify.exe", "godly-pty-shim.exe", "godly-remote.exe")

foreach ($bin in $binaries) {
    $src = Join-Path $srcDir $bin
    if (Test-Path $src) {
        $dst = Join-Path $installDir $bin
        # Rename locked binary if in use
        if (Test-Path $dst) {
            $old = "$dst.old"
            if (Test-Path $old) { Remove-Item $old -Force -ErrorAction SilentlyContinue }
            try {
                Rename-Item $dst $old -Force -ErrorAction Stop
            } catch {
                Write-Host "   $bin is locked — kill the process first" -ForegroundColor Yellow
                continue
            }
        }
        Copy-Item $src $dst -Force
        Write-Ok $bin
    }
}

# ── Copy sound assets if present ───────────────────────────────────────

$soundsSrc = Join-Path $repoRoot "sounds"
if (Test-Path $soundsSrc) {
    $soundsDst = Join-Path $installDir "sounds"
    if (-not (Test-Path $soundsDst)) {
        New-Item -ItemType Directory -Path $soundsDst -Force | Out-Null
    }
    Copy-Item "$soundsSrc\*" $soundsDst -Force -Recurse
    Write-Ok "sounds/"
}

Write-Host "`nGodly Terminal (Staging) v$version installed to: $installDir" -ForegroundColor Green
Write-Host "Run with: GODLY_INSTANCE=staging $installDir\godly-native.exe" -ForegroundColor DarkGray
