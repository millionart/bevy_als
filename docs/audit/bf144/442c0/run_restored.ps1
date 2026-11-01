$ErrorActionPreference = "Stop"
& ./docs/audit/bf144/442c0/run.ps1 -Label restored
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& ./docs/audit/bf144/442c0/run.ps1 -Label restored-invalid -Request invalid.request.json
if ($LASTEXITCODE -ne 7) { throw "Restored type guard accepted the string flag" }
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe docs/audit/bf144/441b/character.request.json -Output docs/audit/bf144/442c0/legacy240.native.json -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw "docs/audit/bf144/442c0/legacy240.native.json.reader.report.json" | ConvertFrom-Json
if ($report.exit_reason -ne "completed") { throw "Legacy native reader did not complete" }
exit $report.process_exit_code
