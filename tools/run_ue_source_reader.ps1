param(
    [string]$Asset,
    [string]$Output,
    [string]$Plugin = (Join-Path $PSScriptRoot "..\target\ue_graph_source_reader_package\HostProject\Plugins\AlsGraphSourceReader\AlsGraphSourceReader.uplugin"),
    [ValidateRange(1, 3600)]
    [int]$TimeoutSeconds = 300,
    [ValidateRange(1, 3600)]
    [int]$NoProgressSeconds = 150,
    [switch]$SelfTest
)

function ConvertTo-WindowsArgument {
    param([string]$Value)

    if ($Value.Length -eq 0) {
        return '""'
    }
    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    $escaped = [regex]::Replace($Value, '(\\*)"', '$1$1\\"')
    $escaped = [regex]::Replace($escaped, '(\\+)$', '$1$1')
    return '"' + $escaped + '"'
}

function Write-ReaderReport {
    param(
        [string]$Path,
        [object]$Value
    )

    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 6 -Compress) + "`n"),
        [Text.UTF8Encoding]::new($false)
    )
}

function Invoke-ProcessWithWatchdog {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList,
        [string]$ReportPath,
        [string]$StdoutPath,
        [string]$StderrPath,
        [string[]]$ProgressPaths,
        [int]$TimeoutSeconds,
        [int]$NoProgressSeconds
    )

    Set-Content -LiteralPath $StdoutPath -Value ""
    Set-Content -LiteralPath $StderrPath -Value ""
    $startedAt = Get-Date
    $lastProgressAt = $startedAt
    $lastProgressSource = "process_started"
    $argumentText = ($ArgumentList | ForEach-Object { ConvertTo-WindowsArgument $_ }) -join " "
    try {
        $process = Start-Process -FilePath $FilePath -ArgumentList $argumentText -PassThru -NoNewWindow -RedirectStandardOutput $StdoutPath -RedirectStandardError $stderrPath
    }
    catch {
        $report = [ordered]@{
            format = "ue_source_reader_watchdog_v1"
            success = $false
            exit_reason = "start_failed"
            started_at_utc = $startedAt.ToUniversalTime().ToString("o")
            last_progress_at_utc = $lastProgressAt.ToUniversalTime().ToString("o")
            last_progress_source = $lastProgressSource
            elapsed_seconds = 0.0
            process_exit_code = $null
        }
        Write-ReaderReport -Path $ReportPath -Value $report
        throw
    }
    $exitReason = "completed"
    while (-not $process.HasExited) {
        Start-Sleep -Milliseconds 250
        $now = Get-Date
        foreach ($path in $ProgressPaths) {
            if (Test-Path -LiteralPath $path) {
                $item = Get-Item -LiteralPath $path
                if ($item.LastWriteTime -gt $lastProgressAt) {
                    $lastProgressAt = $item.LastWriteTime
                    $lastProgressSource = [IO.Path]::GetFileName($path)
                }
            }
        }
        if (($now - $startedAt).TotalSeconds -ge $TimeoutSeconds) {
            $exitReason = "wall_timeout"
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            break
        }
        if (($now - $lastProgressAt).TotalSeconds -ge $NoProgressSeconds) {
            $exitReason = "no_progress_timeout"
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            break
        }
        $process.Refresh()
    }
    if ($exitReason -eq "completed") {
        $null = $process.WaitForExit(5000)
        $process.Refresh()
    }
    elseif (-not $process.HasExited) {
        $null = $process.WaitForExit(5000)
        $process.Refresh()
    }
    $finishedAt = Get-Date
    $hasExited = $process.HasExited
    $exitCode = $null
    if ($hasExited) {
        $exitCode = [int]$process.ExitCode
    }
    $report = [ordered]@{
        format = "ue_source_reader_watchdog_v1"
        success = ($exitReason -eq "completed" -and $exitCode -eq 0)
        exit_reason = $exitReason
        started_at_utc = $startedAt.ToUniversalTime().ToString("o")
        last_progress_at_utc = $lastProgressAt.ToUniversalTime().ToString("o")
        last_progress_source = $lastProgressSource
        finished_at_utc = $finishedAt.ToUniversalTime().ToString("o")
        elapsed_seconds = [math]::Round(($finishedAt - $startedAt).TotalSeconds, 3)
        process_exit_code = $exitCode
    }
    Write-ReaderReport -Path $ReportPath -Value $report
    return [PSCustomObject]$report
}

