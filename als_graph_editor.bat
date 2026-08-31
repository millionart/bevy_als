@echo off
cd /d "%~dp0"

python tools\validate_animgraph_editor_assets.py --root assets\graphs
if errorlevel 1 (
    echo.
    echo Graph editor asset validation failed. Run the repair command with --fix.
    pause
    exit /b 1
)

if defined ALS_GRAPH_EDITOR_VALIDATE_ONLY exit /b 0

cargo run --example als_graph_editor --jobs 2 -- --asset-source .\assets
pause
