param(
    [string]$Spec,
    [switch]$Build
)

if (-not $Build) {
    $env:SKIP_BUILD = "1"
}

$args_ = @("wdio", "e2e/wdio.conf.ts")

if ($Spec) {
    $args_ += "--spec"
    $args_ += $Spec
}

npx @args_

if (-not $Build) {
    Remove-Item Env:\SKIP_BUILD
}
