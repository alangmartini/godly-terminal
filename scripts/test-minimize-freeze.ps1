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

# 5. Wait 60 seconds — poll shim/daemon/shell processes every 5s to catch when they die
Write-Host "4. Waiting 60 seconds while minimized (monitoring processes)..." -ForegroundColor Yellow

# Snapshot PIDs before minimize
$shimsBefore = Get-Process godly-pty-shim -ErrorAction SilentlyContinue
$daemonBefore = Get-Process godly-daemon -ErrorAction SilentlyContinue
$shellsBefore = Get-Process powershell, pwsh, cmd -ErrorAction SilentlyContinue | Where-Object {
    # Filter to shells that started after our app (rough heuristic)
    $_.StartTime -gt $proc.StartTime
}

Write-Host "   Processes at minimize:" -ForegroundColor Gray
Write-Host "     godly-pty-shim: $($shimsBefore.Count) (PIDs: $($shimsBefore.Id -join ', '))" -ForegroundColor Gray
Write-Host "     godly-daemon:   $($daemonBefore.Count) (PIDs: $($daemonBefore.Id -join ', '))" -ForegroundColor Gray
Write-Host "     shells:         $($shellsBefore.Count) (PIDs: $($shellsBefore.Id -join ', '))" -ForegroundColor Gray

for ($t = 5; $t -le 60; $t += 5) {
    Start-Sleep -Seconds 5
    $shimsNow = Get-Process godly-pty-shim -ErrorAction SilentlyContinue
    $daemonNow = Get-Process godly-daemon -ErrorAction SilentlyContinue
    $shellsNow = Get-Process powershell, pwsh, cmd -ErrorAction SilentlyContinue | Where-Object {
        $_.StartTime -gt $proc.StartTime
    }

    $shimDied = $shimsBefore | Where-Object { $_.Id -notin @($shimsNow.Id) }
    $shellDied = $shellsBefore | Where-Object { $_.Id -notin @($shellsNow.Id) }

    $status = "[${t}s]"
    if ($shimDied) {
        $status += " SHIM DIED (PIDs: $($shimDied.Id -join ', '))"
        Write-Host "   $status" -ForegroundColor Red

        # Check Windows Event Log for termination reason
        $shimPid = $shimDied[0].Id
        $events = Get-WinEvent -FilterHashtable @{
            LogName='System','Application','Security'
            Level=1,2,3,4
        } -MaxEvents 20 -ErrorAction SilentlyContinue | Where-Object {
            $_.Message -match "$shimPid|godly-pty-shim"
        }
        if ($events) {
            Write-Host "   Event Log entries:" -ForegroundColor Yellow
            $events | ForEach-Object {
                Write-Host "     [$($_.TimeCreated)] $($_.ProviderName): $($_.Message.Substring(0, [Math]::Min(200, $_.Message.Length)))" -ForegroundColor Yellow
            }
        }

        # Check shim exit code if possible
        try {
            $shimProc = $shimDied[0]
            if ($shimProc.HasExited) {
                Write-Host "   Shim exit code: $($shimProc.ExitCode)" -ForegroundColor Red
            }
        } catch {}
    }
    elseif ($shellDied) {
        $status += " SHELL DIED (PIDs: $($shellDied.Id -join ', ')) shims=$($shimsNow.Count)"
        Write-Host "   $status" -ForegroundColor Red
    }
    else {
        $status += " all alive: shims=$($shimsNow.Count) daemon=$($daemonNow.Count) shells=$($shellsNow.Count)"
        Write-Host "   $status" -ForegroundColor Gray
    }
}

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
$postRestoreGridFetch = 0
foreach ($line in $lines) {
    if ($line -match "\[\s*([\d.]+)\].*KeyboardEvent") {
        $t = [float]$Matches[1]
        if ($restoreTime -and $t -gt $restoreTime) {
            $postRestoreKeyboard++
        }
    }
    if ($line -match "\[\s*([\d.]+)\].*GridFetched") {
        $t = [float]$Matches[1]
        if ($restoreTime -and $t -gt $restoreTime) {
            $postRestoreGridFetch++
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
Write-Host "Post-restore terminal output: $postRestoreOutput" -ForegroundColor $(if ($postRestoreOutput -gt 0) { "Green" } else { "Yellow" })
Write-Host "Post-restore grid fetches: $postRestoreGridFetch" -ForegroundColor $(if ($postRestoreGridFetch -gt 0) { "Green" } else { "Red" })

# Terminal is responsive if keyboard works AND either streaming events or grid polling works
$terminalAlive = $postRestoreKeyboard -gt 0 -and ($postRestoreOutput -gt 0 -or $postRestoreGridFetch -gt 0)

if ($terminalAlive) {
    Write-Host "`nRESULT: PASS - Terminal responsive after restore" -ForegroundColor Green
} else {
    Write-Host "`nRESULT: FAIL - Terminal not responsive after restore" -ForegroundColor Red
    Write-Host "`nLast 20 lines of diag log:" -ForegroundColor Yellow
    $lines | Select-Object -Last 20 | ForEach-Object { Write-Host "  $_" }
}

# Cleanup
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
