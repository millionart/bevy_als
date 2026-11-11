$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "../../../..")).Path
$env:CARGO_TARGET_DIR = Join-Path $root "target_game"
. (Join-Path $root "tools/run_ue_source_reader.ps1") -FunctionsOnly

function Invoke-CleanupCheck($Name, $File, $Arguments, $Expected) {
    $stdout = Join-Path $PSScriptRoot "$Name.stdout.log"
    $stderr = Join-Path $PSScriptRoot "$Name.stderr.log"
    $result = Invoke-ProcessWithWatchdog -FilePath $File -ArgumentList $Arguments `
        -ReportPath (Join-Path $PSScriptRoot "$Name.report.json") -StdoutPath $stdout -StderrPath $stderr `
        -ProgressPaths @($stdout, $stderr) -TimeoutSeconds 2700 -NoProgressSeconds 900
    if (-not $result.success -or -not (Select-String -LiteralPath $stdout -SimpleMatch $Expected -Quiet)) {
        throw "Cleanup check $Name failed. See $Name.report.json."
    }
    Get-Content -LiteralPath $stdout
}

Push-Location $root
try {
    $python = & (Join-Path $root "tools/python.ps1")
    Invoke-CleanupCheck "profile-generator" $python @("tools/import/gen_ragdoll.py", "--self-test") "PhysicsAsset conversion self-test PASS"
    $cargo = (Get-Command cargo).Source
    $cargoTestArguments = @("test", "--locked", "--offline", "--jobs", "2", "--profile", "play", "--lib")
    Invoke-CleanupCheck "native-ccd" $cargo ($cargoTestArguments + "ragdoll_profile_preserves_native_ccd_flag") "1 passed; 0 failed"
    Invoke-CleanupCheck "pawn-filter" $cargo ($cargoTestArguments + "pawn_ignore") "2 passed; 0 failed"
} finally {
    Pop-Location
}
