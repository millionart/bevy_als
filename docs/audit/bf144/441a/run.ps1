param([string]$Label, [string]$Kind, [int]$ExpectedExit = 0)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/441a"
$plugin = "target/ue_graph_source_reader_package/AlsGraphSourceReader.uplugin"
$output = "$root/$Label.native.json"
if (Test-Path -LiteralPath "$output.reader.report.json") { throw "Evidence label already exists" }
$parameters = @{
    Plugin = $plugin
    Output = $output
    TimeoutSeconds = 240
    NoProgressSeconds = 90
}
if ($Kind -eq "character") {
    $parameters.Asset = "/ALS/ALS/Character/B_Als_Character.B_Als_Character"
    $parameters.CharacterAnimationProbe = "docs/audit/bf144/441b/character.request.json"
} else {
    $parameters.Asset = "/ALS/ALS/Character/SK_Als.SK_Als"
    $requestKind = if ($Label -eq "vector-axis-ablated") { "mixed" } else { $Kind }
    $parameters.BlendProfileProbe = "$root/$requestKind.request.json"
}
& ./tools/run_ue_source_reader.ps1 @parameters
$report = Get-Content -Raw "$output.reader.report.json" | ConvertFrom-Json
if ($report.process_exit_code -ne $ExpectedExit -or $report.success -ne ($ExpectedExit -eq 0)) {
    throw "Native exit did not match expected probe outcome"
}
Write-Host "$Label native exit $ExpectedExit"
exit 0
