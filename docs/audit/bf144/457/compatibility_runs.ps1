$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/457"
foreach ($case in @(@("legacy", "legacy.request.json", 0), @("default", "default.request.json", 0), @("invalid", "invalid.request.json", 7), @("missing-boundaries", "missing-boundaries.request.json", 7))) {
    & pwsh -NoProfile -File "$root/run.ps1" -Label $case[0] -Request $case[1]
    if ($LASTEXITCODE -ne $case[2]) { throw "Unexpected native compatibility process result" }
}
& pwsh -NoProfile -File "$root/curves.ps1"
if ($LASTEXITCODE -ne 0) { throw "CurveVector compatibility process failed" }
