# destroy.ps1 — Kill all Godly Terminal processes

$processes = @(
    "godly-terminal"
    "godly-daemon"
    "godly-pty-shim"
    "godly-mcp"
    "godly-notify"
    "godly-remote"
    "godly-whisper"
)

$killed = 0

foreach ($name in $processes) {
    $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
    if ($procs) {
        $procs | ForEach-Object {
            Write-Host "Killing $name (PID $($_.Id))"
            Stop-Process -Id $_.Id -Force
            $killed++
        }
    }
}

if ($killed -eq 0) {
    Write-Host "No Godly Terminal processes found."
} else {
    Write-Host "`nKilled $killed process(es)."
}
