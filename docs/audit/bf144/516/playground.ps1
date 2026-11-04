param([string]$Label,[int]$HalfWidthCm,[int]$Frame=46,[string]$Control="none")
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$relative = "docs/audit/bf144/516/runtime-$Label"
if (Test-Path -LiteralPath $relative) { throw "observer evidence directory already exists" }
$null = New-Item -ItemType Directory -Path $relative
$root = Join-Path (Get-Location) $relative
$logs = Join-Path (Get-Location) "tools/tmp/bf144/516"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
Remove-Item Env:ALS_MANUAL_INTEGRATION_GATE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_BOOT_AUTO_QUIT_SECS -ErrorAction SilentlyContinue
Remove-Item Env:ALS_SHOT_PATH -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C510_EDGE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C512_SPRINT_OVERRIDE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C513_PROFILE -ErrorAction SilentlyContinue
$env:ALS_C513_PROFILE = "normal"
if ($HalfWidthCm -le 0) { throw "Positive fixture half width is required" }
$env:ALS_C516_HALF_WIDTH_CM = $HalfWidthCm.ToString()
$env:ALS_C516_FRAME = $Frame.ToString()
$env:ALS_C516_CONTROL = $Control

if ($Label -in @("acceleration-only", "cutoff")) { $env:ALS_C512_SPRINT_OVERRIDE = $Label }
Remove-Item Env:ALS_C511_LOCAL_CLAMP -ErrorAction SilentlyContinue
if ($Label -eq "clamp-local") { $env:ALS_C511_LOCAL_CLAMP = "1" }
if ($Label -eq "cancel" -or $Label -eq "braking-only") { $env:ALS_C510_EDGE = $Label }

Remove-Item Env:ALS_C510_HALF_LIFE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C510_GROUND_SNAP -ErrorAction SilentlyContinue
if ($Label -eq "config-zero") { $env:ALS_C510_HALF_LIFE = "0" }
if ($Label -eq "config-slow") { $env:ALS_C510_HALF_LIFE = "0.6" }
if ($Label -eq "ground-snap") { $env:ALS_C510_GROUND_SNAP = "1" }
$env:ALS_MANUAL_INTEGRATION_CASE = "startup"
$env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR = $relative
$env:ALS_ANIMATION_VISUAL_CAPTURE_COUNT = "0"
$env:BEVY_ASSET_ROOT = (Get-Location).Path
$env:ALS_C458_OUTPUT = "$relative/parameters.json"
$env:RUST_LOG = "info,wgpu=warn,naga=warn"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) "tools/tmp/bf144/516/playground_observed") -ArgumentList @() -ReportPath "$root/process.report.json" -StdoutPath "$logs/$Label.stdout.log" -StderrPath "$logs/$Label.stderr.log" -ProgressPaths @("$root/parameters.progress.json") -TimeoutSeconds 300 -NoProgressSeconds 90
Get-Content -Tail 10 "$logs/$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 12 "$logs/$Label.stderr.log"; exit 1 }
if (-not (Test-Path "$root/parameters.json")) { throw "observer exited without a complete parameter report" }
