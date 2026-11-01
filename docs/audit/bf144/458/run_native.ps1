$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Ragdolling.AB_Als_Ragdolling -BlendListProbe docs/audit/bf144/458/request.json -Output docs/audit/bf144/458/common-input.native.json -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/458/common-input.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.process_exit_code -ne 0) { throw "Same-input single full-weight BlendList evaluation failed" }
