$ErrorActionPreference = "Stop"

Write-Host "Switching to master..." -ForegroundColor Cyan
git checkout master
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Fetching and pulling latest changes..." -ForegroundColor Cyan
git pull origin master
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Building Tauri app (includes daemon, MCP, notify)..." -ForegroundColor Cyan
npm run tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "Production build complete." -ForegroundColor Green
