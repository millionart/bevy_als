param([string]$Label,[switch]$Build)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = "docs/audit/bf144/477"
$logs = "tools/tmp/bf144/477"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$binary = "$logs/replay"
if ($Build) {
    Remove-Item $binary -ErrorAction SilentlyContinue
    $arguments = @(Get-Content -Raw "$root/rustc-args.json" | ConvertFrom-Json)
    $arguments[2] = "bf144_replay477"
    $arguments[3] = "$root/replay.rs"
    $arguments[11] = $binary
    $env:CARGO_MANIFEST_DIR = (Get-Location).Path
    $result = Invoke-ProcessWithWatchdog -FilePath (Get-Command rustc).Source -ArgumentList $arguments -ReportPath "$root/replay-compile-$Label.report.json" -StdoutPath "$logs/replay-compile-$Label.stdout.log" -StderrPath "$logs/replay-compile-$Label.stderr.log" -ProgressPaths @("$logs/replay-compile-$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
    if (-not $result.success) { Get-Content -Tail 15 "$logs/replay-compile-$Label.stderr.log"; exit 1 }
}
$inputLabel = $Label
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) $binary) -ArgumentList @($inputLabel) -ReportPath "$root/replay-$Label.report.json" -StdoutPath "$logs/replay-$Label.stdout.log" -StderrPath "$logs/replay-$Label.stderr.log" -ProgressPaths @("$logs/replay-$Label.stdout.log", "$logs/replay-$Label.stderr.log") -TimeoutSeconds 60 -NoProgressSeconds 30
Get-Content -Tail 4 "$logs/replay-$Label.stdout.log"
if (-not $result.success) { Get-Content -TotalCount 3 "$logs/replay-$Label.stderr.log"; exit 1 }
