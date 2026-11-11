param([string]$Attempt = "edge-support")

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "../../../..")).Path
$env:CARGO_TARGET_DIR = Join-Path $root "target_game"
. (Join-Path $root "tools/run_ue_source_reader.ps1") -FunctionsOnly

$stdout = Join-Path $PSScriptRoot "$Attempt.stdout.log"
$stderr = Join-Path $PSScriptRoot "$Attempt.stderr.log"
$test = "source_physics_timing::tests::idle_character_keeps_edge_support_during_a_clamped_physics_frame"
Push-Location $root
try {
    $result = Invoke-ProcessWithWatchdog -FilePath (Get-Command cargo).Source `
        -ArgumentList @("test", "--locked", "--offline", "--jobs", "2", "--profile", "play", "--lib", $test, "--", "--exact", "--nocapture") `
        -ReportPath (Join-Path $PSScriptRoot "$Attempt.report.json") -StdoutPath $stdout -StderrPath $stderr `
        -ProgressPaths @($stdout, $stderr) -TimeoutSeconds 2700 -NoProgressSeconds 900
    $result | ConvertTo-Json -Compress
    if (-not $result.success) { exit 1 }
    if (-not (Select-String -LiteralPath $stdout -SimpleMatch "$test ... ok" -Quiet)) { exit 2 }
} finally {
    Pop-Location
}
