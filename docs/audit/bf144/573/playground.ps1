param([ValidateSet("baseline","cached")][string]$Label)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$relative = "docs/audit/bf144/573/runtime-$Label"
if (Test-Path -LiteralPath $relative) { throw "observer evidence directory already exists" }
$null = New-Item -ItemType Directory -Path $relative
$root = Join-Path (Get-Location) $relative
$logs = Join-Path (Get-Location) "tools/tmp/bf144/572"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
Remove-Item Env:ALS_MANUAL_INTEGRATION_GATE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_BOOT_AUTO_QUIT_SECS -ErrorAction SilentlyContinue
Remove-Item Env:ALS_SHOT_PATH -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C510_EDGE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C512_SPRINT_OVERRIDE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C513_PROFILE -ErrorAction SilentlyContinue
$env:ALS_C513_PROFILE = "normal"
$env:ALS_C516_HALF_WIDTH_CM = "100"
$env:ALS_C516_FRAME = "40"
$env:ALS_C516_CONTROL = "none"
Remove-Item Env:ALS_C520_ALIGN -ErrorAction SilentlyContinue
$env:ALS_C523_DECK = "1"
$env:ALS_C524_PITCH_DEG = "0"
$env:ALS_C572_CASE = "coarse"
$env:ALS_C573_MODE = $Label
Remove-Item Env:ALS_C530_BLOCKER -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C532_REQUIRE_STOP_FLOOR -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C527_SHIFT_M -ErrorAction SilentlyContinue
$observer = "tools/tmp/bf144/573/playground_observed"
Remove-Item Env:ALS_C518_NEUTRALIZE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C519_WALL_CM -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C511_LOCAL_CLAMP -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C510_HALF_LIFE -ErrorAction SilentlyContinue
Remove-Item Env:ALS_C510_GROUND_SNAP -ErrorAction SilentlyContinue
$env:ALS_MANUAL_INTEGRATION_CASE = "startup"
$env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR = $relative
$env:ALS_ANIMATION_VISUAL_CAPTURE_COUNT = "0"
$env:BEVY_ASSET_ROOT = (Get-Location).Path
$env:ALS_C458_OUTPUT = "$relative/parameters.json"
$env:RUST_LOG = "info,wgpu=warn,naga=warn"
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) $observer) -ArgumentList @() -ReportPath "$root/process.report.json" -StdoutPath "$logs/$Label.stdout.log" -StderrPath "$logs/$Label.stderr.log" -ProgressPaths @("$root/parameters.progress.json") -TimeoutSeconds 300 -NoProgressSeconds 90
Get-Content -Tail 10 "$logs/$Label.stdout.log"
if (-not $result.success) { Get-Content -Tail 12 "$logs/$Label.stderr.log"; exit 1 }
if (-not (Test-Path "$root/parameters.json")) { throw "observer exited without a complete parameter report" }
