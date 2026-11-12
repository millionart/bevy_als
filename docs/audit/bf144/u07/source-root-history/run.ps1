param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$null = New-Item -ItemType Directory -Force -Path $Directory
$stdout = Join-Path $Directory 'process.stdout.log'
$stderr = Join-Path $Directory 'process.stderr.log'
$report = Join-Path $Directory 'report.json'
$runtime = Join-Path $Directory 'root-history.json'
Remove-Item Env:ALS_MANUAL_INTEGRATION_GATE -ErrorAction SilentlyContinue
$env:ALS_MANUAL_INTEGRATION_CASE = 'startup'
$env:ALS_BOOT_AUTO_QUIT_SECS = '30'
$env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR = $Directory
$env:ALS_ANIMATION_VISUAL_CAPTURE_COUNT = '0'
$env:ALS_U07_ROOT_HISTORY_OUTPUT = $runtime
$env:CARGO_BUILD_JOBS = '2'
$result = Invoke-ProcessWithWatchdog -FilePath (Get-Command pwsh).Source -ArgumentList @('-NoProfile', '-File', 'run_manual.ps1') -ReportPath (Join-Path $Directory 'process.report.json') -StdoutPath $stdout -StderrPath $stderr -ProgressPaths @($stdout,$stderr,$report,$runtime) -TimeoutSeconds 600 -NoProgressSeconds 360
$result | ConvertTo-Json -Compress
if (!(Test-Path $report)) { throw 'Playground did not write its visual evidence' }
& ./.venv/bin/python tools/validate_animation_visual_evidence.py --report $report --min-screenshots 0 --check --functional-only | Set-Content (Join-Path $Directory 'validation.json')
if ($LASTEXITCODE -ne 0 -or !$result.success -or !(Test-Path $runtime)) { throw 'Playground root history did not pass' }
