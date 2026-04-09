param(
    [string]$OutPath = "C:\Users\alanm\Documents\dev\godly-claude\godly-terminal\docs\references\web-reference.png",
    [string]$Url = "http://[::1]:5199",
    [int]$Port = 5199,
    [int]$ViewportWidth = 1920,
    [int]$ViewportHeight = 1080,
    [string]$SessionName = "godly-web-reference"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$env:PYTHONIOENCODING = "utf-8"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function Get-UrlCandidates {
    param(
        [string]$SeedUrl,
        [int]$Port
    )

    $ordered = [System.Collections.Generic.List[string]]::new()
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)

    function Add-Candidate([string]$Candidate) {
        if ([string]::IsNullOrWhiteSpace($Candidate)) {
            return
        }
        $normalized = $Candidate.TrimEnd("/")
        if ($seen.Add($normalized)) {
            [void]$ordered.Add($normalized)
        }
    }

    try {
        $parsed = [System.Uri]$SeedUrl
        if ($parsed.Host -in @("localhost", "127.0.0.1", "::1")) {
            foreach ($host in @("127.0.0.1", "[::1]", "localhost")) {
                $builder = [System.UriBuilder]::new($parsed)
                $builder.Host = if ($host -eq "[::1]") { "::1" } else { $host }
                $builder.Port = if ($parsed.Port -gt 0) { $parsed.Port } else { $Port }
                Add-Candidate $builder.Uri.AbsoluteUri
            }
        } else {
            Add-Candidate $SeedUrl
        }
    } catch {
        Add-Candidate $SeedUrl
    }

    foreach ($candidate in @(
        "http://127.0.0.1:$Port",
        "http://[::1]:$Port",
        "http://localhost:$Port"
    )) {
        Add-Candidate $candidate
    }

    return $ordered
}

function Resolve-HealthyUrl {
    param(
        [string[]]$Candidates,
        [int]$TimeoutSeconds = 20
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        foreach ($candidate in $Candidates) {
            try {
                $response = Invoke-WebRequest -Uri $candidate -UseBasicParsing -TimeoutSec 2
                if (
                    $response.StatusCode -ge 200 -and
                    $response.StatusCode -lt 500 -and
                    $response.Content -match "Godly Terminal"
                ) {
                    return $candidate.TrimEnd("/")
                }
            } catch {
                Start-Sleep -Milliseconds 200
            }
        }

        Start-Sleep -Milliseconds 200
    }

    return $null
}