function Invoke-WatchdogSelfTest {
    $temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("ue-source-reader-watchdog-" + [guid]::NewGuid().ToString("N"))
    $temporaryBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
    try {
        $shell = (Get-Command pwsh -ErrorAction Stop).Source
        $stalled = Invoke-ProcessWithWatchdog -FilePath $shell -ArgumentList @("-NoProfile", "-Command", "Start-Sleep -Seconds 3") -ReportPath (Join-Path $temporaryRoot "stalled.report.json") -StdoutPath (Join-Path $temporaryRoot "stalled.stdout.log") -StderrPath (Join-Path $temporaryRoot "stalled.stderr.log") -ProgressPaths @((Join-Path $temporaryRoot "stalled.stdout.log"), (Join-Path $temporaryRoot "stalled.stderr.log")) -TimeoutSeconds 4 -NoProgressSeconds 1
        if ($stalled.exit_reason -ne "no_progress_timeout" -or $stalled.success) {
            throw "watchdog did not terminate a stalled process"
        }
        $completed = Invoke-ProcessWithWatchdog -FilePath $shell -ArgumentList @("-NoProfile", "-Command", "Write-Output progress") -ReportPath (Join-Path $temporaryRoot "completed.report.json") -StdoutPath (Join-Path $temporaryRoot "completed.stdout.log") -StderrPath (Join-Path $temporaryRoot "completed.stderr.log") -ProgressPaths @((Join-Path $temporaryRoot "completed.stdout.log"), (Join-Path $temporaryRoot "completed.stderr.log")) -TimeoutSeconds 4 -NoProgressSeconds 1
        if ($completed.exit_reason -ne "completed" -or -not $completed.success) {
            throw ("watchdog rejected a completed process: " + ($completed | ConvertTo-Json -Compress))
        }
        [ordered]@{
            stalled_process_terminated = "PASS"
            completed_process_preserved = "PASS"
            machine_readable_report_written = "PASS"
        } | ConvertTo-Json -Compress
    }
    finally {
        $resolvedRoot = [IO.Path]::GetFullPath($temporaryRoot)
        if (-not $resolvedRoot.StartsWith($temporaryBase, [StringComparison]::OrdinalIgnoreCase)) {
            throw "self test cleanup target is outside temporary directory"
        }
        if (Test-Path -LiteralPath $resolvedRoot) {
            Remove-Item -LiteralPath $resolvedRoot -Recurse -Force
        }
    }
}

if ($SelfTest) {
    Invoke-WatchdogSelfTest
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Asset) -or [string]::IsNullOrWhiteSpace($Output)) {
    throw "Asset and Output are required"
}

$configPath = Join-Path $PSScriptRoot "env.local.json"
if (-not (Test-Path -LiteralPath $configPath)) {
    throw "tools/env.local.json is required"
}

$config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
$pluginPath = [IO.Path]::GetFullPath($Plugin)
$outputPath = [IO.Path]::GetFullPath($Output)
$outputDirectory = Split-Path -Parent $outputPath
$reportPath = $outputPath + ".reader.report.json"
$stdoutPath = $outputPath + ".reader.stdout.log"
$stderrPath = $outputPath + ".reader.stderr.log"
$logPath = $outputPath + ".reader.log"

if (-not (Test-Path -LiteralPath $config.ue_cmd)) {
    throw "ue_cmd does not exist"
}
if (-not (Test-Path -LiteralPath $config.ue_project)) {
    throw "ue_project does not exist"
}
if (-not (Test-Path -LiteralPath $pluginPath)) {
    throw "source reader plugin does not exist"
}
if (-not (Test-Path -LiteralPath $outputDirectory)) {
    throw "output directory does not exist"
}

$arguments = @(
    $config.ue_project,
    "-PLUGIN=$pluginPath",
    "-EnablePlugins=AlsGraphSourceReader",
    "-DisablePlugins=ModelContextProtocol",
    "-run=AlsGraphSourceReader",
    "-Asset=$Asset",
    "-Output=$outputPath",
    "-unattended",
    "-nop4",
    "-nosplash",
    "-stdout",
    "-FullStdOutLogOutput",
    "-ForceLogFlush",
    "-abslog=$logPath"
)
$result = Invoke-ProcessWithWatchdog -FilePath $config.ue_cmd -ArgumentList $arguments -ReportPath $reportPath -StdoutPath $stdoutPath -StderrPath $stderrPath -ProgressPaths @($stdoutPath, $stderrPath, $logPath, $outputPath) -TimeoutSeconds $TimeoutSeconds -NoProgressSeconds $NoProgressSeconds
if (-not $result.success) {
    $result | ConvertTo-Json -Compress
    exit 124
}
if (-not (Test-Path -LiteralPath $outputPath)) {
    $result.success = $false
    $result.exit_reason = "output_missing"
    Write-ReaderReport -Path $reportPath -Value $result
    $result | ConvertTo-Json -Compress
    exit 4
}
$result | ConvertTo-Json -Compress
