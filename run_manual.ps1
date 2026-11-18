$ErrorActionPreference = "Stop"
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = "2" }

Push-Location $PSScriptRoot
try {
    cargo run --jobs $env:CARGO_BUILD_JOBS --profile play --features playground --example manual_playground
    $status = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $status
