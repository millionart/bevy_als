$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$env:CARGO_MANIFEST_DIR = (Get-Location).Path
$root = Join-Path (Get-Location) "docs/audit/bf144/442"
$logs = Join-Path (Get-Location) "tools/tmp/bf144/442"
$arguments = @(Get-Content -Raw "$root/rustc-args.json" | ConvertFrom-Json)
$result = Invoke-ProcessWithWatchdog -FilePath (Get-Command rustc).Source -ArgumentList $arguments -ReportPath "$root/compile.report.json" -StdoutPath "$logs/compile.stdout.log" -StderrPath "$logs/compile.stderr.log" -ProgressPaths @("$logs/compile.stdout.log", "$logs/compile.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
Get-Content -Tail 15 "$logs/compile.stderr.log"
if (-not $result.success) { exit 1 }
