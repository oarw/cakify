[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [int]$RootPid,
    [int]$SampleCount = 60,
    [int]$IntervalMs = 1000,
    [string]$OutputPath = "results/process-tree.jsonl"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) | Out-Null

function Get-TreeIds([int]$ProcessId) {
    $ids = New-Object System.Collections.Generic.HashSet[int]
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($ProcessId)
    while ($queue.Count -gt 0) {
        $current = $queue.Dequeue()
        if (-not $ids.Add($current)) { continue }
        Get-CimInstance Win32_Process -Filter "ParentProcessId = $current" |
            ForEach-Object { $queue.Enqueue([int]$_.ProcessId) }
    }
    return @($ids)
}

Remove-Item -LiteralPath $OutputPath -Force -ErrorAction SilentlyContinue
for ($sample = 0; $sample -lt $SampleCount; $sample++) {
    $rows = @()
    foreach ($treeProcessId in (Get-TreeIds $RootPid)) {
        try {
            $process = Get-Process -Id $treeProcessId -ErrorAction Stop
            $rows += [pscustomobject]@{
                pid = $treeProcessId
                name = $process.ProcessName
                working_set = [int64]$process.WorkingSet64
                private_bytes = [int64]$process.PrivateMemorySize64
                virtual_bytes = [int64]$process.VirtualMemorySize64
            }
        } catch {
            # A short-lived child can exit between enumeration and sampling.
        }
    }
    [pscustomobject]@{
        sample = $sample
        timestamp_utc = (Get-Date).ToUniversalTime().ToString("o")
        root_pid = $RootPid
        process_count = $rows.Count
        processes = $rows
    } | ConvertTo-Json -Depth 6 -Compress | Add-Content -LiteralPath $OutputPath -Encoding utf8
    Start-Sleep -Milliseconds $IntervalMs
}
