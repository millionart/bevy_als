param([string]$Label)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/451b"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Head.AB_Als_Head -MeshAdditiveProbe "$root/$Label.request.json" -Output "$root/$Label.native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/$Label.native.json.reader.report.json" | ConvertFrom-Json
if ($report.exit_reason -ne "completed") { throw "Native reader did not complete" }
exit $report.process_exit_code
