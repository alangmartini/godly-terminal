<#
.SYNOPSIS
  Signal the OMX visual parity loop to stop after the current iteration.
#>

param(
    [string]$ArtifactsRoot = (Join-Path $PSScriptRoot ".omx-visual-parity-loop")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ArtifactsRoot = [System.IO.Path]::GetFullPath($ArtifactsRoot)
$stopFile = Join-Path $ArtifactsRoot "STOP"

New-Item -ItemType Directory -Force -Path $ArtifactsRoot | Out-Null
New-Item -ItemType File -Force -Path $stopFile | Out-Null

Write-Host "Stop signal written to $stopFile" -ForegroundColor Yellow
Write-Host "The loop will exit gracefully after the current iteration." -ForegroundColor Yellow
