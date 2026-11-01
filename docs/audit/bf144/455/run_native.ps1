$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als.AB_Als -Output docs/audit/bf144/455/main.native.json -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/455/main.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.exit_reason -ne "completed") { throw "Fresh main AnimBP source read failed" }
exit $report.process_exit_code
