param(
    [string]$Asset,
    [string]$Output,
    [string]$BlendSpaceProbe,
    [string]$SequencePoseProbe,
    [string]$MontageClockProbe,
    [string]$PhysicsBodyProbe,
    [string]$PhysicsContactProbe,
    [string]$GeometryQueryProbe,
    [switch]$CachedPoseLifecycleProbe,
    [switch]$CachedPoseSchedule,
    [switch]$AlphaBlendProbe,
    [switch]$NotifyClockProbe,
    [switch]$NotifyChanceProbe,
    [switch]$NotifyDispatchProbe,
    [switch]$PhysicsMaterialProbe,
    [string]$Plugin = (Join-Path $PSScriptRoot "../target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin"),
    [ValidateRange(1, 3600)]
    [int]$TimeoutSeconds = 300,
    [ValidateRange(1, 3600)]
    [int]$NoProgressSeconds = 150,
    [switch]$SelfTest,
    [switch]$FunctionsOnly
)

function ConvertTo-ProcessArgument {
    param([string]$Value)

    if ($Value.Length -eq 0) {
        return '""'
    }
    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    $escaped = [regex]::Replace($Value, '(\\*)"', '$1$1\"')
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
    $argumentText = ($ArgumentList | ForEach-Object { ConvertTo-ProcessArgument $_ }) -join " "
    try {
        $process = Start-Process -FilePath $FilePath -ArgumentList $argumentText -PassThru -NoNewWindow -RedirectStandardOutput $StdoutPath -RedirectStandardError $StderrPath
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
            $process.Kill($true)
            break
        }
        if (($now - $lastProgressAt).TotalSeconds -ge $NoProgressSeconds) {
            $exitReason = "no_progress_timeout"
            $process.Kill($true)
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
        no_progress_seconds = [math]::Round(($finishedAt - $lastProgressAt).TotalSeconds, 3)
        no_progress_limit_seconds = $NoProgressSeconds
        timeout_seconds = $TimeoutSeconds
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
        $sourceRoot = Join-Path $temporaryRoot "source"
        $builtRoot = Join-Path $temporaryRoot "built"
        $sourceFile = Join-Path $sourceRoot "Source/Reader.cpp"
        $sourcePlugin = Join-Path $sourceRoot "AlsGraphSourceReader.uplugin"
        $builtPlugin = Join-Path $builtRoot "AlsGraphSourceReader.uplugin"
        $builtDll = Join-Path $builtRoot $(if ($IsWindows) { "Binaries/Win64/UnrealEditor-AlsGraphSourceReader.dll" } else { "Binaries/Mac/libUnrealEditor-AlsGraphSourceReader.dylib" })
        New-Item -ItemType Directory -Path (Join-Path $sourceRoot "Source"), (Split-Path $builtDll) -Force | Out-Null
        Set-Content -LiteralPath $sourceFile -Value "source"
        Set-Content -LiteralPath $sourcePlugin -Value "descriptor"
        if (-not (Test-ReaderBuildRequired $sourceRoot $builtPlugin)) { throw "missing reader build was accepted" }
        Set-Content -LiteralPath $builtPlugin -Value "descriptor"
        Set-Content -LiteralPath $builtDll -Value "binary"
        (Get-Item -LiteralPath $builtDll).LastWriteTimeUtc = [DateTime]::UtcNow.AddSeconds(10)
        if (Test-ReaderBuildRequired $sourceRoot $builtPlugin) { throw "current reader build was rejected" }
        (Get-Item -LiteralPath $sourceFile).LastWriteTimeUtc = [DateTime]::UtcNow.AddSeconds(20)
        if (-not (Test-ReaderBuildRequired $sourceRoot $builtPlugin)) { throw "stale reader build was accepted" }
        $shell = (Get-Command pwsh -ErrorAction Stop).Source
        $stalledCommand = '$child = Start-Process -FilePath (Get-Command pwsh).Source -ArgumentList @("-NoProfile", "-Command", "Start-Sleep -Seconds 10") -NoNewWindow -PassThru; $child.Id; Start-Sleep -Seconds 10'
        $stalledScript = Join-Path $temporaryRoot "stalled.ps1"
        [IO.File]::WriteAllText($stalledScript, $stalledCommand, [Text.UTF8Encoding]::new($false))
        $stalled = Invoke-ProcessWithWatchdog -FilePath $shell -ArgumentList @("-NoProfile", "-File", $stalledScript) -ReportPath (Join-Path $temporaryRoot "stalled.report.json") -StdoutPath (Join-Path $temporaryRoot "stalled.stdout.log") -StderrPath (Join-Path $temporaryRoot "stalled.stderr.log") -ProgressPaths @((Join-Path $temporaryRoot "stalled.stdout.log"), (Join-Path $temporaryRoot "stalled.stderr.log")) -TimeoutSeconds 4 -NoProgressSeconds 1
        if ($stalled.exit_reason -ne "no_progress_timeout" -or $stalled.success) {
            throw "watchdog did not terminate a stalled process"
        }
        $childProcessId = [int](Get-Content -LiteralPath (Join-Path $temporaryRoot "stalled.stdout.log") | Where-Object { $_ -match '^\d+$' } | Select-Object -Last 1)
        if ($childProcessId -le 0) { throw "watchdog child fixture did not start" }
        if (Get-Process -Id $childProcessId -ErrorAction SilentlyContinue) {
            Stop-Process -Id $childProcessId -Force
            throw "watchdog left its child process running"
        }
        $completed = Invoke-ProcessWithWatchdog -FilePath $shell -ArgumentList @("-NoProfile", "-Command", "Write-Output progress") -ReportPath (Join-Path $temporaryRoot "completed.report.json") -StdoutPath (Join-Path $temporaryRoot "completed.stdout.log") -StderrPath (Join-Path $temporaryRoot "completed.stderr.log") -ProgressPaths @((Join-Path $temporaryRoot "completed.stdout.log"), (Join-Path $temporaryRoot "completed.stderr.log")) -TimeoutSeconds 4 -NoProgressSeconds 1
        if ($completed.exit_reason -ne "completed" -or -not $completed.success) {
            throw ("watchdog rejected a completed process: " + ($completed | ConvertTo-Json -Compress))
        }
        $argumentScript = Join-Path $temporaryRoot "arguments with spaces.ps1"
        Set-Content -LiteralPath $argumentScript -Value 'ConvertTo-Json -InputObject @($args) -Compress'
        $expected = @("", "path with spaces", 'quote"value', 'trailing slash\')
        $argumentOutput = Join-Path $temporaryRoot "arguments.stdout.log"
        $roundtrip = Invoke-ProcessWithWatchdog -FilePath $shell -ArgumentList (@("-NoProfile", "-File", $argumentScript) + $expected) -ReportPath (Join-Path $temporaryRoot "arguments.report.json") -StdoutPath $argumentOutput -StderrPath (Join-Path $temporaryRoot "arguments.stderr.log") -ProgressPaths @($argumentOutput) -TimeoutSeconds 4 -NoProgressSeconds 2
        $actual = Get-Content -Raw -LiteralPath $argumentOutput | ConvertFrom-Json
        if (-not $roundtrip.success -or (ConvertTo-Json -InputObject @($actual) -Compress) -cne (ConvertTo-Json -InputObject $expected -Compress)) { throw "process arguments changed" }
        [ordered]@{
            stalled_process_terminated = "PASS"
            stalled_child_process_terminated = "PASS"
            completed_process_preserved = "PASS"
            machine_readable_report_written = "PASS"
            missing_stale_and_current_reader_build = "PASS"
            empty_quoted_and_spaced_arguments = "PASS"
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

function Test-ReaderBuildRequired {
    param([string]$SourceRoot, [string]$BuiltPlugin)

    $library = if ($IsWindows) { "Binaries/Win64/UnrealEditor-AlsGraphSourceReader.dll" } else { "Binaries/Mac/libUnrealEditor-AlsGraphSourceReader.dylib" }
    $dll = Join-Path (Split-Path -Parent $BuiltPlugin) $library
    if (-not (Test-Path -LiteralPath $BuiltPlugin) -or -not (Test-Path -LiteralPath $dll)) { return $true }
    $binaryTime = (Get-Item -LiteralPath $dll).LastWriteTimeUtc
    $sources = @(Get-ChildItem -LiteralPath (Join-Path $SourceRoot "Source") -File -Recurse)
    $sources += Get-Item -LiteralPath (Join-Path $SourceRoot "AlsGraphSourceReader.uplugin")
    return @($sources | Where-Object { $_.LastWriteTimeUtc -gt $binaryTime }).Count -gt 0
}

if ($FunctionsOnly) {
    return
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
if (-not $PSBoundParameters.ContainsKey("Plugin") -and (Test-ReaderBuildRequired (Join-Path $PSScriptRoot "ue_graph_source_reader") $pluginPath)) {
    $buildPrefix = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/ue_graph_source_reader_build"))
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $buildPrefix)
    $build = Invoke-ProcessWithWatchdog -FilePath (Get-Command pwsh).Source -ArgumentList @("-NoProfile", "-File", (Join-Path $PSScriptRoot "build_ue_source_reader.ps1")) -ReportPath ($buildPrefix + ".report.json") -StdoutPath ($buildPrefix + ".stdout.log") -StderrPath ($buildPrefix + ".stderr.log") -ProgressPaths @(($buildPrefix + ".stdout.log"), ($buildPrefix + ".stderr.log")) -TimeoutSeconds 600 -NoProgressSeconds 120
    if (-not $build.success) { throw ("source reader build failed: " + $build.exit_reason) }
    if (Test-ReaderBuildRequired (Join-Path $PSScriptRoot "ue_graph_source_reader") $pluginPath) { throw "source reader build did not produce a current binary" }
}
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
if ($BlendSpaceProbe) {
    $probePath = (Resolve-Path -LiteralPath $BlendSpaceProbe -ErrorAction Stop).Path
    $arguments += "-BlendSpaceProbe=$probePath"
}
if ($SequencePoseProbe) {
    $probePath = (Resolve-Path -LiteralPath $SequencePoseProbe -ErrorAction Stop).Path
    $arguments += "-SequencePoseProbe=$probePath"
}
if ($MontageClockProbe) {
    $probePath = (Resolve-Path -LiteralPath $MontageClockProbe -ErrorAction Stop).Path
    $arguments += "-MontageClockProbe=$probePath"
}
if ($PhysicsBodyProbe) {
    $probePath = (Resolve-Path -LiteralPath $PhysicsBodyProbe -ErrorAction Stop).Path
    $arguments += "-PhysicsBodyProbe=$probePath"
}
if ($PhysicsContactProbe) {
    $probePath = (Resolve-Path -LiteralPath $PhysicsContactProbe -ErrorAction Stop).Path
    $arguments += "-PhysicsContactProbe=$probePath"
}
if ($GeometryQueryProbe) {
    $probePath = (Resolve-Path -LiteralPath $GeometryQueryProbe -ErrorAction Stop).Path
    $arguments += "-GeometryQueryProbe=$probePath"
}
if ($CachedPoseSchedule) {
    $arguments += "-CachedPoseSchedule"
}
if ($CachedPoseLifecycleProbe) {
    $arguments += "-CachedPoseLifecycleProbe"
}
if ($AlphaBlendProbe) {
    $arguments += "-AlphaBlendProbe"
}
if ($NotifyClockProbe) {
    $arguments += "-NotifyClockProbe"
}
if ($NotifyChanceProbe) {
    $arguments += "-NotifyChanceProbe"
}
if ($NotifyDispatchProbe) {
    $arguments += "-NotifyDispatchProbe"
}
if ($PhysicsMaterialProbe) {
    $arguments += "-PhysicsMaterialProbe"
}
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
