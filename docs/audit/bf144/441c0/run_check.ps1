$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/441c0"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) ".venv/bin/python") -ArgumentList @("$root/check.py") -ReportPath "$root/check.report.json" -StdoutPath "$root/check.stdout.log" -StderrPath "$root/check.stderr.log" -ProgressPaths @("$root/check.stdout.log", "$root/check.stderr.log") -TimeoutSeconds 30 -NoProgressSeconds 10
Get-Content -Tail 2 "$root/check.stdout.log"
if (-not $result.success) { Get-Content "$root/check.stderr.log"; exit 1 }
