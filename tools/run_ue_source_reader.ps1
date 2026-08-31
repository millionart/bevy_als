param(
    [Parameter(Mandatory = $true)]
    [string]$Asset,
    [Parameter(Mandatory = $true)]
    [string]$Output,
    [string]$Plugin = (Join-Path $PSScriptRoot "..\target\ue_graph_source_reader_package\HostProject\Plugins\AlsGraphSourceReader\AlsGraphSourceReader.uplugin")
)

$configPath = Join-Path $PSScriptRoot "env.local.json"
if (-not (Test-Path -LiteralPath $configPath)) {
    throw "tools/env.local.json is required"
}

$config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
$pluginPath = [IO.Path]::GetFullPath($Plugin)
$outputPath = [IO.Path]::GetFullPath($Output)

if (-not (Test-Path -LiteralPath $config.ue_cmd)) {
    throw "ue_cmd does not exist"
}
if (-not (Test-Path -LiteralPath $config.ue_project)) {
    throw "ue_project does not exist"
}
if (-not (Test-Path -LiteralPath $pluginPath)) {
    throw "source reader plugin does not exist"
}

& $config.ue_cmd $config.ue_project "-PLUGIN=$pluginPath" "-EnablePlugins=AlsGraphSourceReader" "-DisablePlugins=ModelContextProtocol" "-run=AlsGraphSourceReader" "-Asset=$Asset" "-Output=$outputPath" "-unattended" "-nop4" "-nosplash" "-stdout" "-FullStdOutLogOutput"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
