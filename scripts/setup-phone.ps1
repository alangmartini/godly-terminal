# Automated phone setup: starts godly-remote + ngrok tunnel + displays QR code
# Usage: pwsh scripts/setup-phone.ps1 [-Port <port>]

param(
    [int]$Port = 3377
)

$ErrorActionPreference = "Stop"

# --- Find ngrok ---
$ngrokBin = (Get-Command ngrok -ErrorAction SilentlyContinue).Source
if (-not $ngrokBin) {
    # Check common winget install location
    $wingetPath = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages"
    $ngrokDir = Get-ChildItem -Path $wingetPath -Filter "Ngrok*" -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($ngrokDir) {
        $candidate = Join-Path $ngrokDir.FullName "ngrok.exe"
        if (Test-Path $candidate) { $ngrokBin = $candidate }
    }
}
if (-not $ngrokBin) {
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

# --- Generate API key and password ---
$ApiKey = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 24 | ForEach-Object { [char]$_ })
$Password = -join ((48..57) + (97..122) | Get-Random -Count 6 | ForEach-Object { [char]$_ })

# --- Kill any existing godly-remote so we start fresh with our API key + password ---
$remoteProc = $null
$existingPids = Get-Process -Name "godly-remote" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id
if ($existingPids) {
    Write-Host ""
    Write-Host "Stopping existing godly-remote (PID: $($existingPids -join ', '))..." -ForegroundColor Yellow
    foreach ($pid in $existingPids) {
        Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Milliseconds 500
}

# --- Start godly-remote ---
Write-Host ""
Write-Host "Starting godly-remote on port $Port..." -ForegroundColor Green
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

# --- Start ngrok ---
Write-Host "Starting ngrok tunnel..." -ForegroundColor Green
$ngrokProc = Start-Process -FilePath $ngrokBin -ArgumentList "http $Port --log=stdout --log-level=warn" -PassThru -WindowStyle Hidden

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

# Use qrcode-terminal (installed as devDependency) to render QR in terminal
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
        if ($remoteProc.HasExited) {
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
    Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
    Write-Host "Done." -ForegroundColor Green
}
