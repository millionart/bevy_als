param([string]$Label = "baseline")
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$relative = "docs/audit/bf144/505/runtime-$Label"
if (Test-Path -LiteralPath $relative) { throw "observer evidence directory already exists" }
$null = New-Item -ItemType Directory -Path $relative
$root = Join-Path (Get-Location) $relative
$logs = Join-Path (Get-Location) "tools/tmp/bf144/505"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
Remove-Item Env:ALS_MANUAL_INTEGRATION_GATE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_BOOT_AUTO_QUIT_SECS -ErrorAction SilentlyContinue
Remove-Item Env:ALS_SHOT_PATH -ErrorAction SilentlyContinue
$env:ALS_MANUAL_INTEGRATION_CASE = "startup"
$env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR = $relative
$env:ALS_ANIMATION_VISUAL_CAPTURE_COUNT = "0"
$env:BEVY_ASSET_ROOT = (Get-Location).Path
$env:ALS_C458_OUTPUT = "$relative/parameters.json"
$env:RUST_LOG = "info,wgpu=warn,naga=warn"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) "tools/tmp/bf144/505/playground_observed") -ArgumentList @() -ReportPath "$root/process.report.json" -StdoutPath "$logs/$Label.stdout.log" -StderrPath "$logs/$Label.stderr.log" -ProgressPaths @("$root/parameters.progress.json") -TimeoutSeconds 300 -NoProgressSeconds 90
Get-Content -Tail 10 "$logs/$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 12 "$logs/$Label.stderr.log"; exit 1 }
if (-not (Test-Path "$root/parameters.json")) { throw "observer exited without a complete parameter report" }
