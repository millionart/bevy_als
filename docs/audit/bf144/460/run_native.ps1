$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als.AB_Als -RigNodeHistoryProbe docs/audit/bf144/460/request.json -Output docs/audit/bf144/460/common-rig.native.json -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/460/common-rig.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.process_exit_code -ne 0) { throw "Continuous common-input source Rig evaluation failed" }
