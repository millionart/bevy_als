param([string]$Label)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/495"
$path = @{
    stand = "Base/A_Als_Stand_Pose.A_Als_Stand_Pose"
    idle = "Base/A_Als_Idle.A_Als_Idle"
    look = "View/A_Als_Look_Forward.A_Als_Look_Forward"
}[$Label]
if (-not $path) { throw "Unknown sequence witness" }
$output = "$root/$Label.native.json"
& ./tools/run_ue_source_reader.ps1 -Plugin target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin -Asset "/ALS/ALS/Animations/$path" -SequencePoseProbe "$root/$Label.request.json" -Output $output -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$output.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Sequence witness failed" }
exit 0
