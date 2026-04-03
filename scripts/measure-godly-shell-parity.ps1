param(
    [int]$ProcessId,
    [string]$WebReference = (Join-Path $PSScriptRoot "..\docs\references\web-reference.png"),
    [string]$NativeCapture = (Join-Path $PSScriptRoot "..\docs\references\current-godly-shell.png"),
    [string]$NativeExe = (Join-Path $PSScriptRoot "..\src-tauri\target\debug\godly-native.exe"),
    [int]$ViewportWidth = 1920,
    [int]$ViewportHeight = 1080,
    [int]$LaunchTimeoutMs = 15000,
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$captureWeb = Join-Path $PSScriptRoot "capture-web-reference.ps1"
$captureNative = Join-Path $PSScriptRoot "take-screenshot-now.ps1"
$diffScript = Join-Path $PSScriptRoot "check-pixels.ps1"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$launchedProcess = $null

try {
    if ($ProcessId -le 0) {
        $NativeExe = [System.IO.Path]::GetFullPath($NativeExe)
        if (-not (Test-Path $NativeExe)) {
            throw "No native executable found at $NativeExe. Build `godly-shell` first or pass -ProcessId."
        }

        $launchedProcess = Start-Process `
            -FilePath $NativeExe `
            -ArgumentList @("--web-reference-crop") `
            -WorkingDirectory $repoRoot `
            -PassThru

        $deadline = (Get-Date).AddMilliseconds($LaunchTimeoutMs)
        do {
            Start-Sleep -Milliseconds 200
            try {
                $launchedProcess.Refresh()
            } catch {
                break
            }
        } while ((Get-Date) -lt $deadline -and $launchedProcess.MainWindowHandle -eq 0 -and -not $launchedProcess.HasExited)

        if ($launchedProcess.HasExited) {
            throw "Reference-mode godly-native exited before capture."
        }
        if ($launchedProcess.MainWindowHandle -eq 0) {
            throw "Timed out waiting for the reference-mode godly-native window."
        }

        $ProcessId = $launchedProcess.Id
    }

    & $captureWeb -OutPath $WebReference -ViewportWidth $ViewportWidth -ViewportHeight $ViewportHeight
    & $captureNative -ProcessId $ProcessId -OutPath $NativeCapture -ClientOnly -ClientWidth $ViewportWidth -ClientHeight $ViewportHeight

    if ($Json) {
        & $diffScript -Reference $WebReference -Actual $NativeCapture -Json
        return
    }

    & $diffScript -Reference $WebReference -Actual $NativeCapture
}
finally {
    if ($launchedProcess -and -not $launchedProcess.HasExited) {
        Stop-Process -Id $launchedProcess.Id -Force -ErrorAction SilentlyContinue
    }
}
