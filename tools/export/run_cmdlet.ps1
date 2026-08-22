param(
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
New-Item -ItemType Directory -Force -Path 'tmp/export' | Out-Null
$log  = Join-Path (Get-Location) ("tmp/export/cmdlet_{0}.log" -f (Get-Date -UFormat %s))
$env:BEVY_ALS_ROOT = (Get-Location).Path

$allArgs = @($PROJ, '-run=pythonscript', "-script=$full", '-stdout', '-ForceLogFlush', "-abslog=$log") + $UeArgs
& $UE @allArgs
$ec = $LASTEXITCODE

Write-Host "== exit=$ec log=$log =="
Select-String -Path $log -Pattern 'M0EXPORT|PYEXPORT|Error:|Fatal|LogPython' |
    Select-Object -Last 60 | ForEach-Object Line
exit $ec
