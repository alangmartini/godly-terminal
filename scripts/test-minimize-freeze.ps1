$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32 {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
    public const int SW_MINIMIZE = 6;
    public const int SW_RESTORE = 9;
}
"@

$appPath = "$env:LOCALAPPDATA\Godly Terminal\godly-native.exe"
$diagLog = "$env:APPDATA\com.godly.terminal\iced-diag.log"

Write-Host "=== Minimize Freeze Test ===" -ForegroundColor Cyan

# 1. Launch
Write-Host "1. Launching Godly Terminal..." -ForegroundColor Yellow
$proc = Start-Process $appPath -PassThru
Start-Sleep -Seconds 5

# 2. Find window
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 10; $i++) {
    $hwnd = [Win32]::FindWindow($null, "Godly Terminal")
    if ($hwnd -ne [IntPtr]::Zero) { break }
    # Try partial match
    Get-Process godly-native -ErrorAction SilentlyContinue | ForEach-Object {
        $hwnd = $_.MainWindowHandle
    }
    if ($hwnd -ne [IntPtr]::Zero) { break }
    Start-Sleep -Seconds 1
}

if ($hwnd -eq [IntPtr]::Zero) {
    Write-Host "FAIL: Could not find window" -ForegroundColor Red
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "   Window found: $hwnd" -ForegroundColor Green

# 3. Focus and type
Write-Host "2. Typing 'hello' ..." -ForegroundColor Yellow
[Win32]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Seconds 2
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait("hello{ENTER}")
Start-Sleep -Seconds 2

# 4. Minimize
Write-Host "3. Minimizing window..." -ForegroundColor Yellow
[Win32]::ShowWindow($hwnd, [Win32]::SW_MINIMIZE) | Out-Null
Start-Sleep -Seconds 1

# 5. Wait 15 seconds (simulating user away)
Write-Host "4. Waiting 15 seconds while minimized..." -ForegroundColor Yellow
Start-Sleep -Seconds 15

# 6. Restore
Write-Host "5. Restoring window..." -ForegroundColor Yellow
[Win32]::ShowWindow($hwnd, [Win32]::SW_RESTORE) | Out-Null
Start-Sleep -Seconds 2
[Win32]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Seconds 2

# 7. Type again
Write-Host "6. Typing 'world' after restore..." -ForegroundColor Yellow
[System.Windows.Forms.SendKeys]::SendWait("world{ENTER}")
Start-Sleep -Seconds 3

# 8. Check diagnostic log
Write-Host "`n=== Diagnostic Log Analysis ===" -ForegroundColor Cyan

if (-not (Test-Path $diagLog)) {
    Write-Host "FAIL: Diagnostic log not found" -ForegroundColor Red
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

$log = Get-Content $diagLog -Raw
$lines = Get-Content $diagLog

# Check for heartbeat focus corrections
$corrections = $lines | Select-String "focus correction"
$focusEvents = $lines | Select-String "WindowFocusChanged"
$heartbeats = $lines | Select-String "Heartbeat"
$lastLine = $lines[-1]

Write-Host "Focus events: $($focusEvents.Count)" -ForegroundColor Gray
Write-Host "Heartbeat entries: $($heartbeats.Count)" -ForegroundColor Gray
Write-Host "Focus corrections: $($corrections.Count)" -ForegroundColor Gray
Write-Host "Last log line: $lastLine" -ForegroundColor Gray

# Check if keyboard events appear after restore
$restoreTime = $null
foreach ($line in $lines) {
    if ($line -match "RESIZE:.*->" -and $restoreTime -eq $null) {
        # Skip initial resizes
    }
    if ($line -match "\[\s*([\d.]+)\].*WindowFocusChanged\(focused=true\)") {
        $restoreTime = [float]$Matches[1]
    }
}

$postRestoreKeyboard = 0
$postRestoreOutput = 0
foreach ($line in $lines) {
    if ($line -match "\[\s*([\d.]+)\].*KeyboardEvent") {
        $t = [float]$Matches[1]
        if ($restoreTime -and $t -gt $restoreTime) {
            $postRestoreKeyboard++
        }
    }
    if ($line -match "\[\s*([\d.]+)\].*TerminalOutput") {
        $t = [float]$Matches[1]
        if ($restoreTime -and $t -gt $restoreTime) {
            $postRestoreOutput++
        }
    }
}

Write-Host "`nPost-restore keyboard events: $postRestoreKeyboard" -ForegroundColor $(if ($postRestoreKeyboard -gt 0) { "Green" } else { "Red" })
Write-Host "Post-restore terminal output: $postRestoreOutput" -ForegroundColor $(if ($postRestoreOutput -gt 0) { "Green" } else { "Red" })

if ($postRestoreKeyboard -gt 0 -and $postRestoreOutput -gt 0) {
    Write-Host "`nRESULT: PASS - Terminal responsive after restore" -ForegroundColor Green
} else {
    Write-Host "`nRESULT: FAIL - Terminal not responsive after restore" -ForegroundColor Red
    Write-Host "`nLast 20 lines of diag log:" -ForegroundColor Yellow
    $lines | Select-Object -Last 20 | ForEach-Object { Write-Host "  $_" }
}

# Cleanup
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
