param([string]$Label,[switch]$WithoutYawGuard)
$ErrorActionPreference = "Stop"
$root = Join-Path (Get-Location) "docs/audit/bf144/442b0"
$names = if ($WithoutYawGuard) { @("yaw-string") } else { @("yaw-string","axis-string","axis-short") }
$expected = if ($WithoutYawGuard) { 0 } else { 7 }
$results = @()
foreach ($name in $names) {
    & pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als.SK_Als -GeometryQueryProbe "$root/$name.request.json" -Output "$root/$Label-$name.native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
    $result = Get-Content -Raw "$root/$Label-$name.native.json.reader.report.json" | ConvertFrom-Json
    $results += @{ name = $name; expected_exit = $expected; actual_exit = $result.process_exit_code }
    $results | ConvertTo-Json -Depth 4 | Set-Content "$root/$Label-invalid.json"
    if ($result.process_exit_code -ne $expected) { throw "native invalid input did not produce expected exit code" }
    Write-Output ("Checked native input contract " + $name)
}
