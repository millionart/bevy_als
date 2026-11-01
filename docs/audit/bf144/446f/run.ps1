param([string]$Label, [string]$Request = "request.json")
$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe "docs/audit/bf144/446f/$Request" -Output "docs/audit/bf144/446f/$Label.native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "docs/audit/bf144/446f/$Label.native.json.reader.report.json" | ConvertFrom-Json
if ($report.exit_reason -ne "completed") { throw "Native reader did not complete" }
exit $report.process_exit_code
