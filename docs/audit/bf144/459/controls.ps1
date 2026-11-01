$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/459"
try {
    foreach ($label in @("post-exposure-clock", "wrong-trace-start")) {
        & pwsh -NoProfile -File "$root/run_python.ps1" -Name ablate -Case $label
        if ($LASTEXITCODE -ne 0) { throw "VM input control failed" }
        & pwsh -NoProfile -File "$root/run_replay.ps1" -Label $label
        if ($LASTEXITCODE -eq 0) { throw "Invalid actual VM input was accepted" }
        $report = Get-Content -Raw "$root/replay-$label.report.json" | ConvertFrom-Json
        if ($report.process_exit_code -ne 101) { throw "VM input control did not reach its exact contract" }
    }
    & pwsh -NoProfile -File "$root/run_python.ps1" -Name ablate -Case restore
    if ($LASTEXITCODE -ne 0) { throw "VM input restoration failed" }
    & pwsh -NoProfile -File "$root/run_python.ps1" -Name ablate -Case no-construction
    if ($LASTEXITCODE -ne 0) { throw "VM construction control mutation failed" }
    & pwsh -NoProfile -File "$root/run_replay.ps1" -Label no-construction -Build
    if ($LASTEXITCODE -eq 0) { throw "Missing construction was accepted" }
    $report = Get-Content -Raw "$root/replay-no-construction.report.json" | ConvertFrom-Json
    if ($report.process_exit_code -ne 101) { throw "Construction control did not reach its exact contract" }
} finally {
    & pwsh -NoProfile -File "$root/run_python.ps1" -Name ablate -Case restore
    if ($LASTEXITCODE -ne 0) { throw "VM replay source or input byte restoration failed" }
}
& pwsh -NoProfile -File "$root/run_replay.ps1" -Label restored -Build
if ($LASTEXITCODE -ne 0) { exit 1 }
