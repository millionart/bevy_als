$ErrorActionPreference = "Stop"
$env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot "target_game"
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = "2" }

Push-Location $PSScriptRoot
try {
    cargo run --jobs $env:CARGO_BUILD_JOBS --profile play --example manual_playground
    $status = $LASTEXITCODE
    if ($IsWindows -and (Test-Path (Join-Path $env:CARGO_TARGET_DIR "play"))) {
        Get-ChildItem (Join-Path $env:CARGO_TARGET_DIR "play") -Filter *.pdb -Recurse -File | Remove-Item -Force
    }
} finally {
    Pop-Location
}
exit $status
