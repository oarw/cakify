[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("gpui", "avalonia", "flutter", "tauri")]
    [string]$Candidate,
    [Parameter(Mandatory = $true)]
    [string]$AppPath,
    [Parameter(Mandatory = $true)]
    [string]$CorePath,
    [ValidateRange(1, 10)]
    [int]$RunCount = 3,
    [ValidateRange(5, 300)]
    [int]$IdleSeconds = 60,
    [ValidateRange(100, 5000)]
    [int]$SampleIntervalMs = 1000,
    [string]$OutputDirectory = "results/benchmark"
)

$ErrorActionPreference = "Stop"
$app = (Resolve-Path -LiteralPath $AppPath).Path
$core = (Resolve-Path -LiteralPath $CorePath).Path
$output = [IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputDirectory))
$metricsDirectory = Join-Path $output "metrics"
$logsDirectory = Join-Path $output "logs"
$screenshotsDirectory = Join-Path $output "screenshots"
New-Item -ItemType Directory -Force -Path $metricsDirectory, $logsDirectory, $screenshotsDirectory | Out-Null

$manifest = Get-Content -LiteralPath "bench/fixtures/manifest.json" -Raw | ConvertFrom-Json
$samples = @()
$failures = @()

function Get-TreeIds([int]$RootPid) {
    $ids = New-Object System.Collections.Generic.HashSet[int]
    $queue = New-Object System.Collections.Generic.Queue[int]
    $queue.Enqueue($RootPid)
    while ($queue.Count -gt 0) {
        $current = $queue.Dequeue()
        if (-not $ids.Add($current)) { continue }
        Get-CimInstance Win32_Process -Filter "ParentProcessId = $current" -ErrorAction SilentlyContinue |
            ForEach-Object { $queue.Enqueue([int]$_.ProcessId) }
    }
    return @($ids)
}

function Get-TreeSnapshot([int]$RootPid, [int]$Index) {
    $rows = @()
    foreach ($treeProcessId in (Get-TreeIds $RootPid)) {
        try {
            $process = Get-Process -Id $treeProcessId -ErrorAction Stop
            $rows += [pscustomobject]@{
                pid = $treeProcessId
                name = $process.ProcessName
                working_set_bytes = [int64]$process.WorkingSet64
                private_bytes = [int64]$process.PrivateMemorySize64
                virtual_bytes = [int64]$process.VirtualMemorySize64
            }
        } catch {
            # Short-lived framework helpers can exit between enumeration and sampling.
        }
    }
    $workingSet = [int64](($rows | Measure-Object -Property working_set_bytes -Sum).Sum ?? 0)
    $privateBytes = [int64](($rows | Measure-Object -Property private_bytes -Sum).Sum ?? 0)
    $virtualBytes = [int64](($rows | Measure-Object -Property virtual_bytes -Sum).Sum ?? 0)
    return [pscustomobject]@{
        sample = $Index
        timestamp_utc = (Get-Date).ToUniversalTime().ToString("o")
        root_pid = $RootPid
        process_count = $rows.Count
        working_set_bytes = $workingSet
        private_bytes = $privateBytes
        virtual_bytes = $virtualBytes
        processes = $rows
    }
}

