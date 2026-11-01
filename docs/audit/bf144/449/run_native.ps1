param([string]$Label, [string]$Request)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/449"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Base/A_Als_Stand_Pose.A_Als_Stand_Pose -SequencePoseProbe "$root/$Request" -Output "$root/$Label.native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/$Label.native.json.reader.report.json" | ConvertFrom-Json
if ($report.exit_reason -ne "completed") { throw "Native reader did not complete" }
exit $report.process_exit_code
