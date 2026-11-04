param([string]$Label)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$config = Get-Content -Raw tools/env.local.json | ConvertFrom-Json
$engine = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $config.ue_cmd))
$platform = if ($IsWindows) { "Win64" } else { "Mac" }
$uat = Join-Path $engine $(if ($IsWindows) { "Build/BatchFiles/RunUAT.bat" } else { "Build/BatchFiles/RunUAT.sh" })
$root = Join-Path (Get-Location) "docs/audit/bf144/566"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/566"
$plugin = Join-Path $logs "reader-source/AlsGraphSourceReader.uplugin"
$package = Join-Path (Get-Location) "target/ue_graph_source_reader_566"
$architecture = if ($IsMacOS) { @("-Architecture_Mac=arm64") } else { @() }
$result = Invoke-ProcessWithWatchdog -FilePath $uat -ArgumentList (@("BuildPlugin", "-Plugin=$plugin", "-Package=$package", "-TargetPlatforms=$platform", "-Rocket", "-MaxParallelActions=1") + $architecture) -ReportPath "$root/build-$Label.report.json" -StdoutPath "$logs/build-$Label.stdout.log" -StderrPath "$logs/build-$Label.stderr.log" -ProgressPaths @("$logs/build-$Label.stdout.log", "$logs/build-$Label.stderr.log") -TimeoutSeconds 600 -NoProgressSeconds 120
Get-Content -Tail 8 "$logs/build-$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 10 "$logs/build-$Label.stderr.log"; exit 1 }
