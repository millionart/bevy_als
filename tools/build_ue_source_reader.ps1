$ErrorActionPreference = "Stop"
$config = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot "env.local.json") | ConvertFrom-Json
$engine = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $config.ue_cmd))
$uat = Join-Path $engine "Build/BatchFiles/RunUAT.bat"
$plugin = Join-Path $PSScriptRoot "ue_graph_source_reader/AlsGraphSourceReader.uplugin"
$targetRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target")) + [IO.Path]::DirectorySeparatorChar
$package = [IO.Path]::GetFullPath((Join-Path $targetRoot "ue_graph_source_reader_package"))
if (-not $package.StartsWith($targetRoot, [StringComparison]::OrdinalIgnoreCase)) {
    throw "reader package target is outside the workspace target directory"
}
& $uat BuildPlugin "-Plugin=$plugin" "-Package=$package" -TargetPlatforms=Win64 -Rocket -MaxParallelActions=2
exit $LASTEXITCODE
