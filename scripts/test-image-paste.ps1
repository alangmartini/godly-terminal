$ErrorActionPreference = "Stop"

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32 {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
"@

$appPath = "$env:LOCALAPPDATA\Godly Terminal\godly-native.exe"
$diagLog = "$env:APPDATA\com.godly.terminal\iced-diag.log"

Write-Host "=== Image Paste in Quick Claude Test (Bug #732) ===" -ForegroundColor Cyan

# 0. Clear old diagnostic log so we only see events from this run
if (Test-Path $diagLog) { Remove-Item $diagLog -Force }

# 1. Launch
Write-Host "1. Launching Godly Terminal..." -ForegroundColor Yellow
$proc = Start-Process $appPath -PassThru
Start-Sleep -Seconds 5

# 2. Find window
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 10; $i++) {
    $hwnd = [Win32]::FindWindow($null, "Godly Terminal")
    if ($hwnd -ne [IntPtr]::Zero) { break }
    Get-Process godly-native -ErrorAction SilentlyContinue | ForEach-Object {
        $hwnd = $_.MainWindowHandle
    }
    if ($hwnd -ne [IntPtr]::Zero) { break }
    Start-Sleep -Seconds 1
}

if ($hwnd -eq [IntPtr]::Zero) {
    Write-Host "RESULT: FAIL - Could not find window" -ForegroundColor Red
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "   Window found: $hwnd" -ForegroundColor Green

# 3. Focus
[Win32]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Seconds 2

# 4. Put a screenshot (image-only) on the clipboard
Write-Host "2. Placing image-only data on clipboard..." -ForegroundColor Yellow
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

# Clear clipboard first
[System.Windows.Forms.Clipboard]::Clear()
Start-Sleep -Milliseconds 200

# Create a small test image and place on clipboard (no text)
$bmp = New-Object System.Drawing.Bitmap(4, 4)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::Red)
$g.Dispose()
[System.Windows.Forms.Clipboard]::SetImage($bmp)
$bmp.Dispose()
Start-Sleep -Milliseconds 500

# Verify clipboard has image but no text
$hasImage = [System.Windows.Forms.Clipboard]::ContainsImage()
$hasText = [System.Windows.Forms.Clipboard]::ContainsText()
Write-Host "   Clipboard: image=$hasImage text=$hasText" -ForegroundColor Gray

if (-not $hasImage) {
    Write-Host "RESULT: FAIL - Could not place image on clipboard" -ForegroundColor Red
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}
if ($hasText) {
    Write-Host "   WARNING: clipboard also has text (test may not exercise image-only path)" -ForegroundColor Yellow
}

# 5. Open Quick Claude dialog (Ctrl+Shift+Q)
Write-Host "3. Opening Quick Claude dialog (Ctrl+Shift+Q)..." -ForegroundColor Yellow
[System.Windows.Forms.SendKeys]::SendWait("^+q")
Start-Sleep -Seconds 2

# 6. Paste with Ctrl+V (image-only clipboard)
Write-Host "4. Pasting image with Ctrl+V..." -ForegroundColor Yellow
[System.Windows.Forms.SendKeys]::SendWait("^v")
Start-Sleep -Seconds 3

# 7. Check diagnostic log for WidgetCapturedPaste event
Write-Host "`n=== Diagnostic Log Analysis ===" -ForegroundColor Cyan

if (-not (Test-Path $diagLog)) {
    Write-Host "RESULT: FAIL - Diagnostic log not found" -ForegroundColor Red
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

$lines = Get-Content $diagLog

# Look for the WidgetCapturedPaste event (our fix)
$pasteEvents = $lines | Select-String "WidgetCapturedPaste"
$imagePastedEvents = $lines | Select-String "QuickClaudeDialogImagePasted"

Write-Host "WidgetCapturedPaste events: $($pasteEvents.Count)" -ForegroundColor Gray
Write-Host "ImagePasted result events:  $($imagePastedEvents.Count)" -ForegroundColor Gray

# Show relevant log lines
$relevantLines = $lines | Select-String "WidgetCapturedPaste|ImagePasted|clipboard"
if ($relevantLines) {
    Write-Host "`nRelevant log lines:" -ForegroundColor Yellow
    $relevantLines | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
}

# The fix is confirmed if WidgetCapturedPaste fired.
# The image attachment succeeding depends on arboard detecting the image.
$passed = $pasteEvents.Count -gt 0

if ($passed) {
    Write-Host "`nRESULT: PASS - WidgetCapturedPaste fired for image-only clipboard" -ForegroundColor Green
} else {
    Write-Host "`nRESULT: FAIL - WidgetCapturedPaste did not fire" -ForegroundColor Red
    Write-Host "`nLast 20 lines of diag log:" -ForegroundColor Yellow
    $lines | Select-Object -Last 20 | ForEach-Object { Write-Host "  $_" }
}

# 8. Escape to close dialog, then cleanup
[System.Windows.Forms.SendKeys]::SendWait("{ESC}")
Start-Sleep -Seconds 1
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
