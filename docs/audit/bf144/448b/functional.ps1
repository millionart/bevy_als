param([string]$Label)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = "docs/audit/bf144/448b"
$logs = "tools/tmp/bf144/448b"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) ".venv/bin/python") -ArgumentList @("tools/validate_animation_visual_evidence.py", "--report", "$root/runtime-$Label/report.json", "--check", "--functional-only", "--min-screenshots", "0") -ReportPath "$root/$Label-functional-process.json" -StdoutPath "$root/$Label-functional.json" -StderrPath "$logs/$Label-functional.stderr.log" -ProgressPaths @("$root/$Label-functional.json", "$logs/$Label-functional.stderr.log") -TimeoutSeconds 60 -NoProgressSeconds 30
Get-Content -Tail 5 "$root/$Label-functional.json"
if (-not $result.success) { Get-Content -Tail 8 "$logs/$Label-functional.stderr.log"; exit 1 }
