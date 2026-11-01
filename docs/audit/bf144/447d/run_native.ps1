param([string]$Label)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/447d"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als.SK_Als -GeometryQueryProbe "$root/request.json" -Output "$root/$Label.native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/$Label.native.json.reader.report.json" | ConvertFrom-Json
if ($report.exit_reason -ne "completed") { throw "Native reader did not complete" }
exit $report.process_exit_code
