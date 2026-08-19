[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,
    [string]$OutputDirectory = "installer-smoke",
    [ValidateRange(1, 10)]
    [int]$RunCount = 3,
    [ValidateRange(1, 1024)]
    [int]$MaxIdleWorkingSetMiB = 80
)

$ErrorActionPreference = "Stop"

$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$output = if ([IO.Path]::IsPathFullyQualified($OutputDirectory)) {
    [IO.Path]::GetFullPath($OutputDirectory)
} else {
    [IO.Path]::GetFullPath((Join-Path (Get-Location).Path $OutputDirectory))
}
$installDirectory = Join-Path $output "installed\Cakify"
$logsDirectory = Join-Path $output "logs"
$runtimeDirectory = Join-Path $output "runtime"
$installLog = Join-Path $logsDirectory "install.log"
$uninstallLog = Join-Path $logsDirectory "uninstall.log"
$summaryPath = Join-Path $output "SUMMARY.md"
New-Item -ItemType Directory -Force -Path $logsDirectory | Out-Null

$installExitCode = $null
$uninstallExitCode = $null
$installedApp = Join-Path $installDirectory "Cakify.exe"
$uninstaller = Join-Path $installDirectory "unins000.exe"
$failure = $null

try {
    $installArguments = @(
        "/VERYSILENT"
        "/SUPPRESSMSGBOXES"
        "/NORESTART"
        "/SP-"
        "/NOICONS"
        "/DIR=`"$installDirectory`""
        "/LOG=`"$installLog`""
    )
    $installProcess = Start-Process -FilePath $installer -ArgumentList $installArguments -Wait -PassThru
    $installExitCode = $installProcess.ExitCode
    if ($installExitCode -ne 0) {
        throw "Installer exited with code $installExitCode."
    }
    if (-not (Test-Path -LiteralPath $installedApp -PathType Leaf)) {
        throw "Installed executable is missing: $installedApp"
    }
    if (-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
        throw "Inno Setup uninstaller is missing: $uninstaller"
    }

    $runtimeSmoke = Join-Path $PSScriptRoot "runtime-smoke.ps1"
    & $runtimeSmoke `
        -AppPath $installedApp `
        -OutputDirectory $runtimeDirectory `
        -RunCount $RunCount `
        -ReadyTimeoutSeconds 15 `
        -IdleSeconds 3 `
        -SampleIntervalMs 250 `
        -ExitTimeoutSeconds 10 `
        -ExpectedWindowTitle Cakify `
        -MaxIdleWorkingSetMiB $MaxIdleWorkingSetMiB
} catch {
    $failure = $_
} finally {
    if (Test-Path -LiteralPath $uninstaller -PathType Leaf) {
        $uninstallArguments = @(
            "/VERYSILENT"
            "/SUPPRESSMSGBOXES"
            "/NORESTART"
            "/LOG=`"$uninstallLog`""
        )
        $uninstallProcess = Start-Process -FilePath $uninstaller -ArgumentList $uninstallArguments -Wait -PassThru
        $uninstallExitCode = $uninstallProcess.ExitCode
        if ($uninstallExitCode -ne 0 -and $null -eq $failure) {
            $failure = "Uninstaller exited with code $uninstallExitCode."
        }
    } elseif ($null -eq $failure) {
        $failure = "Uninstaller was not available after installation."
    }

    if ((Test-Path -LiteralPath $installedApp -PathType Leaf) -and $null -eq $failure) {
        $failure = "Installed executable remains after uninstall: $installedApp"
    }

    $passed = $null -eq $failure
    @(
        "# Cakify installer smoke"
        ""
        "- Installer: ``$([IO.Path]::GetFileName($installer))``"
        "- Install exit code: ``$installExitCode``"
        "- Installed runtime smoke: ``$(if (Test-Path -LiteralPath (Join-Path $runtimeDirectory 'SUMMARY.md')) { 'completed' } else { 'not completed' })``"
        "- Uninstall exit code: ``$uninstallExitCode``"
        "- Installed executable removed: ``$(-not (Test-Path -LiteralPath $installedApp -PathType Leaf))``"
        "- Passed: ``$passed``"
    ) | Set-Content -LiteralPath $summaryPath -Encoding utf8
}

if ($null -ne $failure) {
    throw $failure
}
