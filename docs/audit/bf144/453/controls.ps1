$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/453"
$results = @()
& .venv/bin/python "$root/mutate.py" save
if ($LASTEXITCODE -ne 0) { exit 1 }
try {
    & .venv/bin/python "$root/mutate.py" no-unit-stage
    if ($LASTEXITCODE -ne 0) { throw "Unit control mutation failed" }
    & pwsh -NoProfile -File "$root/run_replay.ps1" -Label no-unit-stage -Build
    if ($LASTEXITCODE -ne 0) { throw "Unit replay failed before its negative contract" }
    & pwsh -NoProfile -File "$root/run_python.ps1" -Name analyze -Case no-unit-stage
    if ($LASTEXITCODE -eq 0) { throw "Missing unit stage signal was accepted" }
    $results += @{ label = "no-unit-stage"; expected_failure = "analysis" }
    & .venv/bin/python "$root/mutate.py" no-child-projection
    if ($LASTEXITCODE -ne 0) { throw "Child projection mutation failed" }
    & pwsh -NoProfile -File "$root/run_replay.ps1" -Label no-child-projection -Build
    if ($LASTEXITCODE -eq 0) { throw "Missing child projection was accepted" }
    $compiled = Get-Content -Raw "$root/replay-compile-no-child-projection.report.json" | ConvertFrom-Json
    $replay = Get-Content -Raw "$root/replay-no-child-projection.report.json" | ConvertFrom-Json
    if (-not $compiled.success -or $replay.process_exit_code -ne 101) { throw "Child projection did not reach its actual current endpoint contract" }
    $results += @{ label = "no-child-projection"; expected_failure = "actual-current-endpoint" }
} finally {
    & .venv/bin/python "$root/mutate.py" restore
    if ($LASTEXITCODE -ne 0) { throw "Source byte restoration failed" }
}
$results | ConvertTo-Json -Depth 5 | Set-Content "$root/controls.json"
& pwsh -NoProfile -File "$root/run_replay.ps1" -Label restored -Build
if ($LASTEXITCODE -ne 0) { exit 1 }
& pwsh -NoProfile -File "$root/run_python.ps1" -Name analyze -Case restored
if ($LASTEXITCODE -ne 0) { exit 1 }
