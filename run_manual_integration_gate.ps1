$ErrorActionPreference = "Stop"
$env:ALS_MANUAL_INTEGRATION_GATE = "1"
$env:RUST_LOG = "info"
. (Join-Path $PSScriptRoot "tools/run_ue_source_reader.ps1") -FunctionsOnly
foreach ($case in @("base", "ragdoll", "camera", "interface")) {
    $env:ALS_MANUAL_INTEGRATION_CASE = $case
    $logs = Join-Path $PSScriptRoot ("tools/tmp/manual_integration/" + $case)
    New-Item -ItemType Directory -Force -Path $logs | Out-Null
    $stdout = Join-Path $logs "stdout.log"
    $stderr = Join-Path $logs "stderr.log"
    $result = Invoke-ProcessWithWatchdog -FilePath (Get-Command pwsh).Source -ArgumentList @("-NoProfile", "-File", (Join-Path $PSScriptRoot "run_manual.ps1")) -ReportPath (Join-Path $logs "process.report.json") -StdoutPath $stdout -StderrPath $stderr -ProgressPaths @($stdout, $stderr) -TimeoutSeconds 3600 -NoProgressSeconds 300
    if ($result.exit_reason -ne "completed") { exit 124 }
    if ($result.process_exit_code -ne 0) { exit $result.process_exit_code }
}
exit 0
