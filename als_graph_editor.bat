@echo off
cd /d "%~dp0"

pwsh -NoProfile -File "%~dp0als_graph_editor.ps1"
set "run_status=%errorlevel%"
if not defined ALS_GRAPH_EDITOR_VALIDATE_ONLY pause
exit /b %run_status%
