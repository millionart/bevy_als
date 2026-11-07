$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$python = if ($env:ALS_PYTHON) { $env:ALS_PYTHON } else { (Get-Command python3).Source }
$logs = "tools/tmp/bf144/d262a"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$result = Invoke-ProcessWithWatchdog -FilePath $python -ArgumentList @("docs/audit/bf144/d262a/compare_products.py") `
    -ReportPath "docs/audit/bf144/d262a/compare.watchdog.json" `
    -StdoutPath "$logs/compare.stdout.log" -StderrPath "$logs/compare.stderr.log" `
    -ProgressPaths @("$logs/compare.stdout.log", "$logs/compare.stderr.log") `
    -TimeoutSeconds 300 -NoProgressSeconds 90
if (-not $result.success) { throw ("Quaternion comparison failed: " + $result.exit_reason) }
