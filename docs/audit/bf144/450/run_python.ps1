param([string]$Name,[string]$Case)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/450"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/450"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$suffix = if ($Case) { "$Name-$Case" } else { $Name }
$arguments = @("$root/$Name.py")
if ($Case) { $arguments += $Case }
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) ".venv/bin/python") -ArgumentList $arguments -ReportPath "$root/$suffix.report.json" -StdoutPath "$logs/$suffix.stdout.log" -StderrPath "$logs/$suffix.stderr.log" -ProgressPaths @("$logs/$suffix.stdout.log", "$logs/$suffix.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 30
Get-Content -Tail 5 "$logs/$suffix.stdout.log"
if (-not $result.success) { Get-Content -Tail 15 "$logs/$suffix.stderr.log"; exit 1 }
