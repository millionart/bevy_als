param([string]$Name)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/442-final"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/442-final"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) ".venv/bin/python") -ArgumentList @("$root/$Name.py") -ReportPath "$root/$Name.report.json" -StdoutPath "$logs/$Name.stdout.log" -StderrPath "$logs/$Name.stderr.log" -ProgressPaths @("$logs/$Name.stdout.log", "$logs/$Name.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 30
Get-Content -Tail 4 "$logs/$Name.stdout.log"
if (-not $result.success) { Get-Content -Tail 12 "$logs/$Name.stderr.log"; exit 1 }
