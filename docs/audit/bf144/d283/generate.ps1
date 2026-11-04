$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
if ([string]::IsNullOrWhiteSpace($env:ALS_SOURCE_CONTENT_ROOT)) { throw "ALS_SOURCE_CONTENT_ROOT is required" }
$source = Join-Path $env:ALS_SOURCE_CONTENT_ROOT "ALS/Data/AnimationInstance/AIS_Als_Default.uasset"
$python = if ($env:ALS_PYTHON) { $env:ALS_PYTHON } else { (Get-Command python3).Source }
$logs = "tools/tmp/bf144/d283"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$arguments = @("tools/import/gen_standing_curves.py", "--uasset", $source,
    "--content-root", $env:ALS_SOURCE_CONTENT_ROOT,
    "--output", "docs/audit/bf144/d283/standing.generated.json",
    "--report", "docs/audit/bf144/d283/generator.report.json")
$result = Invoke-ProcessWithWatchdog -FilePath $python -ArgumentList $arguments `
    -ReportPath "docs/audit/bf144/d283/generator.watchdog.json" `
    -StdoutPath "$logs/generator.stdout.log" -StderrPath "$logs/generator.stderr.log" `
    -ProgressPaths @("$logs/generator.stdout.log", "$logs/generator.stderr.log") `
    -TimeoutSeconds 1800 -NoProgressSeconds 150
if (-not $result.success) { throw ("Standing settings generation failed: " + $result.exit_reason) }
