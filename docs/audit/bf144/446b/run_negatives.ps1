param([string]$Label)
$ErrorActionPreference = "Stop"
foreach ($name in @("over-limit", "bad-xy", "bad-yaw")) {
    & ./docs/audit/bf144/446b/run.ps1 -Label "$Label-$name" -Request "$name.request.json"
    $report = Get-Content -Raw "docs/audit/bf144/446b/$Label-$name.native.json.reader.report.json" | ConvertFrom-Json
    if ($report.exit_reason -ne "completed" -or $report.process_exit_code -ne 7) { throw "Invalid fixture request was not rejected" }
}
exit 0
