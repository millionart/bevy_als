param([string]$Label, [string]$Runner = "tools/run_animation_visual_validation.ps1")
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/441c0"
$result = Invoke-ProcessWithWatchdog -FilePath (Get-Command pwsh).Source -ArgumentList @("-NoProfile", "-File", "tools/tests/test_visual_evidence_routing.ps1", "-Runner", $Runner, "-Label", $Label) -ReportPath "$root/$Label.report.json" -StdoutPath "$root/$Label.stdout.log" -StderrPath "$root/$Label.stderr.log" -ProgressPaths @("$root/$Label.stdout.log", "$root/$Label.stderr.log") -TimeoutSeconds 30 -NoProgressSeconds 10
Get-Content -Tail 4 "$root/$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 7 "$root/$Label.stderr.log"; exit 1 }