function Resize-ImageToTarget {
    param(
        [string]$Path,
        [int]$TargetWidth,
        [int]$TargetHeight
    )

    Add-Type -AssemblyName System.Drawing

    $source = [System.Drawing.Image]::FromFile($Path)
    try {
        if ($source.Width -eq $TargetWidth -and $source.Height -eq $TargetHeight) {
            return $false
        }

        $bitmap = New-Object System.Drawing.Bitmap($TargetWidth, $TargetHeight)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
            $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
            if ($source.Width -ge $TargetWidth -and $source.Height -ge $TargetHeight) {
                $srcRect = [System.Drawing.Rectangle]::new(0, 0, $TargetWidth, $TargetHeight)
                $dstRect = [System.Drawing.Rectangle]::new(0, 0, $TargetWidth, $TargetHeight)
                $graphics.DrawImage($source, $dstRect, $srcRect, [System.Drawing.GraphicsUnit]::Pixel)
            } else {
                $sourceRatio = $source.Width / [double]$source.Height
                $targetRatio = $TargetWidth / [double]$TargetHeight
                if ([Math]::Abs($sourceRatio - $targetRatio) -gt 0.001) {
                    throw "Unexpected screenshot aspect ratio: $($source.Width)x$($source.Height) cannot be normalized to ${TargetWidth}x${TargetHeight}"
                }
                $graphics.DrawImage($source, 0, 0, $TargetWidth, $TargetHeight)
            }
        } finally {
            $graphics.Dispose()
        }
    } finally {
        $source.Dispose()
    }

    try {
        $tempPath = "$Path.tmp.png"
        $bitmap.Save($tempPath, [System.Drawing.Imaging.ImageFormat]::Png)
        Move-Item -LiteralPath $tempPath -Destination $Path -Force
        return $true
    } finally {
        $bitmap.Dispose()
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$webDir = (Resolve-Path (Join-Path $repoRoot "web")).Path
$OutPath = [System.IO.Path]::GetFullPath($OutPath)
$outDir = Split-Path -Parent $OutPath
if ($outDir) {
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
}

$startedServer = $null
$serverLog = Join-Path $env:TEMP "godly-web-reference-$Port.log"
$serverErrLog = Join-Path $env:TEMP "godly-web-reference-$Port.err.log"
$browserScript = $null
$resolvedUrl = $null
try {
    $candidates = Get-UrlCandidates -SeedUrl $Url -Port $Port
    $resolvedUrl = Resolve-HealthyUrl -Candidates $candidates -TimeoutSeconds 2

    if (-not $resolvedUrl) {
        $startedServer = Start-Process `
            -FilePath "pnpm.cmd" `
            -ArgumentList @("exec", "vite", "--host", "127.0.0.1", "--port", $Port.ToString()) `
            -WorkingDirectory $webDir `
            -PassThru `
            -WindowStyle Hidden `
            -RedirectStandardOutput $serverLog `
            -RedirectStandardError $serverErrLog

        $resolvedUrl = Resolve-HealthyUrl -Candidates $candidates -TimeoutSeconds 20
    }

    if (-not $resolvedUrl) {
        $stdout = if (Test-Path $serverLog) { Get-Content $serverLog -Raw } else { "" }
        $stderr = if (Test-Path $serverErrLog) { Get-Content $serverErrLog -Raw } else { "" }
        throw "Failed to resolve a healthy web reference URL. Stdout:`n$stdout`nStderr:`n$stderr"
    }

    $browserScript = Join-Path $env:TEMP "godly-web-reference-$PID.py"
    @"
page = browser._run(browser._session.get_current_page())
browser._run(page.set_viewport_size($ViewportWidth, $ViewportHeight))
browser.goto(r"$resolvedUrl")
browser.wait(1.0)
page = browser._run(browser._session.get_current_page())
body_text = browser._run(page.evaluate('(...args) => document.body.innerText'))
required_tokens = ["The Gardener of Broken Things", "Sessions 3", "opensessions"]
for token in required_tokens:
    if token not in body_text:
        raise RuntimeError(f"Expected token not found in web reference DOM: {token}")
"@ | Set-Content -LiteralPath $browserScript -Encoding utf8

    & browser-use --session $SessionName close 2>$null | Out-Null
    & browser-use --session $SessionName open $resolvedUrl | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "browser-use open failed for $resolvedUrl"
    }
    & browser-use --session $SessionName python --file $browserScript | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "browser-use python capture failed for $resolvedUrl"
    }
    & browser-use --session $SessionName screenshot $OutPath | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "browser-use screenshot failed for $resolvedUrl"
    }
    & browser-use --session $SessionName close | Out-Null

    if (-not (Test-Path $OutPath)) {
        throw "Browser capture completed without producing $OutPath"
    }

    $resized = Resize-ImageToTarget -Path $OutPath -TargetWidth $ViewportWidth -TargetHeight $ViewportHeight

    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Image]::FromFile($OutPath)
    try {
        if ($image.Width -ne $ViewportWidth -or $image.Height -ne $ViewportHeight) {
            throw "Unexpected screenshot size: $($image.Width)x$($image.Height) (expected ${ViewportWidth}x${ViewportHeight})"
        }
    } finally {
        $image.Dispose()
    }

    if ($resized) {
        Write-Host ("Web reference saved to {0} ({1}x{2}, normalized from browser output via {3})" -f $OutPath, $ViewportWidth, $ViewportHeight, $resolvedUrl)
    } else {
        Write-Host ("Web reference saved to {0} ({1}x{2}) via {3}" -f $OutPath, $ViewportWidth, $ViewportHeight, $resolvedUrl)
    }
}
finally {
    if ($browserScript) {
        Remove-Item -LiteralPath $browserScript -ErrorAction SilentlyContinue
    }
    if ($startedServer) {
        Stop-Process -Id $startedServer.Id -Force -ErrorAction SilentlyContinue
    }
}
