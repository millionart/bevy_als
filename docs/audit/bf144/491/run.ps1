param([string]$Label,[string]$Request,[int]$ExpectedExit=0)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/491"
$output = "$root/$Label.native.json"
if (Test-Path "$output.reader.report.json") { throw "Existing native evidence label" }
& ./tools/run_ue_source_reader.ps1 -Plugin target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe "$root/$Request.request.json" -Output $output -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$output.reader.report.json" | ConvertFrom-Json
if ($report.process_exit_code -ne $ExpectedExit -or $report.success -ne ($ExpectedExit -eq 0)) { throw "Unexpected native probe status" }
exit 0
