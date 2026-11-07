$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$python = if ($env:ALS_PYTHON) { $env:ALS_PYTHON } else { (Get-Command python3).Source }
$logs = "tools/tmp/bf144/d267"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$arguments = @("tools/import/generate_grounded_dynamics_curves.py",
    "--native", "docs/audit/bf144/d267/curves.native.json",
    "--normal-settings", "docs/audit/bf144/d267/settings_normal.native.json",
    "--responsive-settings", "docs/audit/bf144/d267/settings_responsive.native.json",
    "--sluggish-settings", "docs/audit/bf144/d267/settings_sluggish.native.json",
    "--baseline", "docs/audit/bf144/513/runtime-baseline/profile.json",
    "--output", "assets/ALS/Data/Character/Movement/grounded_dynamics.runtime.json",
    "--report", "docs/audit/bf144/d267/generated-report.json")
$result = Invoke-ProcessWithWatchdog -FilePath $python -ArgumentList $arguments `
    -ReportPath "docs/audit/bf144/d267/generator.watchdog.json" `
    -StdoutPath "$logs/generator.stdout.log" -StderrPath "$logs/generator.stderr.log" `
    -ProgressPaths @("$logs/generator.stdout.log", "$logs/generator.stderr.log") `
    -TimeoutSeconds 300 -NoProgressSeconds 90
if (-not $result.success) { throw ("Grounded dynamics generation failed: " + $result.exit_reason) }
