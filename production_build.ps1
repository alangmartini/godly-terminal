$ErrorActionPreference = "Stop"

Write-Host "Building daemon (release)..." -ForegroundColor Cyan
npm run build:daemon:release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Building MCP server (release)..." -ForegroundColor Cyan
npm run build:mcp:release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Building Tauri app..." -ForegroundColor Cyan
npm run tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Production build complete." -ForegroundColor Green
