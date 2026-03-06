# Find the Godly Terminal process
$godly = Get-WmiObject Win32_Process | Where-Object { $_.ExecutablePath -like '*Godly*' -or $_.ExecutablePath -like '*godly*' }
if ($godly) {
    foreach ($p in $godly) {
        Write-Output "FOUND: PID=$($p.ProcessId) Name=$($p.Name) Path=$($p.ExecutablePath)"
    }
} else {
    Write-Output "No Godly Terminal process found"
}

# Also check for any tauri-related process
$tauri = Get-WmiObject Win32_Process | Where-Object { $_.ExecutablePath -like '*tauri*' }
if ($tauri) {
    foreach ($p in $tauri) {
        Write-Output "TAURI: PID=$($p.ProcessId) Name=$($p.Name) Path=$($p.ExecutablePath)"
    }
}

# Check webview2 processes that are NOT SearchHost
$wv = Get-WmiObject Win32_Process -Filter "name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -notlike '*SearchHost*' -and $_.CommandLine -notlike '*Teams*' -and $_.CommandLine -notlike '*Widgets*' }
if ($wv) {
    Write-Output "`nGodly WebView2 processes:"
    foreach ($p in $wv) {
        $cmdShort = $p.CommandLine.Substring(0, [Math]::Min(300, $p.CommandLine.Length))
        Write-Output "  PID=$($p.ProcessId): $cmdShort"
    }
} else {
    Write-Output "`nNo non-system WebView2 processes found"
}
