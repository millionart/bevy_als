$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/441c"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/441c"
$null = New-Item -ItemType Directory -Force -Path $logs
$result = Invoke-ProcessWithWatchdog -FilePath (Get-Command cargo).Source -ArgumentList @("build", "--locked", "--jobs", "2", "--profile", "play", "--example", "manual_playground") -ReportPath "$root/build.report.json" -StdoutPath "$logs/build.stdout.log" -StderrPath "$logs/build.stderr.log" -ProgressPaths @("$logs/build.stdout.log", "$logs/build.stderr.log") -TimeoutSeconds 3600 -NoProgressSeconds 600
Get-Content -Tail 12 "$logs/build.stderr.log"
if (-not $result.success) { exit 1 }
