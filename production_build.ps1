param(
    [switch]$Test  # Pass -Test to run full test suite before building
)

$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }
function Write-Skip($msg) { Write-Host "   $msg" -ForegroundColor DarkGray }

function Assert-ExitCode {
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`nBuild failed (exit code $LASTEXITCODE)." -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

# ── Switch to master and pull latest ─────────────────────────────────────

Write-Step "Switching to master..."
git checkout master
Assert-ExitCode

Write-Step "Pulling latest changes..."
git pull origin master
Assert-ExitCode

# ── Install npm dependencies ─────────────────────────────────────────────

Write-Step "Installing npm dependencies..."
npm install
Assert-ExitCode

# ── Unlock binaries (rename locked .exe files so cargo can overwrite) ────

Write-Step "Unlocking release binaries..."
npm run unlock -- --release
Assert-ExitCode

# ── Run tests (only with -Test flag; CI covers this on every push) ───────

if ($Test) {
    Write-Step "Running frontend tests..."
    npm test
    Assert-ExitCode

    Write-Step "Running Rust tests..."
    Push-Location src-tauri
    cargo test -p godly-protocol
    Assert-ExitCode
    cargo test -p godly-vt
    Assert-ExitCode
    # Daemon tests must run single-threaded: integration tests spawn daemon + PTY
    # processes, and parallel spawning triggers Windows 0xc0000142 (DLL init failure)
    cargo test -p godly-daemon -- --test-threads=1
    Assert-ExitCode
    Pop-Location
} else {
    Write-Step "Skipping tests (CI runs them on push). Use -Test to run locally."
}

# ── Build Tauri app (daemon + MCP + notify + frontend + bundle) ──────────

Write-Step "Building Tauri production bundle..."
Write-Host "   This builds: godly-daemon.exe, godly-mcp.exe, godly-notify.exe," -ForegroundColor DarkGray
Write-Host "   the frontend (tsc + vite), and packages NSIS + MSI installers." -ForegroundColor DarkGray
npm run tauri build
Assert-ExitCode

# ── Report output artifacts ──────────────────────────────────────────────

Write-Step "Build artifacts:"

$bundleDir = Join-Path $PSScriptRoot "src-tauri\target\release\bundle"
$nsisExe   = Get-ChildItem "$bundleDir\nsis\*.exe" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$msiFile   = Get-ChildItem "$bundleDir\msi\*.msi"  -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($nsisExe) { Write-Ok "NSIS: $($nsisExe.FullName)  ($([math]::Round($nsisExe.Length / 1MB, 1)) MB)" }
if ($msiFile) { Write-Ok "MSI:  $($msiFile.FullName)  ($([math]::Round($msiFile.Length / 1MB, 1)) MB)" }

if (-not $nsisExe -and -not $msiFile) {
    Write-Host "   No installers found in $bundleDir" -ForegroundColor Yellow
}

Write-Host "`nProduction build complete." -ForegroundColor Green
