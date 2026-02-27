# Automated phone setup: starts godly-remote + optional tunnel + displays QR code
# Usage:
#   pwsh scripts/setup-phone.ps1                          # local-only (no tunnel)
#   pwsh scripts/setup-phone.ps1 -Tunnel cloudflare -TunnelName my-tunnel -Hostname phone.example.com
#   pwsh scripts/setup-phone.ps1 -Tunnel ngrok -NgrokDomain my-app.ngrok-free.app
#
# Tunnel modes:
#   local       — no tunnel, accessible only on your LAN (default)
#   cloudflare  — requires cloudflared + a pre-configured named tunnel
#   ngrok       — requires ngrok + optional static domain

param(
    [int]$Port = 3377,
    [ValidateSet("local", "cloudflare", "ngrok")]
    [string]$Tunnel = "local",

    # Cloudflare options
    [string]$TunnelName,
    [string]$Hostname,

    # ngrok options
    [string]$NgrokDomain
)

$ErrorActionPreference = "Stop"

# ─── Validate tunnel-specific params ───

if ($Tunnel -eq "cloudflare") {
    if (-not $TunnelName) { Write-Host "-TunnelName is required for cloudflare tunnel mode." -ForegroundColor Red; exit 1 }
    if (-not $Hostname)   { Write-Host "-Hostname is required for cloudflare tunnel mode." -ForegroundColor Red; exit 1 }
}

# ─── Find tunnel binary (if needed) ───

$tunnelBin = $null
$tunnelProc = $null

if ($Tunnel -eq "cloudflare") {
    $tunnelBin = (Get-Command cloudflared -ErrorAction SilentlyContinue).Source
    if (-not $tunnelBin) {
        $candidate = "${env:ProgramFiles(x86)}\cloudflared\cloudflared.exe"
        if (Test-Path $candidate) { $tunnelBin = $candidate }
    }
    if (-not $tunnelBin) {
        $candidate = "$env:ProgramFiles\cloudflared\cloudflared.exe"
        if (Test-Path $candidate) { $tunnelBin = $candidate }
    }
    if (-not $tunnelBin) {
        Write-Host "cloudflared not found." -ForegroundColor Red
        Write-Host ""
        Write-Host "Install it with:" -ForegroundColor Yellow
        Write-Host "  winget install Cloudflare.cloudflared" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Then authenticate and create a tunnel:" -ForegroundColor Yellow
        Write-Host "  cloudflared tunnel login" -ForegroundColor Cyan
        Write-Host "  cloudflared tunnel create <name>" -ForegroundColor Cyan
        Write-Host "  cloudflared tunnel route dns <name> <hostname>" -ForegroundColor Cyan
        exit 1
    }
}

if ($Tunnel -eq "ngrok") {
    $tunnelBin = (Get-Command ngrok -ErrorAction SilentlyContinue).Source
    if (-not $tunnelBin) {
        Write-Host "ngrok not found." -ForegroundColor Red
        Write-Host ""
        Write-Host "Install it with:" -ForegroundColor Yellow
        Write-Host "  winget install ngrok.ngrok" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Then authenticate:" -ForegroundColor Yellow
        Write-Host "  ngrok config add-authtoken <token>" -ForegroundColor Cyan
        exit 1
    }
}

# ─── Check godly-remote binary ───

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

# ─── Read persisted settings (if any) ───

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

# ─── API key and password ───
# Match the charset and length from the Settings UI (remote-settings-store.ts)
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

if ($savedConfig -and $savedConfig.port -and $Port -eq 3377) {
    $Port = $savedConfig.port
}

# ─── Save settings for next run ───

$configDir = Split-Path $configPath
if (-not (Test-Path $configDir)) { New-Item -ItemType Directory -Path $configDir -Force | Out-Null }
@{
    api_key  = $ApiKey
    password = $Password
    port     = $Port
    tunnel   = $Tunnel
} | ConvertTo-Json | Set-Content $configPath

# ─── Kill any existing godly-remote ───

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

# ─── Start godly-remote ───

$bindHost = if ($Tunnel -eq "local") { "0.0.0.0" } else { "127.0.0.1" }
Write-Host ""
Write-Host "Starting godly-remote on ${bindHost}:${Port}..." -ForegroundColor Green
$env:GODLY_REMOTE_HOST = $bindHost
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

# ─── Start tunnel ───

$publicUrl = $null

