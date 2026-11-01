$ErrorActionPreference = "Stop"
$root = "docs/audit/bf144/452b"
$results = @()
try {
    foreach ($label in @("native-no-neon")) {
        & .venv/bin/python "$root/mutate.py" $label
        if ($LASTEXITCODE -ne 0) { throw "Primitive mutation failed" }
        & pwsh -NoProfile -File "$root/run_replay.ps1" -Label $label -Build
        $replayExit = $LASTEXITCODE
        if ($replayExit -eq 0) { throw "Changed source arithmetic primitive was accepted" }
        $compiled = Get-Content -Raw "$root/replay-compile-$label.report.json" | ConvertFrom-Json
        $replay = Get-Content -Raw "$root/replay-$label.report.json" | ConvertFrom-Json
        if (-not $compiled.success -or $replay.process_exit_code -ne 101) { throw "Primitive control did not reach the exact native contract" }
        $results += @{ label = $label; replay_exit_code = $replay.process_exit_code; exact_native_contract_rejected = $true }
    }
} finally {
    & .venv/bin/python "$root/mutate.py" restore
    if ($LASTEXITCODE -ne 0) { throw "Source byte restoration failed" }
}
ConvertTo-Json -InputObject @($results) -Depth 5 | Set-Content "$root/primitive-controls.json"
& pwsh -NoProfile -File "$root/run_replay.ps1" -Label primitive-restored -Build
if ($LASTEXITCODE -ne 0) { exit 1 }
& pwsh -NoProfile -File "$root/run_python.ps1" -Name analyze -Case primitive-restored
if ($LASTEXITCODE -ne 0) { exit 1 }
