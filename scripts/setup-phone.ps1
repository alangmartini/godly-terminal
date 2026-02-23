# Automated phone setup: starts godly-remote + Cloudflare Tunnel + displays QR code
# Usage: pwsh scripts/setup-phone.ps1 [-Port <port>]
#
# Prerequisites:
#   winget install Cloudflare.cloudflared
#   cloudflared tunnel login
#   cloudflared tunnel create godly-phone
#   cloudflared tunnel route dns godly-phone phone.godlybr.com

param(
    [int]$Port = 3377,
    [string]$TunnelName = "godly-phone",
    [string]$Hostname = "phone.godlybr.com"
)

$ErrorActionPreference = "Stop"

# --- Find cloudflared ---
$cfBin = (Get-Command cloudflared -ErrorAction SilentlyContinue).Source
if (-not $cfBin) {
    $candidate = "${env:ProgramFiles(x86)}\cloudflared\cloudflared.exe"
    if (Test-Path $candidate) { $cfBin = $candidate }
}
if (-not $cfBin) {
    $candidate = "$env:ProgramFiles\cloudflared\cloudflared.exe"
    if (Test-Path $candidate) { $cfBin = $candidate }
}
if (-not $cfBin) {
    Write-Host "cloudflared not found." -ForegroundColor Red
    Write-Host ""
    Write-Host "Install it with:" -ForegroundColor Yellow
    Write-Host "  winget install Cloudflare.cloudflared" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Then authenticate and create tunnel:" -ForegroundColor Yellow
    Write-Host "  cloudflared tunnel login" -ForegroundColor Cyan
    Write-Host "  cloudflared tunnel create godly-phone" -ForegroundColor Cyan
    Write-Host "  cloudflared tunnel route dns godly-phone phone.godlybr.com" -ForegroundColor Cyan
    exit 1
}

# --- Check godly-remote binary ---
$scriptDir = $PSScriptRoot
$repoRoot = (Resolve-Path "$scriptDir\..").Path
$remoteBin = "$repoRoot\src-tauri\target\release\godly-remote.exe"
if (-not (Test-Path $remoteBin)) {
    $remoteBin = "$repoRoot\src-tauri\target\debug\godly-remote.exe"
}
if (-not (Test-Path $remoteBin)) {
    Write-Host "godly-remote.exe not found. Building..." -ForegroundColor Yellow
    Push-Location "$repoRoot\src-tauri"
    cargo build -p godly-remote --release
    Pop-Location
    $remoteBin = "$repoRoot\src-tauri\target\release\godly-remote.exe"
    if (-not (Test-Path $remoteBin)) {
        Write-Host "Build failed. Cannot find godly-remote.exe" -ForegroundColor Red
        exit 1
    }
}

# --- Read persisted settings (if any) ---
$configPath = Join-Path $env:APPDATA "com.godly.terminal\remote-config.json"
$savedConfig = $null
if (Test-Path $configPath) {
    try {
        $savedConfig = Get-Content $configPath -Raw | ConvertFrom-Json
        Write-Host "Using saved settings from $configPath" -ForegroundColor Green
    } catch {
        Write-Host "Could not parse saved config, using defaults" -ForegroundColor Yellow
    }
}

# --- Use saved or generate API key and password ---
# Match the charset and length from the Settings UI (remote-settings-store.ts)
# API key: 24 chars, alphanumeric (A-Z, a-z, 0-9)
# Password: 100 chars, alphanumeric + special (!@#$%^&*()-_=+[]{}|;:,.<>?)
$alphanumeric = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
$passwordCharset = $alphanumeric + '!@#$%^&*()-_=+[]{}|;:,.<>?'

function New-SecureString([string]$Charset, [int]$Length) {
    $bytes = [byte[]]::new($Length)
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    -join ($bytes | ForEach-Object { $Charset[$_ % $Charset.Length] })
}

if ($savedConfig -and $savedConfig.api_key) {
    $ApiKey = $savedConfig.api_key
    Write-Host "  API Key: (from saved settings)" -ForegroundColor DarkGray
} else {
    $ApiKey = New-SecureString $alphanumeric 24
}

if ($savedConfig -and $savedConfig.password) {
    $Password = $savedConfig.password
    Write-Host "  Password: (from saved settings)" -ForegroundColor DarkGray
} else {
    $Password = New-SecureString $passwordCharset 100
}

# Override port from saved settings if not explicitly provided via CLI
if ($savedConfig -and $savedConfig.port -and $Port -eq 3377) {
    $Port = $savedConfig.port
}

# --- Save settings for next run ---
$configDir = Split-Path $configPath
if (-not (Test-Path $configDir)) { New-Item -ItemType Directory -Path $configDir -Force | Out-Null }
@{
    api_key  = $ApiKey
    password = $Password
    port     = $Port
    tunnel   = $TunnelName
    hostname = $Hostname
} | ConvertTo-Json | Set-Content $configPath

