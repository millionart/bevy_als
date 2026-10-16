param(
    [string]$Map,
    [Parameter(Mandatory = $true)][string]$Script,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$UeArgs
)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..' | Join-Path -ChildPath '..')

$envJson = 'tools/env.local.json'
if (-not (Test-Path $envJson)) {
    Write-Error "missing $envJson (copy tools/env.example.json and fill in local paths)"
    exit 2
}
$cfg = Get-Content $envJson -Raw | ConvertFrom-Json
$UE   = $cfg.ue_cmd
$PROJ = $cfg.ue_project

$full = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Script))
New-Item -ItemType Directory -Force -Path 'tools/tmp/export' | Out-Null
$log  = Join-Path (Get-Location) ("tools/tmp/export/cmdlet_{0}.log" -f (Get-Date -UFormat %s))
$env:BEVY_ALS_ROOT = (Get-Location).Path

$allArgs = @($PROJ) + $(if ($Map) { , $Map } else { @() }) +
    @('-run=pythonscript', "-script=$full", '-stdout', '-ForceLogFlush', "-abslog=$log") +
    $UeArgs
. (Join-Path $PSScriptRoot "../run_ue_source_reader.ps1") -FunctionsOnly
$result = Invoke-ProcessWithWatchdog -FilePath $UE -ArgumentList $allArgs -ReportPath ($log + ".report.json") -StdoutPath ($log + ".stdout.log") -StderrPath ($log + ".stderr.log") -ProgressPaths @($log, ($log + ".stdout.log"), ($log + ".stderr.log")) -TimeoutSeconds 3600 -NoProgressSeconds 300
$ec = if ($result.exit_reason -eq "completed") { $result.process_exit_code } else { 124 }

Write-Host "== exit=$ec log=$log =="
if (Test-Path -LiteralPath $log) {
    Select-String -LiteralPath $log -Pattern 'M0EXPORT|PYEXPORT|Error:|Fatal|LogPython' |
        Select-Object -Last 60 | ForEach-Object Line
}
exit $ec
