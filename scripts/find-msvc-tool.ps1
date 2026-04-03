param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("linker", "cl")]
    [string]$Tool
)

$ErrorActionPreference = "Stop"

function Get-OverridePath {
    $override = switch ($Tool) {
        "linker" { $env:GODLY_MSVC_LINKER }
        "cl" { $env:GODLY_MSVC_CL }
    }

    if ($override -and (Test-Path -LiteralPath $override)) {
        return (Resolve-Path -LiteralPath $override).Path
    }

    return $null
}

function Get-VsWherePath {
    $roots = @(
        ${env:ProgramFiles(x86)},
        $env:ProgramFiles
    ) | Where-Object { $_ }

    foreach ($root in $roots | Select-Object -Unique) {
        $candidate = Join-Path $root "Microsoft Visual Studio\Installer\vswhere.exe"
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    return $null
}

function Convert-ToVersion {
    param([string]$Value)

    try {
        return [version]$Value
    } catch {
        return [version]"0.0"
    }
}

function Get-VsWhereCandidates {
    $exeName = switch ($Tool) {
        "linker" { "link.exe" }
        "cl" { "cl.exe" }
    }

    $vsWhere = Get-VsWherePath
    if (-not $vsWhere) {
        return @()
    }

    $rawInstances = & $vsWhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json 2>$null
    if (-not $rawInstances) {
        return @()
    }

    $instances = @($rawInstances | ConvertFrom-Json)
    $candidates = foreach ($instance in $instances) {
        $toolRoot = Join-Path $instance.installationPath "VC\Tools\MSVC"
        if (-not (Test-Path -LiteralPath $toolRoot)) {
            continue
        }

        foreach ($toolsetDir in Get-ChildItem -LiteralPath $toolRoot -Directory -ErrorAction SilentlyContinue) {
            $candidatePath = Join-Path $toolsetDir.FullName "bin\Hostx64\x64\$exeName"
            if (Test-Path -LiteralPath $candidatePath) {
                [pscustomobject]@{
                    Path = (Resolve-Path -LiteralPath $candidatePath).Path
                    ToolsetVersion = Convert-ToVersion $toolsetDir.Name
                    InstallVersion = Convert-ToVersion $instance.installationVersion
                }
            }
        }
    }

    return @($candidates | Sort-Object ToolsetVersion, InstallVersion -Descending)
}

function Get-PathFallback {
    $exeName = switch ($Tool) {
        "linker" { "link.exe" }
        "cl" { "cl.exe" }
    }

    $command = Get-Command $exeName -CommandType Application -ErrorAction SilentlyContinue
    if ($command -and $command.Source) {
        return $command.Source
    }

    return $null
}

$resolvedPath = Get-OverridePath

if (-not $resolvedPath) {
    $vsWhereMatch = Get-VsWhereCandidates | Select-Object -First 1
    if ($vsWhereMatch) {
        $resolvedPath = $vsWhereMatch.Path
    }
}

if (-not $resolvedPath) {
    $resolvedPath = Get-PathFallback
}

if (-not $resolvedPath) {
    $toolName = switch ($Tool) {
        "linker" { "link.exe" }
        "cl" { "cl.exe" }
    }

    Write-Error "Unable to resolve $toolName. Install Visual Studio Build Tools with Microsoft.VisualStudio.Component.VC.Tools.x86.x64, or set GODLY_MSVC_$($Tool.ToUpperInvariant()) to an explicit path."
}

Write-Output $resolvedPath
