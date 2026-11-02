param([string]$Label,[switch]$Build)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = "docs/audit/bf144/481"
$logs = "tools/tmp/bf144/481"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$binary = "$logs/current_vm"
if ($Build) {
    Remove-Item $binary -ErrorAction SilentlyContinue
    $arguments = @(Get-Content -Raw "$root/rustc-args.json" | ConvertFrom-Json)
    $arguments[2] = "bf144_replay481"
    $arguments[3] = "$root/current_vm.rs"
    $arguments[11] = $binary
    $env:CARGO_MANIFEST_DIR = (Get-Location).Path
    $result = Invoke-ProcessWithWatchdog -FilePath (Get-Command rustc).Source -ArgumentList $arguments -ReportPath "$root/current-vm-compile-$Label.report.json" -StdoutPath "$logs/current_vm-compile-$Label.stdout.log" -StderrPath "$logs/current_vm-compile-$Label.stderr.log" -ProgressPaths @("$logs/current_vm-compile-$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
    if (-not $result.success) { Get-Content -Tail 15 "$logs/current_vm-compile-$Label.stderr.log"; exit 1 }
}
$inputLabel = $Label
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) $binary) -ArgumentList @($inputLabel) -ReportPath "$root/current-vm-$Label.report.json" -StdoutPath "$logs/current_vm-$Label.stdout.log" -StderrPath "$logs/current_vm-$Label.stderr.log" -ProgressPaths @("$logs/current_vm-$Label.stdout.log", "$logs/current_vm-$Label.stderr.log") -TimeoutSeconds 60 -NoProgressSeconds 30
Get-Content -Tail 4 "$logs/current_vm-$Label.stdout.log"
if (-not $result.success) { Get-Content -TotalCount 3 "$logs/current_vm-$Label.stderr.log"; exit 1 }
