$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/CR_Als.CR_Als -Output docs/audit/bf144/461/rig.native.json -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/461/rig.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.process_exit_code -ne 0) { throw "Fresh source Rig ABI extraction failed" }
