param([string]$Label)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/446d"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/446d"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) ".venv/bin/python") -ArgumentList @("$root/check.py", $Label) -ReportPath "$root/check-$Label.report.json" -StdoutPath "$logs/check-$Label.stdout.log" -StderrPath "$logs/check-$Label.stderr.log" -ProgressPaths @("$logs/check-$Label.stdout.log", "$logs/check-$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 30
Get-Content -Tail 5 "$logs/check-$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 15 "$logs/check-$Label.stderr.log"; exit 1 }
