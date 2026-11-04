$ErrorActionPreference = "Stop"
$output = "docs/audit/bf144/577/floor.native.json"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Floor_20x20.SM_Als_Floor_20x20 -Output $output -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "$output.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Native source floor asset read failed" }
