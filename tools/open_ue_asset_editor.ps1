param(
    [Parameter(Mandatory = $true)]
    [string]$Asset
)

$configPath = Join-Path $PSScriptRoot "env.local.json"
if (-not (Test-Path -LiteralPath $configPath)) {
    throw "tools/env.local.json is required"
}

$config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
$scriptPath = Join-Path $PSScriptRoot "ue_open_asset.py"

if (-not (Test-Path -LiteralPath $config.ue_cmd)) {
    throw "ue_cmd does not exist"
}
if (-not (Test-Path -LiteralPath $config.ue_project)) {
    throw "ue_project does not exist"
}
if (-not (Test-Path -LiteralPath $scriptPath)) {
    throw "ue_open_asset.py does not exist"
}

$ueScriptPath = (Resolve-Path -LiteralPath $scriptPath).Path.Replace('\', '/')
$argument = '-ExecutePythonScript="' + $ueScriptPath + ' ' + $Asset + '"'
. (Join-Path $PSScriptRoot "run_ue_source_reader.ps1") -FunctionsOnly
$arguments = (@($config.ue_project, $argument) | ForEach-Object { ConvertTo-ProcessArgument $_ }) -join " "
Start-Process -FilePath $config.ue_cmd -ArgumentList $arguments
