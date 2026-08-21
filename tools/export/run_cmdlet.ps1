# M0.5 commandlet 启动器（PowerShell 版，规避 bash 间歇性丢参）
# 用法: powershell -File tools/export/run_cmdlet.ps1 <script.py> [附加 UE 参数...]
# 路径经 tools/env.local.json 注入，禁止硬编码
param(
    [Parameter(Mandatory = $true)][string]$Script,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$UeArgs
)
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..' | Join-Path -ChildPath '..')

$envJson = 'tools/env.local.json'
if (-not (Test-Path $envJson)) {
    Write-Error "缺少 $envJson（复制 tools/env.example.json 并填入本机路径）"
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
