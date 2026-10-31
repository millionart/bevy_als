$ErrorActionPreference = "Stop"
$root = Join-Path (Get-Location) "docs/audit/bf144/442b0"
foreach ($item in @(
    @{ label = "restored"; request = "docs/audit/bf144/442b0/request.json" },
    @{ label = "legacy"; request = "docs/audit/bf144/441c/mapping.request.json" }
)) {
    & pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als.SK_Als -GeometryQueryProbe $item.request -Output "$root/$($item.label).native.json" -TimeoutSeconds 180 -NoProgressSeconds 90
    $result = Get-Content -Raw "$root/$($item.label).native.json.reader.report.json" | ConvertFrom-Json
    if (-not $result.success) { throw "restored native probe failed" }
    Write-Output ("Captured final native probe " + $item.label)
}
& pwsh -NoProfile -File docs/audit/bf144/442b0/run_invalid.ps1 -Label restored
if ($LASTEXITCODE -ne 0) { throw "restored invalid input checks failed" }
