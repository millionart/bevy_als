param([string]$Name)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/441b"
$stdout = Join-Path $root ($Name + ".stdout.log")
$stderr = Join-Path $root ($Name + ".stderr.log")
$result = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) ".venv/bin/python") -ArgumentList @((Join-Path $root ($Name + ".py"))) -ReportPath (Join-Path $root ($Name + ".report.json")) -StdoutPath $stdout -StderrPath $stderr -ProgressPaths @($stdout, $stderr) -TimeoutSeconds 300 -NoProgressSeconds 30
Get-Content -Tail 4 $stdout
if (-not $result.success) { Get-Content $stderr; exit 1 }
