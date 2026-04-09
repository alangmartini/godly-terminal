param(
    [int]$ProcessId,
    [string]$OutPath = "C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\current-godly-shell.png",
    [switch]$WindowOnly,
    [switch]$ClientOnly,
    [switch]$UsePrintWindow,
    [int]$ClientWidth,
    [int]$ClientHeight,
    [int]$WaitAfterResizeMs = 200,
    [int]$WaitAfterActivateMs = 450
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (($ClientWidth -gt 0) -xor ($ClientHeight -gt 0)) {
    throw "Specify both -ClientWidth and -ClientHeight, or neither."
}

if ($ClientWidth -gt 0 -and $ClientHeight -gt 0) {
    $ClientOnly = $true
}

if (($WindowOnly -or $ClientOnly) -and -not $PSBoundParameters.ContainsKey('UsePrintWindow')) {
    $UsePrintWindow = $true
}

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class WinApi {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT {
        public int X;
        public int Y;
    }

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    public static extern bool SetProcessDPIAware();

    [DllImport("user32.dll")]
    public static extern bool SetProcessDpiAwarenessContext(IntPtr dpiFlag);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);

    [DllImport("user32.dll")]
    public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);

    [DllImport("user32.dll")]
    public static extern bool SetWindowPos(
        IntPtr hWnd,
        IntPtr hWndInsertAfter,
        int X,
        int Y,
        int cx,
        int cy,
        uint uFlags
    );

    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdcBlt, int nFlags);

    [DllImport("user32.dll")]
    public static extern bool BringWindowToTop(IntPtr hWnd);
}
"@

[void][WinApi]::SetProcessDpiAwarenessContext([IntPtr](-4))
[void][WinApi]::SetProcessDPIAware()

$SW_MAXIMIZE = 3
$SW_RESTORE = 9
$SWP_NOZORDER = 0x0004
$SWP_NOOWNERZORDER = 0x0200
$SWP_NOACTIVATE = 0x0010
$RESIZE_FLAGS = $SWP_NOZORDER -bor $SWP_NOOWNERZORDER -bor $SWP_NOACTIVATE
$SWP_NOMOVE = 0x0002
$SWP_NOSIZE = 0x0001
$SWP_SHOWWINDOW = 0x0040
$TOPMOST_FLAGS = $SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW
$HWND_TOPMOST = [IntPtr](-1)
$HWND_NOTOPMOST = [IntPtr](-2)

function Get-WindowBounds {
    param([IntPtr]$Hwnd)

    $rect = New-Object WinApi+RECT
    if (-not [WinApi]::GetWindowRect($Hwnd, [ref]$rect)) {
        throw "GetWindowRect failed."
    }

    [pscustomobject]@{
        Left = $rect.Left
        Top = $rect.Top
        Right = $rect.Right
        Bottom = $rect.Bottom
        Width = $rect.Right - $rect.Left
        Height = $rect.Bottom - $rect.Top
    }
}

function Get-ClientMetrics {
    param([IntPtr]$Hwnd)

    $window = Get-WindowBounds -Hwnd $Hwnd
    $clientRect = New-Object WinApi+RECT
    if (-not [WinApi]::GetClientRect($Hwnd, [ref]$clientRect)) {
        throw "GetClientRect failed."
    }

    $origin = New-Object WinApi+POINT
    $origin.X = 0
    $origin.Y = 0
    if (-not [WinApi]::ClientToScreen($Hwnd, [ref]$origin)) {
        throw "ClientToScreen failed."
    }

    $clientWidth = $clientRect.Right - $clientRect.Left
    $clientHeight = $clientRect.Bottom - $clientRect.Top

    [pscustomobject]@{
        Window = $window
        ClientWidth = $clientWidth
        ClientHeight = $clientHeight
        OffsetX = $origin.X - $window.Left
        OffsetY = $origin.Y - $window.Top
    }
}

function Get-ClientBounds {
    param([IntPtr]$Hwnd)

    $metrics = Get-ClientMetrics -Hwnd $Hwnd
    [pscustomobject]@{
        Left = $metrics.Window.Left + $metrics.OffsetX
        Top = $metrics.Window.Top + $metrics.OffsetY
        Width = $metrics.ClientWidth
        Height = $metrics.ClientHeight
    }
}

function Resize-ClientArea {
    param(
        [IntPtr]$Hwnd,
        [int]$TargetWidth,
        [int]$TargetHeight,
        [int]$WaitMs
    )

    for ($i = 0; $i -lt 6; $i++) {
        $metrics = Get-ClientMetrics -Hwnd $Hwnd
        $deltaWidth = $TargetWidth - $metrics.ClientWidth
        $deltaHeight = $TargetHeight - $metrics.ClientHeight

        if ([Math]::Abs($deltaWidth) -le 1 -and [Math]::Abs($deltaHeight) -le 1) {
            return
        }

        $newWidth = [Math]::Max(1, $metrics.Window.Width + $deltaWidth)
        $newHeight = [Math]::Max(1, $metrics.Window.Height + $deltaHeight)
        [WinApi]::SetWindowPos(
            $Hwnd,
            [IntPtr]::Zero,
            $metrics.Window.Left,
            $metrics.Window.Top,
            $newWidth,
            $newHeight,
            $RESIZE_FLAGS
        ) | Out-Null
        Start-Sleep -Milliseconds $WaitMs
    }

    $finalMetrics = Get-ClientMetrics -Hwnd $Hwnd
    if ($finalMetrics.ClientWidth -ne $TargetWidth -or $finalMetrics.ClientHeight -ne $TargetHeight) {
        throw "Failed to resize client area to ${TargetWidth}x${TargetHeight}. Final size: $($finalMetrics.ClientWidth)x$($finalMetrics.ClientHeight)"
    }
}

