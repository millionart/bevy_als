param([string]$Label,[switch]$Build)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = "docs/audit/bf144/445"
$logs = "tools/tmp/bf144/445"
$binary = "$logs/fixture"
if ($Build) {
    Remove-Item $binary -ErrorAction SilentlyContinue
    $arguments = @(Get-Content -Raw "$root/rustc-args.json" | ConvertFrom-Json)
    $arguments[2] = "bf144_fixture445"
    $arguments[3] = "$root/fixture.rs"
    $arguments[11] = $binary
    $env:CARGO_MANIFEST_DIR = (Get-Location).Path
    $result = Invoke-ProcessWithWatchdog -FilePath (Get-Command rustc).Source -ArgumentList $arguments -ReportPath "$root/fixture-compile-$Label.report.json" -StdoutPath "$logs/fixture-compile-$Label.stdout.log" -StderrPath "$logs/fixture-compile-$Label.stderr.log" -ProgressPaths @("$logs/fixture-compile-$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
    if (-not $result.success) { Get-Content -Tail 15 "$logs/fixture-compile-$Label.stderr.log"; exit 1 }
}
$inputLabel = $Label
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) $binary) -ArgumentList @($inputLabel) -ReportPath "$root/fixture-$Label.report.json" -StdoutPath "$logs/fixture-$Label.stdout.log" -StderrPath "$logs/fixture-$Label.stderr.log" -ProgressPaths @("$logs/fixture-$Label.stdout.log", "$logs/fixture-$Label.stderr.log") -TimeoutSeconds 60 -NoProgressSeconds 30
Get-Content -Tail 4 "$logs/fixture-$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 15 "$logs/fixture-$Label.stderr.log"; exit 1 }
