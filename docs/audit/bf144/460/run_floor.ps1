$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Floor_20x20.SM_Als_Floor_20x20 -Output docs/audit/bf144/460/floor.native.json -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/460/floor.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.process_exit_code -ne 0) { throw "Source floor collision read failed" }
