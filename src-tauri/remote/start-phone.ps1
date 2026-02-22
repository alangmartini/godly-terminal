# Start godly-remote with ngrok tunnel for phone access
# Usage: .\start-phone.ps1 [-ApiKey <key>] [-Port <port>]

param(
    [string]$ApiKey,
    [int]$Port = 3377
)

$ErrorActionPreference = "Stop"

# Check ngrok is installed
if (-not (Get-Command ngrok -ErrorAction SilentlyContinue)) {
    Write-Host "ngrok not found. Install it: https://ngrok.com/download" -ForegroundColor Red
    exit 1
}

# Check godly-remote binary
$remoteBin = "$PSScriptRoot\..\..\target\release\godly-remote.exe"
if (-not (Test-Path $remoteBin)) {
    $remoteBin = "$PSScriptRoot\..\..\target\debug\godly-remote.exe"
}
if (-not (Test-Path $remoteBin)) {
    Write-Host "godly-remote.exe not found. Build it first:" -ForegroundColor Red
    Write-Host "  cd src-tauri && cargo build -p godly-remote --release" -ForegroundColor Yellow
    exit 1
}

# Set env vars
$env:GODLY_REMOTE_PORT = $Port
if ($ApiKey) {
    $env:GODLY_REMOTE_API_KEY = $ApiKey
} elseif (-not $env:GODLY_REMOTE_API_KEY) {
    # Generate a random key if none provided
    $ApiKey = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 24 | ForEach-Object { [char]$_ })
    $env:GODLY_REMOTE_API_KEY = $ApiKey
    Write-Host "Generated API key: $ApiKey" -ForegroundColor Cyan
}

Write-Host "Starting godly-remote on port $Port..." -ForegroundColor Green

# Start godly-remote in background
$remote = Start-Process -FilePath $remoteBin -PassThru -NoNewWindow

# Give it a moment to bind
Start-Sleep -Seconds 1

if ($remote.HasExited) {
    Write-Host "godly-remote failed to start. Is the daemon running?" -ForegroundColor Red
    exit 1
}

Write-Host "Starting ngrok tunnel..." -ForegroundColor Green

# Start ngrok (foreground so Ctrl+C stops everything)
try {
    ngrok http $Port
} finally {
    Write-Host "`nShutting down godly-remote..." -ForegroundColor Yellow
    Stop-Process -Id $remote.Id -Force -ErrorAction SilentlyContinue
}