function Save-DesktopScreenshot([string]$Path) {
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = [System.Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Invoke-ProtocolProbe($Ready, [int]$RunIndex) {
    $headers = @{ "x-cakify-session" = [string]$Ready.session_token }
    $base = "http://127.0.0.1:$($Ready.port)"
    $health = Invoke-RestMethod -Uri "$base/health" -Headers $headers -TimeoutSec 5
    $page = Invoke-RestMethod -Uri "$base/fixture/messages?offset=0&limit=200" -Headers $headers -TimeoutSec 5
    $runId = "$Candidate-$RunIndex"
    $body = @{ run_id = $runId } | ConvertTo-Json -Compress
    $cancel = Invoke-RestMethod -Method Post -Uri "$base/run/cancel" -Headers $headers -ContentType "application/json" -Body $body -TimeoutSec 5
    $stream = Invoke-StreamProbe -Ready $Ready -RunId "$Candidate-stream-$RunIndex"
    return [pscustomobject]@{
        ok = [bool]$health.ok -and ([string]$page.fixture_hash -eq [string]$manifest.fixture_hash) -and [bool]$cancel.accepted -and [bool]$stream.ok
        health_fixture_hash = [string]$health.fixture_hash
        page_count = @($page.messages).Count
        page_total = [int]$page.total
        cancel_accepted = [bool]$cancel.accepted
        stream = $stream
    }
}

function Invoke-StreamProbe($Ready, [string]$RunId) {
    $base = "http://127.0.0.1:$($Ready.port)"
    $headers = @{ "x-cakify-session" = [string]$Ready.session_token }
    $job = Start-Job -ArgumentList "$base/run/events?run_id=$RunId&scenario=cancel", ([string]$Ready.session_token) -ScriptBlock {
        param($Url, $Token)
        try {
            $response = Invoke-WebRequest -Uri $Url -Headers @{ "x-cakify-session" = $Token } -TimeoutSec 15
            [pscustomobject]@{ ok = $true; content = [string]$response.Content }
        } catch {
            [pscustomobject]@{ ok = $false; content = $_.Exception.Message }
        }
    }
    Start-Sleep -Milliseconds 250
    $cancelBody = @{ run_id = $RunId } | ConvertTo-Json -Compress
    $cancelAccepted = $false
    try {
        $cancel = Invoke-RestMethod -Method Post -Uri "$base/run/cancel" -Headers $headers -ContentType "application/json" -Body $cancelBody -TimeoutSec 5
        $cancelAccepted = [bool]$cancel.accepted
    } catch {
        Stop-Job -Job $job -ErrorAction SilentlyContinue
    }
    Wait-Job -Job $job -Timeout 20 | Out-Null
    $payload = Receive-Job -Job $job -ErrorAction SilentlyContinue | Select-Object -Last 1
    Remove-Job -Job $job -Force -ErrorAction SilentlyContinue
    $content = if ($payload) { [string]$payload.content } else { "" }
    [pscustomobject]@{
        ok = $cancelAccepted -and ($content -match 'event: ready') -and ($content -match 'event: cancelled')
        cancel_accepted = $cancelAccepted
        saw_ready = $content -match 'event: ready'
        saw_cancelled = $content -match 'event: cancelled'
    }
}

for ($runIndex = 0; $runIndex -lt $RunCount; $runIndex++) {
    $notes = New-Object System.Collections.Generic.List[string]
    $readyFile = Join-Path $logsDirectory "core-ready-$runIndex.json"
    $treeLog = Join-Path $metricsDirectory "process-tree-$runIndex.jsonl"
    if (Test-Path -LiteralPath $readyFile) { Remove-Item -LiteralPath $readyFile -Force }
    if (Test-Path -LiteralPath $treeLog) { Remove-Item -LiteralPath $treeLog -Force }
    $process = $null
    $startupMs = 0.0
    $readyMs = 0.0
    $idleWorkingSet = [int64]0
    $idlePrivateBytes = [int64]0
    $peakWorkingSet = [int64]0
    $processCount = 1
    $protocolProbe = $null
    $windowReady = $false
    $runFailed = $false

    try {
        $startInfo = [Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $app
        $startInfo.UseShellExecute = $false
        $startInfo.WorkingDirectory = Split-Path -Parent $app
        $startInfo.ArgumentList.Add("--core-path")
        $startInfo.ArgumentList.Add($core)
        $startInfo.ArgumentList.Add("--core-ready-file")
        $startInfo.ArgumentList.Add($readyFile)

        $watch = [Diagnostics.Stopwatch]::StartNew()
        $process = [Diagnostics.Process]::Start($startInfo)
        if ($null -eq $process) { throw "application process did not start" }
        $startupMs = $watch.Elapsed.TotalMilliseconds

        while ($watch.Elapsed.TotalSeconds -lt 60) {
            $process.Refresh()
            if ($process.HasExited) { throw "application exited before ready with code $($process.ExitCode)" }
            if (-not $windowReady -and $process.MainWindowHandle -ne 0) { $windowReady = $true }
            if (Test-Path -LiteralPath $readyFile) { break }
            Start-Sleep -Milliseconds 100
        }
        $readyMs = $watch.Elapsed.TotalMilliseconds
        if (-not $windowReady) { $notes.Add("main_window_handle_not_observed") }
        if (-not (Test-Path -LiteralPath $readyFile)) { throw "core ready file was not created" }

        $ready = Get-Content -LiteralPath $readyFile -Raw | ConvertFrom-Json
        if ([string]$ready.fixture_hash -ne [string]$manifest.fixture_hash) {
            throw "fixture hash mismatch: $($ready.fixture_hash)"
        }
        $protocolProbe = Invoke-ProtocolProbe -Ready $ready -RunIndex $runIndex
        if (-not $protocolProbe.ok) { throw "protocol probe failed" }

        $sampleCount = [Math]::Max(1, [Math]::Ceiling(($IdleSeconds * 1000) / $SampleIntervalMs))
        for ($sampleIndex = 0; $sampleIndex -lt $sampleCount; $sampleIndex++) {
            $snapshot = Get-TreeSnapshot -RootPid $process.Id -Index $sampleIndex
            $snapshot | ConvertTo-Json -Depth 8 -Compress | Add-Content -LiteralPath $treeLog -Encoding utf8
            $idleWorkingSet = [int64]$snapshot.working_set_bytes
            $idlePrivateBytes = [int64]$snapshot.private_bytes
            $processCount = [Math]::Max(1, [int]$snapshot.process_count)
            if ($snapshot.working_set_bytes -gt $peakWorkingSet) { $peakWorkingSet = [int64]$snapshot.working_set_bytes }
            Start-Sleep -Milliseconds $SampleIntervalMs
        }

        if ($runIndex -eq 0) {
            try {
                Save-DesktopScreenshot -Path (Join-Path $screenshotsDirectory "light.png")
            } catch {
                $notes.Add("screenshot_failed: $($_.Exception.Message)")
            }
        }
    } catch {
        $runFailed = $true
        $message = $_.Exception.Message
        $notes.Add("run_failed: $message")
        $failures += "run ${runIndex}: $message"
    } finally {
        if (Test-Path -LiteralPath $readyFile) {
            Remove-Item -LiteralPath $readyFile -Force -ErrorAction SilentlyContinue
        }
        if ($null -ne $process) {
            try {
                if (-not $process.HasExited) { $process.Kill($true) }
                $process.WaitForExit(10000) | Out-Null
            } catch {
                $notes.Add("cleanup_error: $($_.Exception.Message)")
            }
        }
        Start-Sleep -Milliseconds 500
        $residual = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
            $_.ExecutablePath -eq $app -or $_.ExecutablePath -eq $core
        })
        if ($residual.Count -gt 0) {
            $runFailed = $true
            $notes.Add("residual_process_count=$($residual.Count)")
            foreach ($item in $residual) {
                Stop-Process -Id ([int]$item.ProcessId) -Force -ErrorAction SilentlyContinue
            }
        }
    }

    $samples += [ordered]@{
        run_index = $runIndex
        startup_ms = [Math]::Round($startupMs, 3)
        ready_ms = [Math]::Round($readyMs, 3)
        idle_working_set_bytes = $idleWorkingSet
        idle_private_bytes = $idlePrivateBytes
        peak_working_set_bytes = $peakWorkingSet
        process_count = $processCount
        window_ready = $windowReady
        protocol_probe = $protocolProbe
        failed = $runFailed
        notes = @($notes)
    }
}

$packageDirectory = Split-Path -Parent $app
$packageSize = [int64](Get-ChildItem -LiteralPath $packageDirectory -Recurse -File | Measure-Object -Property Length -Sum).Sum
$computer = Get-CimInstance Win32_ComputerSystem
$runner = [ordered]@{
    image = if ($env:ImageOS) { $env:ImageOS } else { "unknown" }
    image_version = if ($env:ImageVersion) { $env:ImageVersion } else { "unknown" }
    os_version = [Environment]::OSVersion.VersionString
    cpu = if ($env:PROCESSOR_IDENTIFIER) { $env:PROCESSOR_IDENTIFIER } else { "unknown" }
    logical_cores = [Environment]::ProcessorCount
    memory_bytes = [int64]$computer.TotalPhysicalMemory
}
$result = [ordered]@{
    schema_version = "result.v1"
    candidate = $Candidate
    fixture_hash = [string]$manifest.fixture_hash
    commit_sha = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { "local-source-only" }
    runner = $runner
    build_mode = "release"
    framework_version = if ($env:CAKIFY_FRAMEWORK_VERSION) { $env:CAKIFY_FRAMEWORK_VERSION } else { "unknown" }
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    package_size_bytes = $packageSize
    samples = $samples
    failures = $failures
}
$resultPath = Join-Path $metricsDirectory "result.json"
$result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $resultPath -Encoding utf8

[ordered]@{
    candidate = $Candidate
    app = $app
    core = $core
    run_count = $RunCount
    idle_seconds = $IdleSeconds
    framework_version = $result.framework_version
    generated_at_utc = $result.generated_at_utc
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $logsDirectory "environment.json") -Encoding utf8

if ($failures.Count -gt 0) {
    throw "$Candidate benchmark had $($failures.Count) failed run(s); diagnostics were written to $output"
}

$result | ConvertTo-Json -Depth 5 -Compress
