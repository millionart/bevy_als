$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/577"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als.SK_Als -GeometryQueryProbe "$root/geometry.request.json" -Output "$root/geometry.native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/geometry.native.json.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Native common-geometry sweep failed" }
