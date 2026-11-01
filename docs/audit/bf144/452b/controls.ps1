$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/452b"
$python = ".venv/bin/python"
$results = @()
try {
    foreach ($label in @("no-delta-factor", "no-product-factor", "no-second-factor", "no-sqrt-factor", "no-projection-factor")) {
        & $python "$root/mutate.py" $label
        if ($LASTEXITCODE -ne 0) { throw "Source mutation failed" }
        & pwsh -NoProfile -File "$root/run_replay.ps1" -Label $label -Build
        if ($LASTEXITCODE -ne 0) { throw "Controlled replay failed before its negative contract" }
        & pwsh -NoProfile -File "$root/run_python.ps1" -Name analyze -Case $label
        $analysisExit = $LASTEXITCODE
        if ($analysisExit -eq 0) { throw "Removed stage factor was accepted" }
        $results += @{ label = $label; analysis_exit_code = $analysisExit; source_control_rejected = $true }
    }
} finally {
    & $python "$root/mutate.py" restore
    if ($LASTEXITCODE -ne 0) { throw "Source byte restoration failed" }
}
$results | ConvertTo-Json -Depth 5 | Set-Content "$root/controls.json"
& pwsh -NoProfile -File "$root/run_replay.ps1" -Label restored -Build
if ($LASTEXITCODE -ne 0) { exit 1 }
& pwsh -NoProfile -File "$root/run_python.ps1" -Name analyze -Case restored
if ($LASTEXITCODE -ne 0) { exit 1 }
