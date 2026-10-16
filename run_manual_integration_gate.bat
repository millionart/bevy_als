@echo off
cd /d "%~dp0"
pwsh -NoProfile -File "%~dp0run_manual_integration_gate.ps1"
exit /b %errorlevel%
