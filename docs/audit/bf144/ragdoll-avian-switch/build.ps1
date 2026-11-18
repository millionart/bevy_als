param([string]$Attempt = "build")

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "../../../..")).Path
$env:CARGO_BUILD_JOBS = "2"
. (Join-Path $root "tools/run_ue_source_reader.ps1") -FunctionsOnly

$stdout = Join-Path $PSScriptRoot "$Attempt.stdout.log"
$stderr = Join-Path $PSScriptRoot "$Attempt.stderr.log"
Push-Location $root
try {
    $result = Invoke-ProcessWithWatchdog -FilePath (Get-Command cargo).Source `
        -ArgumentList @("build", "--locked", "--offline", "--jobs", "2", "--profile", "play", "--example", "manual_playground") `
        -ReportPath (Join-Path $PSScriptRoot "$Attempt.report.json") -StdoutPath $stdout -StderrPath $stderr `
        -ProgressPaths @($stdout, $stderr) -TimeoutSeconds 2700 -NoProgressSeconds 900
    $result | ConvertTo-Json -Compress
    if (-not $result.success) { exit 1 }
} finally {
    Pop-Location
}
