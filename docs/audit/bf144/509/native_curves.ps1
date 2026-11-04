$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/509"
foreach ($curve in @("CF_Als_LeanAmount_Air", "CF_Als_GroundPredictionAmount")) {
    $reportPath = "$root/$curve.native.json.reader.report.json"
    if (-not (Test-Path $reportPath)) {
        & ./tools/run_ue_source_reader.ps1 -Plugin target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin -Asset "/ALS/ALS/Data/AnimationInstance/Air/$curve.$curve" -Output "$root/$curve.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
    }
    $report = Get-Content -Raw $reportPath | ConvertFrom-Json
    if (-not $report.success) { throw "Native curve extraction failed" }
}
