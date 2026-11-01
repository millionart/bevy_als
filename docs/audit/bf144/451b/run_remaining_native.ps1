$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/451b"
foreach ($batch in @(Get-Content -Raw "$root/requests.json" | ConvertFrom-Json)) {
    if ($batch.label -eq "batch-00") { continue }
    & pwsh -NoProfile -File "$root/run_native.ps1" -Label $batch.label
    $report = Get-Content -Raw "$root/$($batch.label).native.json.reader.report.json" | ConvertFrom-Json
    if (-not $report.success -or $report.process_exit_code -ne 0) { throw "A native four-domain batch failed" }
    Write-Output "Completed native four-domain batch $($batch.label)"
}
