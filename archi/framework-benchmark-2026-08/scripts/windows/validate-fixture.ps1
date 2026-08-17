[CmdletBinding()]
param(
    [string]$ManifestPath = "bench/fixtures/manifest.json"
)

$ErrorActionPreference = "Stop"
$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json

$required = @(
    "protocol_version", "fixture_id", "fixture_hash", "message_count",
    "page_size", "stream_event_count", "stream_interval_ms", "tool_event_count",
    "image_asset", "visual_spec_version"
)
foreach ($name in $required) {
    if ($null -eq $manifest.$name -or [string]::IsNullOrWhiteSpace([string]$manifest.$name)) {
        throw "fixture manifest missing field: $name"
    }
}
if ([int]$manifest.message_count -ne 10000) { throw "message_count must remain 10000" }
if ([int]$manifest.page_size -le 0 -or [int]$manifest.page_size -gt 500) { throw "page_size out of range" }
if ([int]$manifest.stream_event_count -ne 30) { throw "stream_event_count must remain 30" }
if ([int]$manifest.stream_interval_ms -ne 1000) { throw "stream_interval_ms must remain 1000" }

$fixtureDirectory = Split-Path -Parent (Resolve-Path -LiteralPath $ManifestPath)
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $fixtureDirectory "../..")).Path
$assetPath = Join-Path $repoRoot $manifest.image_asset
if (-not (Test-Path -LiteralPath $assetPath)) {
    throw "fixture asset not found: $assetPath"
}

[pscustomobject]@{
    ok = $true
    fixture_id = $manifest.fixture_id
    fixture_hash = $manifest.fixture_hash
    message_count = [int]$manifest.message_count
} | ConvertTo-Json -Compress
