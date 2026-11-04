$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = "docs/audit/bf144/579"
$scratch = "tools/tmp/bf144/579"
$executable = Join-Path (Get-Location) "$scratch/playground_observed"
$result = Invoke-ProcessWithWatchdog -FilePath $executable -ArgumentList @("$root/inputs.json", "$root/probe.json") -ReportPath "$root/probe.report.json" -StdoutPath "$scratch/probe.stdout.log" -StderrPath "$scratch/probe.stderr.log" -ProgressPaths @("$scratch/probe.stdout.log", "$scratch/probe.stderr.log") -TimeoutSeconds 60 -NoProgressSeconds 30
if (-not $result.success) { Get-Content "$scratch/probe.stderr.log"; exit 1 }
Get-Content "$scratch/probe.stdout.log"
