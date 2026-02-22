# Automated phone setup: starts godly-remote + ngrok tunnel + displays QR code
# Usage: pwsh scripts/setup-phone.ps1 [-Port <port>]

param(
    [int]$Port = 3377
)

$ErrorActionPreference = "Stop"

# --- Check ngrok ---
if (-not (Get-Command ngrok -ErrorAction SilentlyContinue)) {
    Write-Host "ngrok not found." -ForegroundColor Red
    Write-Host ""
    Write-Host "Install it with:" -ForegroundColor Yellow
    Write-Host "  winget install ngrok.ngrok" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Then authenticate:" -ForegroundColor Yellow
    Write-Host "  ngrok config add-authtoken <your-token>" -ForegroundColor Cyan
    Write-Host "  (Get a free token at https://dashboard.ngrok.com/get-started/your-authtoken)" -ForegroundColor DarkGray
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

# --- Generate API key ---
$ApiKey = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 24 | ForEach-Object { [char]$_ })

# --- Check if godly-remote is already running on this port ---
$remoteProc = $null
$remoteAlreadyRunning = $false
try {
    $resp = Invoke-RestMethod -Uri "http://localhost:$Port/health" -TimeoutSec 2 -ErrorAction SilentlyContinue
    $remoteAlreadyRunning = $true
} catch {
    # Not running, we'll start it
}

if ($remoteAlreadyRunning) {
    Write-Host ""
    Write-Host "godly-remote already running on port $Port, reusing it." -ForegroundColor Green
    Write-Host "  Note: API key auth is managed by the existing instance." -ForegroundColor DarkGray
    # Use whatever key the running instance has (or none)
    $ApiKey = $null
} else {
    # --- Start godly-remote ---
    Write-Host ""
    Write-Host "Starting godly-remote on port $Port..." -ForegroundColor Green
    $env:GODLY_REMOTE_PORT = $Port
    $env:GODLY_REMOTE_API_KEY = $ApiKey
    $remoteProc = Start-Process -FilePath $remoteBin -PassThru -NoNewWindow
    Start-Sleep -Seconds 1

    if ($remoteProc.HasExited) {
        Write-Host "godly-remote failed to start. Is the daemon running?" -ForegroundColor Red
        Write-Host "Start Godly Terminal first, or run: src-tauri\target\release\godly-daemon.exe" -ForegroundColor Yellow
        exit 1
    }
}

# --- Start ngrok ---
Write-Host "Starting ngrok tunnel..." -ForegroundColor Green
$ngrokProc = Start-Process -FilePath "ngrok" -ArgumentList "http", "$Port", "--log=stderr" -PassThru -NoNewWindow -RedirectStandardError "$env:TEMP\ngrok-stderr.log"

# --- Get public URL from ngrok API ---
$publicUrl = $null
$attempts = 0
$maxAttempts = 15
while (-not $publicUrl -and $attempts -lt $maxAttempts) {
    $attempts++
    Start-Sleep -Seconds 1
    try {
        $tunnels = Invoke-RestMethod -Uri "http://localhost:4040/api/tunnels" -ErrorAction SilentlyContinue
        $tunnel = $tunnels.tunnels | Where-Object { $_.proto -eq "https" } | Select-Object -First 1
        if ($tunnel) {
            $publicUrl = $tunnel.public_url
        }
    } catch {
        # ngrok not ready yet
    }
}

if (-not $publicUrl) {
    Write-Host "Failed to get ngrok tunnel URL after ${maxAttempts}s." -ForegroundColor Red
    Write-Host "Check ngrok auth: ngrok config add-authtoken <token>" -ForegroundColor Yellow
    if ($remoteProc) { Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $ngrokProc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

# --- Build phone URL with embedded API key ---
if ($ApiKey) {
    $phoneUrl = "$publicUrl/phone?key=$ApiKey"
} else {
    $phoneUrl = "$publicUrl/phone"
}

# --- Display QR code ---
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Scan this QR code with your phone:" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Use npx qrcode-terminal to render QR in terminal
$npxPath = (Get-Command npx -ErrorAction SilentlyContinue).Source
if ($npxPath) {
    & npx --yes qrcode-terminal "$phoneUrl" --small
} else {
    Write-Host "(QR code unavailable - npx not found)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Or open this URL manually:" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  $phoneUrl" -ForegroundColor White
Write-Host ""
if ($ApiKey) {
    Write-Host "  API Key: $ApiKey" -ForegroundColor DarkGray
}
Write-Host "  Tunnel:  $publicUrl" -ForegroundColor DarkGray
Write-Host "  Local:   http://localhost:$Port/phone" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Press Ctrl+C to stop." -ForegroundColor Yellow
Write-Host ""

# --- Wait for Ctrl+C, then cleanup ---
try {
    while ($true) {
        Start-Sleep -Seconds 1
        # Check if processes are still alive
        if ($remoteProc -and $remoteProc.HasExited) {
            Write-Host "godly-remote exited unexpectedly." -ForegroundColor Red
            break
        }
        if ($ngrokProc.HasExited) {
            Write-Host "ngrok exited unexpectedly." -ForegroundColor Red
            break
        }
    }
} finally {
    Write-Host ""
    Write-Host "Shutting down..." -ForegroundColor Yellow
    Stop-Process -Id $ngrokProc.Id -Force -ErrorAction SilentlyContinue
    if ($remoteProc) {
        Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
    }
    Write-Host "Done." -ForegroundColor Green
}
