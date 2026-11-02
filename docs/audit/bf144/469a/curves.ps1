$ErrorActionPreference = "Stop"
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als.SK_Als -BlendProfileProbe docs/audit/bf144/441b/curves.request.json -Output docs/audit/bf144/469a/curves.native.json -TimeoutSeconds 180 -NoProgressSeconds 90
$report = Get-Content -Raw docs/audit/bf144/469a/curves.native.json.reader.report.json | ConvertFrom-Json
if (-not $report.success -or $report.process_exit_code -ne 0) { throw "CurveVector compatibility failed" }
