[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("gpui", "avalonia", "flutter", "tauri")]
    [string]$Candidate,
    [int]$RunCount = 3,
    [string]$OutputDirectory = "results/scaffold"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$manifest = Get-Content -LiteralPath "bench/fixtures/manifest.json" -Raw | ConvertFrom-Json
$record = [ordered]@{
    schema_version = "scaffold.v1"
    status = "scaffold_only"
    candidate = $Candidate
    requested_runs = $RunCount
    fixture_id = $manifest.fixture_id
    fixture_hash = $manifest.fixture_hash
    commit_sha = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { "local-scaffold" }
    runner_image = if ($env:ImageOS) { $env:ImageOS } else { "unknown" }
    runner_name = if ($env:RUNNER_NAME) { $env:RUNNER_NAME } else { "unknown" }
    os_version = [Environment]::OSVersion.VersionString
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    note = "UI shell is not implemented yet; this artifact proves workflow wiring only."
}
$path = Join-Path $OutputDirectory "$Candidate-scaffold.json"
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $path -Encoding utf8
$record | ConvertTo-Json -Compress
