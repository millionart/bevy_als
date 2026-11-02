param([string]$Kind)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/481a"
if ($Kind -eq "character-default") {
    & ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe docs/audit/bf144/457/default.request.json -Output "$root/character-default.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
} elseif ($Kind -eq "curves") {
    & ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als.SK_Als -BlendProfileProbe docs/audit/bf144/441b/curves.request.json -Output "$root/curves.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
} else { throw "Unknown compatibility fixture" }
$report = Get-Content -Raw "$root/$Kind.native.json.reader.report.json" | ConvertFrom-Json
if (-not $report.success) { throw "Native compatibility probe failed" }
