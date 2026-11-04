param([string]$Label,[switch]$Ablate)
$ErrorActionPreference = "Stop"
. ./tools/run_ue_source_reader.ps1 -FunctionsOnly
$root = "docs/audit/bf144/521"
$logs = "tools/tmp/bf144/521"
$binary = "$logs/replay"
$source = "$root/replay.rs"
$original = [IO.File]::ReadAllBytes($source)
$arguments = @(Get-Content -Raw "$root/observer-rustc-args.json" | ConvertFrom-Json)
$arguments[2] = "bf144_replay521"
$arguments[3] = $source
$arguments[11] = $binary
$env:CARGO_MANIFEST_DIR = (Get-Location).Path
try {
    if ($Ablate) {
        $text = [Text.Encoding]::UTF8.GetString($original)
        if (-not $text.Contains('const REPLACE_NORMAL: bool = true;')) { throw "Expected enabled normal control" }
        [IO.File]::WriteAllText($source, $text.Replace('const REPLACE_NORMAL: bool = true;', 'const REPLACE_NORMAL: bool = false;'))
    }
    $build = Invoke-ProcessWithWatchdog -FilePath (Get-Command rustc).Source -ArgumentList $arguments -ReportPath "$root/replay-compile-$Label.report.json" -StdoutPath "$logs/replay-compile-$Label.stdout.log" -StderrPath "$logs/replay-compile-$Label.stderr.log" -ProgressPaths @("$logs/replay-compile-$Label.stderr.log") -TimeoutSeconds 300 -NoProgressSeconds 120
    if (-not $build.success) { Get-Content -Tail 15 "$logs/replay-compile-$Label.stderr.log"; throw "Replay compile failed" }
    $run = Invoke-ProcessWithWatchdog -FilePath (Join-Path (Get-Location) $binary) -ArgumentList @($Label) -ReportPath "$root/replay-$Label.report.json" -StdoutPath "$logs/replay-$Label.stdout.log" -StderrPath "$logs/replay-$Label.stderr.log" -ProgressPaths @("$logs/replay-$Label.stdout.log", "$logs/replay-$Label.stderr.log") -TimeoutSeconds 60 -NoProgressSeconds 30
    if ($Ablate) {
        $errorText = Get-Content -Raw "$logs/replay-$Label.stderr.log"
        if ($run.process_exit_code -ne 101 -or $errorText -notmatch 'Native normal replacement must affect the live slide') { throw "Ablation did not fail the normal control" }
    } elseif (-not $run.success) {
        Get-Content -Tail 10 "$logs/replay-$Label.stderr.log"
        throw "Replay failed"
    }
} finally {
    if ($Ablate) { [IO.File]::WriteAllBytes($source, $original) }
}
