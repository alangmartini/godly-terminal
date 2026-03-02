$ErrorActionPreference = "Stop"

# ── Helpers ──────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   $msg" -ForegroundColor Green }

function Assert-ExitCode {
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`nRelease failed (exit code $LASTEXITCODE)." -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

# ── Read version from package.json ───────────────────────────────────────

$repoRoot = Split-Path $PSScriptRoot
$packageJson = Get-Content (Join-Path $repoRoot "package.json") -Raw | ConvertFrom-Json
$version = $packageJson.version
$tag = "v$version"

Write-Host "Godly Terminal release  $tag" -ForegroundColor Magenta

# ── Preflight checks ────────────────────────────────────────────────────

Write-Step "Running preflight checks..."

# gh CLI available?
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Host "   Error: gh CLI not found. Install from https://cli.github.com/" -ForegroundColor Red
    exit 1
}

# Authenticated?
gh auth status 2>&1 | Out-Null
Assert-ExitCode

# Release already exists?
$existing = gh release view $tag 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "   Error: Release $tag already exists. Bump the version first." -ForegroundColor Red
    Write-Host "   Run: node scripts/bump-version.mjs patch" -ForegroundColor Yellow
    exit 1
}

Write-Ok "gh authenticated, $tag not yet released"

# ── Locate installer ────────────────────────────────────────────────────

Write-Step "Locating installer..."

$bundleDir = Join-Path $repoRoot "src-tauri\target\release\bundle\nsis"
$installerName = "Godly Terminal_${version}_x64-setup.exe"
$installerPath = Join-Path $bundleDir $installerName

if (-not (Test-Path $installerPath)) {
    Write-Host "   Installer not found: $installerName" -ForegroundColor Yellow
    Write-Host "   Run 'pnpm tauri build' first, or 'scripts\production-build.ps1'" -ForegroundColor Yellow
    exit 1
}

$sizeMB = [math]::Round((Get-Item $installerPath).Length / 1MB, 1)
Write-Ok "Found $installerName ($sizeMB MB)"

# ── Extract release notes from CHANGELOG.md ─────────────────────────────

Write-Step "Extracting release notes..."

$changelog = Get-Content (Join-Path $repoRoot "CHANGELOG.md") -Raw

# Match the section for this version: everything between ## [X.Y.Z] and the next ## [
$pattern = "(?ms)## \[$([regex]::Escape($version))\][^\r\n]*\r?\n(.*?)(?=\r?\n## \[|$)"
$match = [regex]::Match($changelog, $pattern)

if ($match.Success -and $match.Groups[1].Value.Trim()) {
    $notes = $match.Groups[1].Value.Trim()
    $lineCount = ($notes -split "`n").Count
    Write-Ok "Extracted $lineCount lines from CHANGELOG.md"
} else {
    $notes = "Release $tag"
    Write-Host "   No changelog entry found for [$version], using default notes" -ForegroundColor Yellow
}

# ── Confirm ──────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "  Tag:       $tag" -ForegroundColor White
Write-Host "  Installer: $installerName ($sizeMB MB)" -ForegroundColor White
Write-Host "  Notes:" -ForegroundColor White
$notes -split "`n" | Select-Object -First 8 | ForEach-Object {
    Write-Host "    $_" -ForegroundColor DarkGray
}
if (($notes -split "`n").Count -gt 8) {
    Write-Host "    ... (truncated)" -ForegroundColor DarkGray
}
Write-Host ""

$confirm = Read-Host "Create GitHub release $tag? [y/N]"
if ($confirm -notmatch '^[Yy]') {
    Write-Host "Aborted." -ForegroundColor Yellow
    exit 0
}

# ── Create git tag if missing ────────────────────────────────────────────

Write-Step "Checking git tag..."

$existingTag = git tag --list $tag
if (-not $existingTag) {
    git tag $tag
    Write-Ok "Created tag $tag"
    git push origin $tag
    Assert-ExitCode
    Write-Ok "Pushed tag $tag"
} else {
    Write-Ok "Tag $tag already exists"
}

# ── Create GitHub release ────────────────────────────────────────────────

Write-Step "Creating GitHub release..."

# Write notes to a temp file to avoid shell escaping issues
$notesFile = [System.IO.Path]::GetTempFileName()
$notes | Out-File -FilePath $notesFile -Encoding utf8 -NoNewline

try {
    gh release create $tag $installerPath --title "Godly Terminal $tag" --notes-file $notesFile
    Assert-ExitCode
} finally {
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}

Write-Ok "Release created!"

# ── Done ─────────────────────────────────────────────────────────────────

$releaseUrl = "https://github.com/alangmartini/godly-terminal/releases/tag/$tag"
Write-Host "`nRelease $tag published." -ForegroundColor Green
Write-Host "  $releaseUrl" -ForegroundColor Cyan
Write-Host "  Latest: https://github.com/alangmartini/godly-terminal/releases/latest" -ForegroundColor Cyan
