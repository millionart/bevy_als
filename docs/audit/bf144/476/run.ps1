param([string]$Label,[string]$Request)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/476"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als.AB_Als -RigNodeHistoryProbe "$root/$Request.request.json" -Output "$root/$Label.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/$Label.native.json.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Native IK boundary capture failed" }
