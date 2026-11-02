$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe docs/audit/bf144/457/default.request.json -Output docs/audit/bf144/467/character-default.native.json -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/467/character-default.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.process_exit_code -ne 0) { throw "Default character compatibility failed" }
