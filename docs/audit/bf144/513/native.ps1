$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/513"
$plugin = "target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin"
& ./tools/run_ue_source_reader.ps1 -Plugin $plugin -Asset /ALS/ALS/Data/Character/Movement/MS_Als_Normal.MS_Als_Normal -Output "$root/movement.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/movement.native.json.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Native movement extraction failed" }
& ./tools/run_ue_source_reader.ps1 -Plugin $plugin -Asset /ALS/ALS/Character/SK_Als.SK_Als -BlendProfileProbe "$root/profiles.request.json" -Output "$root/profiles.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/profiles.native.json.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Native curve extraction failed" }
