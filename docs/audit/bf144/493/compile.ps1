param([string]$Label = "baseline")
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$env:CARGO_MANIFEST_DIR = (Get-Location).Path
$root = Join-Path (Get-Location) "docs/audit/bf144/493"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/493"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$arguments = @(Get-Content -Raw "$root/observer-rustc-args.json" | ConvertFrom-Json)
$result = Invoke-ProcessWithWatchdog -FilePath (Get-Command rustc).Source -ArgumentList $arguments -ReportPath "$root/compile-$Label.report.json" -StdoutPath "$logs/compile-$Label.stdout.log" -StderrPath "$logs/compile-$Label.stderr.log" -ProgressPaths @("$logs/compile-$Label.stdout.log", "$logs/compile-$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
Get-Content -Tail 15 "$logs/compile-$Label.stderr.log"
if (-not $result.success) { exit 1 }
