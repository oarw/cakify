[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AppPath,
    [string]$OutputDirectory = "runtime-smoke",
    [ValidateRange(1, 10)]
    [int]$RunCount = 3,
    [ValidateRange(5, 60)]
    [int]$ReadyTimeoutSeconds = 15,
    [ValidateRange(1, 30)]
    [int]$IdleSeconds = 3,
    [ValidateRange(100, 5000)]
    [int]$SampleIntervalMs = 250,
    [ValidateRange(5, 60)]
    [int]$ExitTimeoutSeconds = 10,
    [ValidateNotNullOrEmpty()]
    [string]$ExpectedWindowTitle = "Cakify",
    [ValidateRange(1, 1024)]
    [int]$MaxIdleWorkingSetMiB = 80,
    [ValidateSet("chat", "settings-provider", "settings-mcp")]
    [string[]]$StartupViews = @("chat")
)

$ErrorActionPreference = "Stop"

$app = (Resolve-Path -LiteralPath $AppPath).Path
$output = if ([IO.Path]::IsPathFullyQualified($OutputDirectory)) {
    [IO.Path]::GetFullPath($OutputDirectory)
} else {
    [IO.Path]::GetFullPath((Join-Path (Get-Location).Path $OutputDirectory))
}
$metricsDirectory = Join-Path $output "metrics"
$logsDirectory = Join-Path $output "logs"
$screenshotsDirectory = Join-Path $output "screenshots"
New-Item -ItemType Directory -Force -Path $metricsDirectory, $logsDirectory, $screenshotsDirectory | Out-Null

if ($StartupViews.Count -ne 1 -and $StartupViews.Count -ne $RunCount) {
    throw "StartupViews must contain one view for every run or exactly one default view; got $($StartupViews.Count) for $RunCount runs."
}

function Get-StartupView([int]$RunIndex) {
    if ($StartupViews.Count -eq 1) {
        return $StartupViews[0]
    }
    return $StartupViews[$RunIndex]
}

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

function Get-TreeSnapshot([int]$RootPid, [int]$Index, [string]$StartupView) {
    $rows = @()
    foreach ($treeProcessId in (Get-TreeIds $RootPid)) {
        try {
            $process = Get-Process -Id $treeProcessId -ErrorAction Stop
            $rows += [ordered]@{
                pid = $treeProcessId
                name = $process.ProcessName
                working_set_bytes = [int64]$process.WorkingSet64
                private_bytes = [int64]$process.PrivateMemorySize64
                virtual_bytes = [int64]$process.VirtualMemorySize64
            }
        } catch {
            # A short-lived child can exit between enumeration and sampling.
        }
    }

    $workingSetBytes = [int64]0
    $privateBytes = [int64]0
    $virtualBytes = [int64]0
    foreach ($row in $rows) {
        $workingSetBytes += [int64]$row.working_set_bytes
        $privateBytes += [int64]$row.private_bytes
        $virtualBytes += [int64]$row.virtual_bytes
    }

    return [ordered]@{
        sample = $Index
        startup_view = $StartupView
        timestamp_utc = (Get-Date).ToUniversalTime().ToString("o")
        root_pid = $RootPid
        process_count = $rows.Count
        working_set_bytes = $workingSetBytes
        private_bytes = $privateBytes
        virtual_bytes = $virtualBytes
        processes = $rows
    }
}

