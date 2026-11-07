$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$python = if ($env:ALS_PYTHON) { $env:ALS_PYTHON } else { (Get-Command python3).Source }
$logs = "tools/tmp/bf144/d275"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$arguments = @("tools/import/generate_animation_unit_projection.py",
    "--raw", "docs/audit/bf144/d275/raw.native.json",
    "--compressed", "docs/audit/bf144/d275/compressed.native.json",
    "--gltf", "assets/ALS/Animations/Base/A_Als_Stand_Pose.gltf",
    "--output", "assets/ALSRuntime/Animations/Base/A_Als_Stand_Pose.gltf",
    "--descriptor", "assets/ALS/Animations/Base/A_Als_Stand_Pose.anim.ron",
    "--report", "docs/audit/bf144/d275/generated-report.json")
$result = Invoke-ProcessWithWatchdog -FilePath $python -ArgumentList $arguments `
    -ReportPath "docs/audit/bf144/d275/generator.watchdog.json" `
    -StdoutPath "$logs/generator.stdout.log" -StderrPath "$logs/generator.stderr.log" `
    -ProgressPaths @("$logs/generator.stdout.log", "$logs/generator.stderr.log") `
    -TimeoutSeconds 300 -NoProgressSeconds 90
if (-not $result.success) { throw ("Animation projection generation failed: " + $result.exit_reason) }