function Activate-WindowForCapture {
    param(
        [IntPtr]$Hwnd,
        [int]$TargetProcessId,
        [int]$WaitMs
    )

    [WinApi]::BringWindowToTop($Hwnd) | Out-Null
    [WinApi]::SetForegroundWindow($Hwnd) | Out-Null

    try {
        $wshell = New-Object -ComObject WScript.Shell
        [void]$wshell.AppActivate($TargetProcessId)
    } catch {
        # Best effort only.
    }

    Start-Sleep -Milliseconds $WaitMs
}

function Save-PrintWindowBitmap {
    param(
        [IntPtr]$Hwnd,
        [string]$Path,
        [bool]$CropClient
    )

    $metrics = Get-ClientMetrics -Hwnd $Hwnd
    $bitmap = New-Object System.Drawing.Bitmap($metrics.Window.Width, $metrics.Window.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    [WinApi]::PrintWindow($Hwnd, $hdc, 2) | Out-Null
    $graphics.ReleaseHdc($hdc)
    $graphics.Dispose()

    if ($CropClient) {
        $cropRect = New-Object System.Drawing.Rectangle(
            $metrics.OffsetX,
            $metrics.OffsetY,
            $metrics.ClientWidth,
            $metrics.ClientHeight
        )
        $cropped = $bitmap.Clone($cropRect, $bitmap.PixelFormat)
        $cropped.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
        $cropped.Dispose()
        $bitmap.Dispose()
        return [pscustomobject]@{
            Width = $metrics.ClientWidth
            Height = $metrics.ClientHeight
            Label = "Client"
        }
    }

    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    return [pscustomobject]@{
        Width = $metrics.Window.Width
        Height = $metrics.Window.Height
        Label = "PrintWindow"
    }
}

function Save-ScreenBitmap {
    param(
        [int]$Left,
        [int]$Top,
        [int]$Width,
        [int]$Height,
        [string]$Path,
        [string]$Label
    )

    $bitmap = New-Object System.Drawing.Bitmap($Width, $Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen(
        [System.Drawing.Point]::new($Left, $Top),
        [System.Drawing.Point]::Empty,
        [System.Drawing.Size]::new($Width, $Height)
    )
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()

    return [pscustomobject]@{
        Width = $Width
        Height = $Height
        Label = $Label
    }
}

$proc = if ($ProcessId) {
    Get-Process -Id $ProcessId -ErrorAction Stop
} else {
    Get-Process godly-native -ErrorAction SilentlyContinue |
        Sort-Object StartTime -Descending |
        Select-Object -First 1
}

$outDir = Split-Path -Parent $OutPath
if ($outDir) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

if (-not $proc -or $proc.MainWindowHandle -eq 0) {
    if ($WindowOnly -or $ClientOnly) {
        throw "No godly-native window found to capture."
    }
} else {
    if ($ClientWidth -gt 0 -and $ClientHeight -gt 0) {
        [WinApi]::ShowWindow($proc.MainWindowHandle, $SW_RESTORE) | Out-Null
    } else {
        [WinApi]::ShowWindow($proc.MainWindowHandle, $SW_MAXIMIZE) | Out-Null
    }
    [WinApi]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
    Start-Sleep -Seconds 1

    if ($ClientWidth -gt 0 -and $ClientHeight -gt 0) {
        Resize-ClientArea -Hwnd $proc.MainWindowHandle -TargetWidth $ClientWidth -TargetHeight $ClientHeight -WaitMs $WaitAfterResizeMs
    }
}

if (($WindowOnly -or $ClientOnly) -and $proc -and $proc.MainWindowHandle -ne 0) {
    $saved = $null
    $madeTopMost = $false
    try {
        if (-not $UsePrintWindow) {
            [WinApi]::SetWindowPos(
                $proc.MainWindowHandle,
                $HWND_TOPMOST,
                0,
                0,
                0,
                0,
                $TOPMOST_FLAGS
            ) | Out-Null
            $madeTopMost = $true
            Activate-WindowForCapture -Hwnd $proc.MainWindowHandle -TargetProcessId $proc.Id -WaitMs $WaitAfterActivateMs
        }

        if ($UsePrintWindow) {
            $saved = Save-PrintWindowBitmap -Hwnd $proc.MainWindowHandle -Path $OutPath -CropClient:$ClientOnly
        } elseif ($ClientOnly) {
            $bounds = Get-ClientBounds -Hwnd $proc.MainWindowHandle
            $saved = Save-ScreenBitmap `
                -Left $bounds.Left `
                -Top $bounds.Top `
                -Width $bounds.Width `
                -Height $bounds.Height `
                -Path $OutPath `
                -Label "Client"
        } else {
            $bounds = Get-WindowBounds -Hwnd $proc.MainWindowHandle
            $saved = Save-ScreenBitmap `
                -Left $bounds.Left `
                -Top $bounds.Top `
                -Width $bounds.Width `
                -Height $bounds.Height `
                -Path $OutPath `
                -Label "Window"
        }
    } finally {
        if ($madeTopMost) {
            [WinApi]::SetWindowPos(
                $proc.MainWindowHandle,
                $HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                $TOPMOST_FLAGS
            ) | Out-Null
        }
    }

    $mode = if ($UsePrintWindow) { "PrintWindow" } else { "Screen" }
    Write-Host ("{0} screenshot saved to {1} ({2}x{3}, mode={4})" -f $saved.Label, $OutPath, $saved.Width, $saved.Height, $mode)
    return
}

$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($screen.Location, [System.Drawing.Point]::Empty, $screen.Size)
$bitmap.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
Write-Host ("Screen screenshot saved to {0} ({1}x{2})" -f $OutPath, $screen.Width, $screen.Height)
