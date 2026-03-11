Get-ChildItem -Path "$PSScriptRoot\contracts\*.json" | ForEach-Object {
    $c = Get-Content $_.FullName | ConvertFrom-Json
    [PSCustomObject]@{
        ID          = $c.id
        Steps       = $c.steps.Count
        Fixture     = $c.fixture
        Restart     = $c.requires_restart
        Description = $c.description
    }
} | Format-Table -AutoSize
