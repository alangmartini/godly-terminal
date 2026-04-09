#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Sets up a fresh Windows 11 VM for Godly Terminal development.
    Installs: Git, Python, Node.js, Rust, cargo-nextest, Claude Code, and clones + builds Godly Terminal.

.NOTES
    Run this in an elevated PowerShell inside the VM.
    Estimated time: 15-25 minutes depending on internet speed.
#>

$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────────

function Write-Step { param([string]$msg) Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok   { param([string]$msg) Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn { param([string]$msg) Write-Host "    WARN: $msg" -ForegroundColor Yellow }

function Refresh-Path {
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPath    = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path    = "$machinePath;$userPath"
}

function Install-Winget-Package {
    param([string]$PackageId, [string]$Name)
    Write-Step "Installing $Name..."
    $installed = winget list --id $PackageId 2>$null | Select-String $PackageId
    if ($installed) {
        Write-Ok "$Name is already installed"
    } else {
        winget install --id $PackageId --accept-source-agreements --accept-package-agreements
        if ($LASTEXITCODE -ne 0) { throw "Failed to install $Name" }
        Write-Ok "$Name installed"
    }
    Refresh-Path
}

# ── Pre-flight ───────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "=========================================" -ForegroundColor Magenta
Write-Host "  Godly Terminal Dev VM Setup" -ForegroundColor Magenta
Write-Host "=========================================" -ForegroundColor Magenta
Write-Host ""

# Ensure winget is available (should be on Win11 by default)
Write-Step "Checking winget..."
if (!(Get-Command winget -ErrorAction SilentlyContinue)) {
    throw "winget not found. Please install App Installer from the Microsoft Store."
}
Write-Ok "winget is available"

# ── 1. Git ───────────────────────────────────────────────────────────────────

Install-Winget-Package "Git.Git" "Git"

# Verify git
Refresh-Path
if (!(Get-Command git -ErrorAction SilentlyContinue)) {
    # Git installs to a non-standard path, add it manually
    $gitPath = "C:\Program Files\Git\cmd"
    if (Test-Path $gitPath) {
        $env:Path += ";$gitPath"
    }
}
git --version | Write-Host

# ── 2. Python ────────────────────────────────────────────────────────────────

Install-Winget-Package "Python.Python.3.12" "Python 3.12"

Refresh-Path
python --version | Write-Host

# ── 3. Node.js (required for Claude Code) ────────────────────────────────────

Install-Winget-Package "OpenJS.NodeJS.LTS" "Node.js LTS"

Refresh-Path
node --version | Write-Host
npm --version | Write-Host

# ── 4. Rust ──────────────────────────────────────────────────────────────────

Write-Step "Installing Rust..."
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Ok "Rust is already installed"
    rustc --version | Write-Host
} else {
    # Download and run rustup-init silently
    $rustupUrl = "https://win.rustup.rs/x86_64"
    $rustupPath = "$env:TEMP\rustup-init.exe"
    Write-Host "    Downloading rustup..."
    Invoke-WebRequest -Uri $rustupUrl -UseBasicParsing
    Write-Host "    Running rustup-init (this takes a minute)..."
    & $rustupPath -y --default-toolchain stable 2>&1

    # Add cargo to path
    $cargoPath = "$env:USERPROFILE\.cargo\bin"
    $env:Path += ";$cargoPath"
    [Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";$cargoPath", "User")

    Refresh-Path
    rustc --version | Write-Host
    Write-Ok "Rust installed"
}

# ── 5. cargo-nextest ─────────────────────────────────────────────────────────

Write-Step "Installing cargo-nextest..."
if (Get-Command cargo-nextest -ErrorAction SilentlyContinue) {
    Write-Ok "cargo-nextest already installed"
} else {
    cargo install cargo-nextest
    Write-Ok "cargo-nextest installed"
}

# ── 6. Visual Studio Build Tools (Rust needs MSVC linker) ────────────────────

Write-Step "Checking for MSVC build tools..."
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasBuildTools = $false

if (Test-Path $vsWhere) {
    $instances = & $vsWhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json 2>$null | ConvertFrom-Json
    if ($instances.Count -gt 0) { $hasBuildTools = $true }
}

if ($hasBuildTools) {
    Write-Ok "MSVC build tools already available"
} else {
    Write-Host "    Downloading Visual Studio Build Tools..."
    $vsUrl = "https://aka.ms/vs/17/release/vs_BuildTools.exe"
    $vsPath = "$env:TEMP\vs_BuildTools.exe"
    Invoke-WebRequest -Uri $vsUrl -OutFile $vsPath -UseBasicParsing

    Write-Host "    Installing Build Tools (this takes 5-10 minutes)..."
    Start-Process -FilePath $vsPath -ArgumentList `
        "--quiet", "--wait", "--norestart", `
        "--add", "Microsoft.VisualStudio.Workload.VCTools", `
        "--add", "Microsoft.VisualStudio.Component.Windows11SDK.22621", `
        "--includeRecommended" `
        -Wait -NoNewWindow

    Write-Ok "Visual Studio Build Tools installed"
}

# ── 7. Claude Code ───────────────────────────────────────────────────────────

Write-Step "Installing Claude Code..."
npm install -g @anthropic-ai/claude-code 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Warn "Claude Code install returned non-zero, but may still work"
}
Refresh-Path

if (Get-Command claude -ErrorAction SilentlyContinue) {
    Write-Ok "Claude Code installed"
    claude --version 2>$null | Write-Host
} else {
    Write-Warn "Claude Code binary not found in PATH — you may need to restart the terminal"
}

# ── 8. Clone and build Godly Terminal ────────────────────────────────────────

$repoUrl  = "https://github.com/alangmartini/godly-terminal.git"
$repoDir  = "$env:USERPROFILE\dev\godly-terminal"

Write-Step "Cloning Godly Terminal..."
if (Test-Path "$repoDir\.git") {
    Write-Ok "Already cloned at $repoDir, pulling latest..."
    Push-Location $repoDir
    git pull
    Pop-Location
} else {
    New-Item -ItemType Directory -Path (Split-Path $repoDir) -Force | Out-Null
    git clone $repoUrl $repoDir
    Write-Ok "Cloned to $repoDir"
}

Write-Step "Building Godly Terminal (debug build)..."
Push-Location "$repoDir\src-tauri"
cargo build -p godly-daemon -p godly-iced-shell 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Ok "Build succeeded!"
} else {
    Write-Warn "Build failed — this might be due to missing dependencies. Check the output above."
}
Pop-Location

# ── 9. Enable RDP (in case it's not already on) ─────────────────────────────

Write-Step "Ensuring RDP is enabled..."
Set-ItemProperty -Path 'HKLM:\System\CurrentControlSet\Control\Terminal Server' -Name "fDenyTSConnections" -Value 0
Enable-NetFirewallRule -DisplayGroup "Remote Desktop" -ErrorAction SilentlyContinue
Write-Ok "RDP enabled"

# ── Summary ──────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "=========================================" -ForegroundColor Green
Write-Host "  Setup Complete!" -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Installed:" -ForegroundColor White
Write-Host "  - Git:            $(git --version 2>$null)"
Write-Host "  - Python:         $(python --version 2>$null)"
Write-Host "  - Node.js:        $(node --version 2>$null)"
Write-Host "  - Rust:           $(rustc --version 2>$null)"
Write-Host "  - cargo-nextest:  installed"
Write-Host "  - Claude Code:    $(claude --version 2>$null)"
Write-Host "  - MSVC Build Tools"
Write-Host ""
Write-Host "Godly Terminal cloned to: $repoDir" -ForegroundColor White
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Open a NEW terminal (to pick up PATH changes)"
Write-Host "  2. Run 'claude' to authenticate Claude Code"
Write-Host "  3. cd $repoDir\src-tauri"
Write-Host "  4. cargo run -p godly-iced-shell"
Write-Host ""