function Get-Median([int64[]]$Values) {
    if ($Values.Count -eq 0) { return [int64]0 }
    $sorted = @($Values | Sort-Object)
    $middle = [Math]::Floor($sorted.Count / 2)
    if (($sorted.Count % 2) -eq 1) { return [int64]$sorted[$middle] }
    return [int64](($sorted[$middle - 1] + $sorted[$middle]) / 2)
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

function Get-WindowPlacement([int64]$Handle) {
    Add-Type -AssemblyName System.Windows.Forms
    if ($null -eq ("Cakify.NativeWindow" -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Cakify {
    public static class NativeWindow {
        [StructLayout(LayoutKind.Sequential)]
        public struct Rect {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool GetWindowRect(IntPtr handle, out Rect rect);

        public static Rect ReadBounds(IntPtr handle) {
            if (!GetWindowRect(handle, out Rect rect)) {
                throw new Win32Exception(Marshal.GetLastWin32Error());
            }
            return rect;
        }
    }
}
"@
    }

    $window = [Cakify.NativeWindow]::ReadBounds([IntPtr]$Handle)
    $workArea = [System.Windows.Forms.Screen]::FromHandle([IntPtr]$Handle).WorkingArea
    $fullyVisible =
        $window.Left -ge $workArea.Left -and
        $window.Top -ge $workArea.Top -and
        $window.Right -le $workArea.Right -and
        $window.Bottom -le $workArea.Bottom

    return [ordered]@{
        window = [ordered]@{
            left = $window.Left
            top = $window.Top
            right = $window.Right
            bottom = $window.Bottom
            width = $window.Right - $window.Left
            height = $window.Bottom - $window.Top
        }
        work_area = [ordered]@{
            left = $workArea.Left
            top = $workArea.Top
            right = $workArea.Right
            bottom = $workArea.Bottom
            width = $workArea.Width
            height = $workArea.Height
        }
        fully_visible = $fullyVisible
    }
}

$runs = @()
$failures = New-Object System.Collections.Generic.List[string]
$maxIdleWorkingSetBytes = [int64]$MaxIdleWorkingSetMiB * 1MB

for ($runIndex = 0; $runIndex -lt $RunCount; $runIndex++) {
    $startupView = Get-StartupView -RunIndex $runIndex
    $notes = New-Object System.Collections.Generic.List[string]
    $process = $null
    $watch = $null
    $readyMs = 0.0
    $mainWindowHandle = [int64]0
    $mainWindowTitle = ""
    $windowBounds = $null
    $workAreaBounds = $null
    $windowFullyVisible = $false
    $closeRequested = $false
    $exitMs = 0.0
    $exitCode = $null
    $snapshots = @()
    $observedChildIds = New-Object System.Collections.Generic.HashSet[int]
    $gateFailures = New-Object System.Collections.Generic.List[string]
    $runFailed = $false
    $stdoutPath = Join-Path $logsDirectory "stdout-$runIndex.log"
    $stderrPath = Join-Path $logsDirectory "stderr-$runIndex.log"
    $screenshotName = "desktop-$runIndex-$startupView.png"
    $screenshotPath = Join-Path $screenshotsDirectory $screenshotName

    try {
        $startInfo = [Diagnostics.ProcessStartInfo]::new()
        $startInfo.FileName = $app
        $startInfo.UseShellExecute = $false
        $startInfo.WorkingDirectory = Split-Path -Parent $app
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $startInfo.Environment["CAKIFY_SMOKE_VIEW"] = $startupView

        $watch = [Diagnostics.Stopwatch]::StartNew()
        $process = [Diagnostics.Process]::Start($startInfo)
        if ($null -eq $process) { throw "application process did not start" }

        while ($watch.Elapsed.TotalSeconds -lt $ReadyTimeoutSeconds) {
            $process.Refresh()
            if ($process.HasExited) {
                throw "application exited before opening a window with code $($process.ExitCode)"
            }
            if ($process.MainWindowHandle -ne 0) {
                $mainWindowHandle = [int64]$process.MainWindowHandle
                $mainWindowTitle = [string]$process.MainWindowTitle
                if ($mainWindowTitle -eq $ExpectedWindowTitle) { break }
            }
            Start-Sleep -Milliseconds 50
        }

        $readyMs = $watch.Elapsed.TotalMilliseconds
        if ($mainWindowHandle -eq 0) {
            throw "main window handle was not observed within $ReadyTimeoutSeconds seconds"
        }
        if ($mainWindowTitle -ne $ExpectedWindowTitle) {
            throw "window title '$mainWindowTitle' did not match expected title '$ExpectedWindowTitle' within $ReadyTimeoutSeconds seconds"
        }

        $placement = Get-WindowPlacement -Handle $mainWindowHandle
        $windowBounds = $placement.window
        $workAreaBounds = $placement.work_area
        $windowFullyVisible = $placement.fully_visible
        if (-not $windowFullyVisible) {
            $gateFailures.Add(
                "window bounds $($windowBounds.left),$($windowBounds.top),$($windowBounds.right),$($windowBounds.bottom) " +
                "were outside work area $($workAreaBounds.left),$($workAreaBounds.top),$($workAreaBounds.right),$($workAreaBounds.bottom)"
            )
        }

        try {
            Save-DesktopScreenshot -Path $screenshotPath
            if (-not (Test-Path -LiteralPath $screenshotPath -PathType Leaf)) {
                $gateFailures.Add("screenshot was not created: $screenshotPath")
            }
        } catch {
            $gateFailures.Add("screenshot_failed: $($_.Exception.Message)")
        }

        $sampleCount = [Math]::Max(1, [Math]::Ceiling(($IdleSeconds * 1000) / $SampleIntervalMs))
        for ($sampleIndex = 0; $sampleIndex -lt $sampleCount; $sampleIndex++) {
            $snapshot = Get-TreeSnapshot -RootPid $process.Id -Index $sampleIndex -StartupView $startupView
            $snapshots += $snapshot
            $snapshot | ConvertTo-Json -Depth 8 -Compress |
                Add-Content -LiteralPath (Join-Path $metricsDirectory "process-tree-$runIndex.jsonl") -Encoding utf8
            foreach ($row in $snapshot.processes) {
                if ([int]$row.pid -ne $process.Id) { $observedChildIds.Add([int]$row.pid) | Out-Null }
            }
            Start-Sleep -Milliseconds $SampleIntervalMs
        }

        $workingSets = @($snapshots | ForEach-Object { [int64]$_.working_set_bytes })
        $idleWorkingSet = Get-Median -Values $workingSets
        if ($observedChildIds.Count -ne 0) {
            $gateFailures.Add("default process tree spawned $($observedChildIds.Count) child process(es): $(@($observedChildIds) -join ', ')")
        }
        if ($idleWorkingSet -gt $maxIdleWorkingSetBytes) {
            $gateFailures.Add("median idle working set $idleWorkingSet bytes exceeded the $MaxIdleWorkingSetMiB MiB M0 gate")
        }

        $closeWatch = [Diagnostics.Stopwatch]::StartNew()
        $closeRequested = $process.CloseMainWindow()
        if (-not $closeRequested) { throw "CloseMainWindow did not find a closeable top-level window" }
        if (-not $process.WaitForExit($ExitTimeoutSeconds * 1000)) {
            throw "application did not exit within $ExitTimeoutSeconds seconds after WM_CLOSE"
        }
        $exitMs = $closeWatch.Elapsed.TotalMilliseconds
        $exitCode = $process.ExitCode
        if ($exitCode -ne 0) { throw "application exited with code $exitCode after WM_CLOSE" }
        if ($gateFailures.Count -gt 0) { throw ($gateFailures -join "; ") }
    } catch {
        $runFailed = $true
        $message = $_.Exception.Message
        $notes.Add("run_failed: $message")
        $failures.Add("run ${runIndex}: $message")
    } finally {
        if ($null -ne $process) {
            try {
                if (-not $process.HasExited) {
                    $notes.Add("forced_cleanup_after_failure")
                    $process.Kill($true)
                    $process.WaitForExit(10000) | Out-Null
                }
                $process.StandardOutput.ReadToEnd() | Set-Content -LiteralPath $stdoutPath -Encoding utf8
                $process.StandardError.ReadToEnd() | Set-Content -LiteralPath $stderrPath -Encoding utf8
            } catch {
                $notes.Add("cleanup_error: $($_.Exception.Message)")
            }
        }

        Start-Sleep -Milliseconds 500
        $residual = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
            $_.ExecutablePath -eq $app
        })
        if ($residual.Count -gt 0) {
            $runFailed = $true
            $message = "residual_process_count=$($residual.Count)"
            $notes.Add($message)
            $failures.Add("run ${runIndex}: $message")
            foreach ($item in $residual) {
                Stop-Process -Id ([int]$item.ProcessId) -Force -ErrorAction SilentlyContinue
            }
        }
    }

    $workingSets = @($snapshots | ForEach-Object { [int64]$_.working_set_bytes })
    $privateBytes = @($snapshots | ForEach-Object { [int64]$_.private_bytes })
    $runs += [ordered]@{
        run_index = $runIndex
        startup_view = $startupView
        screenshot = if (Test-Path -LiteralPath $screenshotPath -PathType Leaf) {
            "screenshots/$screenshotName"
        } else {
            $null
        }
        ready_ms = [Math]::Round($readyMs, 3)
        main_window_handle = $mainWindowHandle
        main_window_title = $mainWindowTitle
        window_bounds = $windowBounds
        work_area_bounds = $workAreaBounds
        window_fully_visible = $windowFullyVisible
        sample_count = $snapshots.Count
        idle_working_set_bytes = Get-Median -Values $workingSets
        peak_working_set_bytes = [int64](($workingSets | Measure-Object -Maximum).Maximum ?? 0)
        idle_private_bytes = Get-Median -Values $privateBytes
        max_process_count = [int](($snapshots | ForEach-Object { $_.process_count } | Measure-Object -Maximum).Maximum ?? 0)
        observed_child_process_ids = @($observedChildIds)
        close_requested = $closeRequested
        exit_ms = [Math]::Round($exitMs, 3)
        exit_code = $exitCode
        failed = $runFailed
        notes = @($notes)
    }
}

$computer = Get-CimInstance Win32_ComputerSystem
$result = [ordered]@{
    schema_version = "runtime-smoke.v2"
    commit_sha = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { "local-source-only" }
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
    app_path = $app
    app_size_bytes = [int64](Get-Item -LiteralPath $app).Length
    runner = [ordered]@{
        image = if ($env:ImageOS) { $env:ImageOS } else { "unknown" }
        image_version = if ($env:ImageVersion) { $env:ImageVersion } else { "unknown" }
        os_version = [Environment]::OSVersion.VersionString
        cpu = if ($env:PROCESSOR_IDENTIFIER) { $env:PROCESSOR_IDENTIFIER } else { "unknown" }
        logical_cores = [Environment]::ProcessorCount
        memory_bytes = [int64]$computer.TotalPhysicalMemory
    }
    gates = [ordered]@{
        run_count = $RunCount
        startup_views = @($StartupViews)
        ready_timeout_seconds = $ReadyTimeoutSeconds
        idle_seconds = $IdleSeconds
        max_idle_working_set_mib = $MaxIdleWorkingSetMiB
        expected_window_title = $ExpectedWindowTitle
        expected_window_fully_visible = $true
        expected_child_process_count = 0
        exit_timeout_seconds = $ExitTimeoutSeconds
    }
    passed = $failures.Count -eq 0
    runs = $runs
    failures = @($failures)
}

$resultPath = Join-Path $metricsDirectory "result.json"
$result | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $resultPath -Encoding utf8

$summary = @(
    "# Cakify Windows runtime smoke"
    ""
    "- Commit: ``$($result.commit_sha)``"
    "- Passed: ``$($result.passed)``"
    "- Runs: ``$RunCount``"
    "- Startup views: ``$($StartupViews -join ', ')``"
    "- Idle Working Set gate: ``$MaxIdleWorkingSetMiB MiB``"
    "- Window fully visible gate: ``true``"
    "- Default child-process gate: ``0``"
    ""
    "| Run | Startup view | Screenshot | Window ready (ms) | Visible | Idle WS (MiB) | Peak WS (MiB) | Processes | Close/exit (ms) | Result |"
    "| ---: | --- | --- | ---: | :---: | ---: | ---: | ---: | ---: | --- |"
)
foreach ($run in $runs) {
    $summary += "| $($run.run_index) | ``$($run.startup_view)`` | ``$($run.screenshot)`` | $($run.ready_ms) | $($run.window_fully_visible) | $([Math]::Round($run.idle_working_set_bytes / 1MB, 3)) | $([Math]::Round($run.peak_working_set_bytes / 1MB, 3)) | $($run.max_process_count) | $($run.exit_ms) | $(if ($run.failed) { 'failed' } else { 'passed' }) |"
}
$summary | Set-Content -LiteralPath (Join-Path $output "SUMMARY.md") -Encoding utf8

$result | ConvertTo-Json -Depth 6 -Compress
if ($failures.Count -gt 0) {
    throw "runtime smoke had $($failures.Count) failure(s); diagnostics were written to $output"
}
