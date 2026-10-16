$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$python = & (Join-Path $PSScriptRoot "tools/python.ps1")
& $python tools/validate_animgraph_editor_assets.py --root assets/graphs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if ($env:ALS_GRAPH_EDITOR_VALIDATE_ONLY) { exit 0 }
cargo run --example als_graph_editor --jobs 2 -- --asset-source ./assets
exit $LASTEXITCODE
