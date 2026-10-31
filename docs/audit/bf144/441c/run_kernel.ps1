param([string]$Label,[switch]$Build)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/441c"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/441c"
if ($Build) {
    $program = (Get-Command rustc).Source
    $arguments = @(Get-Content -Raw "$root/kernel-args.json" | ConvertFrom-Json)
} else {
    $program = "$logs/kernel"
    $arguments = @("$root/$Label.json")
}
$result = Invoke-ProcessWithWatchdog -FilePath $program -ArgumentList $arguments -ReportPath "$root/$Label.report.json" -StdoutPath "$logs/$Label.stdout.log" -StderrPath "$logs/$Label.stderr.log" -ProgressPaths @("$logs/$Label.stdout.log", "$logs/$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
Get-Content -Tail 4 "$logs/$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 14 "$logs/$Label.stderr.log"; exit 1 }
