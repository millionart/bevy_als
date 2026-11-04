param([string]$Label,[string]$Request,[int]$ExpectedExit=0,[switch]$Canonical)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/516"
$output = "$root/$Label.native.json"
if (Test-Path "$output.reader.report.json") { throw "Existing native evidence label" }
$package = if ($Canonical) { "target/ue_graph_source_reader_package" } else { "target/ue_graph_source_reader_516" }
$library = if ($IsWindows) { "Binaries/Win64/UnrealEditor-AlsGraphSourceReader.dll" } else { "Binaries/Mac/libUnrealEditor-AlsGraphSourceReader.dylib" }
$identity = @{package = $package; library = $library; sha256 = (Get-FileHash -Algorithm SHA256 "$package/$library").Hash.ToLower()}
$identity | ConvertTo-Json -Compress | Set-Content -Encoding utf8NoBOM "$root/$Label.package.json"
& ./tools/run_ue_source_reader.ps1 -Plugin "$package/AlsGraphSourceReader.uplugin" -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe "$root/$Request.request.json" -Output $output -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$output.reader.report.json" | ConvertFrom-Json
if ($report.process_exit_code -ne $ExpectedExit -or $report.success -ne ($ExpectedExit -eq 0)) { throw "Unexpected native probe status" }
exit 0
