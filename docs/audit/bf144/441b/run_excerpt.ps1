param([string]$Label,[switch]$Build)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = Join-Path (Get-Location) "docs/audit/bf144/441b"
$binary = Join-Path (Get-Location) "tools/tmp/bf144/441b/curve_excerpt"
$null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $binary)
if ($Build) {
    $program = (Get-Command rustc).Source
    $arguments = @("--edition=2024", "$root/curve_excerpt.rs", "-C", "opt-level=1", "-o", $binary)
} else {
    $program = "$binary"
    $arguments = @("$root/$Label.f32le")
}
$result = Invoke-ProcessWithWatchdog -FilePath $program -ArgumentList $arguments -ReportPath "$root/$Label.report.json" -StdoutPath "$root/$Label.stdout.log" -StderrPath "$root/$Label.stderr.log" -ProgressPaths @("$root/$Label.stdout.log", "$root/$Label.stderr.log") -TimeoutSeconds 180 -NoProgressSeconds 30
Get-Content -Tail 4 "$root/$Label.stdout.log"
if (-not $result.success) { Get-Content "$root/$Label.stderr.log"; exit 1 }
