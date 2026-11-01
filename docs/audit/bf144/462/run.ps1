param([string]$Label,[string]$Request="request",[int]$ExpectedExit=0)
$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/462"
$requestPath = if ($Request -eq "default") { "docs/audit/bf144/460/request.json" } else { "$root/$Request.json" }
if (-not (Test-Path $requestPath)) { $requestPath = "$root/$Request.request.json" }
& ./tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AB_Als.AB_Als -RigNodeHistoryProbe $requestPath -Output "$root/$Label.native.json" -TimeoutSeconds 240 -NoProgressSeconds 90
$report = Get-Content -Raw "$root/$Label.native.json.reader.report.json" | ConvertFrom-Json
if ($report.process_exit_code -ne $ExpectedExit) { throw "Native reference ray probe exit contract failed" }