# --- Kill any existing godly-remote so we start fresh with our API key + password ---
$remoteProc = $null
$existingPids = Get-Process -Name "godly-remote" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id
if ($existingPids) {
    Write-Host ""
    Write-Host "Stopping existing godly-remote (PID: $($existingPids -join ', '))..." -ForegroundColor Yellow
    foreach ($p in $existingPids) {
        Stop-Process -Id $p -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 500
}

# --- Start godly-remote ---
Write-Host ""
Write-Host "Starting godly-remote on port $Port..." -ForegroundColor Green
$env:GODLY_REMOTE_HOST = "127.0.0.1"  # Cloudflare Tunnel connects locally — no need to bind 0.0.0.0
$env:GODLY_REMOTE_PORT = $Port
$env:GODLY_REMOTE_API_KEY = $ApiKey
$env:GODLY_REMOTE_PASSWORD = $Password
$remoteProc = Start-Process -FilePath $remoteBin -PassThru -NoNewWindow
Start-Sleep -Seconds 1

if ($remoteProc.HasExited) {
    Write-Host "godly-remote failed to start. Is the daemon running?" -ForegroundColor Red
    Write-Host "Start Godly Terminal first, or run: src-tauri\target\release\godly-daemon.exe" -ForegroundColor Yellow
    exit 1
}

# --- Start Cloudflare Tunnel ---
Write-Host "Starting Cloudflare Tunnel ($TunnelName -> $Hostname)..." -ForegroundColor Green
$cfLog = Join-Path $env:TEMP "godly-cloudflared.log"
$cfProc = Start-Process -FilePath $cfBin -ArgumentList "tunnel run $TunnelName" -PassThru -WindowStyle Hidden -RedirectStandardOutput $cfLog -RedirectStandardError (Join-Path $env:TEMP "godly-cloudflared-err.log")

# --- Wait for tunnel to be ready ---
# cloudflared doesn't have a local API — just verify it stays alive for a few seconds
Write-Host "  Waiting for tunnel connection..." -ForegroundColor DarkGray
Start-Sleep -Seconds 3
if ($cfProc.HasExited) {
    Write-Host "cloudflared exited unexpectedly (exit code: $($cfProc.ExitCode))." -ForegroundColor Red
    $errLog = Join-Path $env:TEMP "godly-cloudflared-err.log"
    if (Test-Path $errLog) {
        $logTail = Get-Content $errLog -Tail 15
        if ($logTail) {
            Write-Host ""
            $logTail | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
        }
    }
    Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "  Tunnel connected." -ForegroundColor Green

# --- Build phone URL ---
$publicUrl = "https://$Hostname"
if ($ApiKey) {
    $phoneUrl = "$publicUrl/phone#key=$ApiKey"
} else {
    $phoneUrl = "$publicUrl/phone"
}

# --- Display QR code ---
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Scan this QR code with your phone:" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$nodePath = (Get-Command node -ErrorAction SilentlyContinue).Source
if ($nodePath) {
    & node -e "require('qrcode-terminal').generate('$phoneUrl', {small: true})" 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "(QR code unavailable - run 'npm install' first)" -ForegroundColor Yellow
    }
} else {
    Write-Host "(QR code unavailable - node not found)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Or open this URL manually:" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  $phoneUrl" -ForegroundColor White
Write-Host ""
Write-Host "  Password: $Password" -ForegroundColor White
Write-Host "  (enter this on your phone to connect)" -ForegroundColor DarkGray
Write-Host ""
if ($ApiKey) {
    Write-Host "  API Key: (embedded in QR code)" -ForegroundColor DarkGray
}
Write-Host "  Tunnel:  $publicUrl (permanent)" -ForegroundColor DarkGray
Write-Host "  Local:   http://localhost:$Port/phone" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Press Ctrl+C to stop." -ForegroundColor Yellow
Write-Host ""

# --- Wait for Ctrl+C, then cleanup ---
try {
    while ($true) {
        Start-Sleep -Seconds 1
        if ($remoteProc.HasExited) {
            Write-Host "godly-remote exited unexpectedly." -ForegroundColor Red
            break
        }
        if ($cfProc.HasExited) {
            Write-Host ""
            Write-Host "cloudflared exited unexpectedly (exit code: $($cfProc.ExitCode))." -ForegroundColor Red
            $errLog = Join-Path $env:TEMP "godly-cloudflared-err.log"
            if (Test-Path $errLog) {
                $logTail = Get-Content $errLog -Tail 20
                if ($logTail) {
                    Write-Host ""
                    Write-Host "--- cloudflared log (last 20 lines) ---" -ForegroundColor Yellow
                    $logTail | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
                    Write-Host "--- end ---" -ForegroundColor Yellow
                }
            }
            break
        }
    }
} finally {
    Write-Host ""
    Write-Host "Shutting down..." -ForegroundColor Yellow
    Stop-Process -Id $cfProc.Id -Force -ErrorAction SilentlyContinue
    Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
    Write-Host "Done." -ForegroundColor Green
}