if ($Tunnel -eq "cloudflare") {
    Write-Host "Starting Cloudflare Tunnel ($TunnelName -> $Hostname)..." -ForegroundColor Green
    $cfLog = Join-Path $env:TEMP "godly-cloudflared.log"
    $cfErrLog = Join-Path $env:TEMP "godly-cloudflared-err.log"
    $tunnelProc = Start-Process -FilePath $tunnelBin -ArgumentList "tunnel run $TunnelName" -PassThru -WindowStyle Hidden -RedirectStandardOutput $cfLog -RedirectStandardError $cfErrLog

    Write-Host "  Waiting for tunnel connection..." -ForegroundColor DarkGray
    Start-Sleep -Seconds 3
    if ($tunnelProc.HasExited) {
        Write-Host "cloudflared exited unexpectedly (exit code: $($tunnelProc.ExitCode))." -ForegroundColor Red
        if (Test-Path $cfErrLog) {
            $logTail = Get-Content $cfErrLog -Tail 15
            if ($logTail) {
                Write-Host ""
                $logTail | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
            }
        }
        Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
        exit 1
    }
    Write-Host "  Tunnel connected." -ForegroundColor Green
    $publicUrl = "https://$Hostname"
}

if ($Tunnel -eq "ngrok") {
    Write-Host "Starting ngrok tunnel to port $Port..." -ForegroundColor Green
    $ngrokArgs = "http $Port --log=stdout"
    if ($NgrokDomain) { $ngrokArgs += " --domain=$NgrokDomain" }
    $ngrokLog = Join-Path $env:TEMP "godly-ngrok.log"
    $tunnelProc = Start-Process -FilePath $tunnelBin -ArgumentList $ngrokArgs -PassThru -WindowStyle Hidden -RedirectStandardOutput $ngrokLog -RedirectStandardError (Join-Path $env:TEMP "godly-ngrok-err.log")

    Write-Host "  Waiting for ngrok..." -ForegroundColor DarkGray
    Start-Sleep -Seconds 3
    if ($tunnelProc.HasExited) {
        Write-Host "ngrok exited unexpectedly." -ForegroundColor Red
        Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
        exit 1
    }

    if ($NgrokDomain) {
        $publicUrl = "https://$NgrokDomain"
    } else {
        # Query ngrok API for the auto-assigned URL
        try {
            $ngrokApi = Invoke-RestMethod -Uri "http://127.0.0.1:4040/api/tunnels" -TimeoutSec 5
            $publicUrl = $ngrokApi.tunnels[0].public_url
        } catch {
            Write-Host "  Could not detect ngrok URL from API. Check http://127.0.0.1:4040" -ForegroundColor Yellow
        }
    }
    if ($publicUrl) {
        Write-Host "  ngrok tunnel: $publicUrl" -ForegroundColor Green
    }
}

# ─── Build phone URL ───

if ($publicUrl) {
    $phoneUrl = if ($ApiKey) { "$publicUrl/phone#key=$ApiKey" } else { "$publicUrl/phone" }
} else {
    # Local mode — use LAN IP so the phone can reach it
    $lanIp = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notlike "*Loopback*" -and $_.PrefixOrigin -eq "Dhcp" } | Select-Object -First 1).IPAddress
    if (-not $lanIp) { $lanIp = "localhost" }
    $phoneUrl = if ($ApiKey) { "http://${lanIp}:${Port}/phone#key=$ApiKey" } else { "http://${lanIp}:${Port}/phone" }
}

# ─── Display QR code ───

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
Write-Host "  Password: (saved in $configPath)" -ForegroundColor DarkGray
Write-Host "  (run: Get-Content '$configPath' | ConvertFrom-Json | Select -Expand password)" -ForegroundColor DarkGray
Write-Host ""
if ($ApiKey) {
    Write-Host "  API Key: (embedded in QR code)" -ForegroundColor DarkGray
}
if ($publicUrl) {
    Write-Host "  Tunnel:  $publicUrl" -ForegroundColor DarkGray
}
Write-Host "  Local:   http://localhost:$Port/phone" -ForegroundColor DarkGray
Write-Host ""
Write-Host "Press Ctrl+C to stop." -ForegroundColor Yellow
Write-Host ""

# ─── Wait for Ctrl+C, then cleanup ───

try {
    while ($true) {
        Start-Sleep -Seconds 1
        if ($remoteProc.HasExited) {
            Write-Host "godly-remote exited unexpectedly." -ForegroundColor Red
            break
        }
        if ($tunnelProc -and $tunnelProc.HasExited) {
            Write-Host ""
            Write-Host "Tunnel process exited unexpectedly." -ForegroundColor Red
            if ($Tunnel -eq "cloudflare") {
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
            }
            break
        }
    }
} finally {
    Write-Host ""
    Write-Host "Shutting down..." -ForegroundColor Yellow
    if ($tunnelProc) { Stop-Process -Id $tunnelProc.Id -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $remoteProc.Id -Force -ErrorAction SilentlyContinue
    Write-Host "Done." -ForegroundColor Green
}
