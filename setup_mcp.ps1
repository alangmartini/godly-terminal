$ErrorActionPreference = "Stop"

$mcpBinary = "$PSScriptRoot\src-tauri\target\release\godly-mcp.exe"

if (-not (Test-Path $mcpBinary)) {
    Write-Host "godly-mcp.exe not found at $mcpBinary" -ForegroundColor Red
    Write-Host "Run 'npm run build:mcp:release' first." -ForegroundColor Yellow
    exit 1
}

Write-Host "Adding godly-terminal MCP server to Claude Code..." -ForegroundColor Cyan
claude mcp add godly-terminal -- $mcpBinary

if ($LASTEXITCODE -eq 0) {
    Write-Host "Done. godly-terminal MCP server added." -ForegroundColor Green
} else {
    Write-Host "Failed to add MCP server (exit code $LASTEXITCODE)." -ForegroundColor Red
    exit $LASTEXITCODE
}
