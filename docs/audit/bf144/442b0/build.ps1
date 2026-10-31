param([string]$Label)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/442b0"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/442b0"
$result = Invoke-ProcessWithWatchdog -FilePath (Get-Command pwsh).Source -ArgumentList @("-NoProfile", "-File", "tools/build_ue_source_reader.ps1") -ReportPath "$root/$Label.report.json" -StdoutPath "$logs/$Label.stdout.log" -StderrPath "$logs/$Label.stderr.log" -ProgressPaths @("$logs/$Label.stdout.log", "$logs/$Label.stderr.log") -TimeoutSeconds 600 -NoProgressSeconds 120
Get-Content -Tail 6 "$logs/$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 10 "$logs/$Label.stderr.log"; exit 1 }
