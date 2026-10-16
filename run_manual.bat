@echo off
title bevy_als - ALS Playground
cd /d "%~dp0"

pwsh -NoProfile -File "%~dp0run_manual.ps1"
set "run_status=%errorlevel%"

if %run_status% neq 0 (
    echo.
    echo Exited with error %run_status%.
    if not defined ALS_MANUAL_INTEGRATION_GATE pause
    exit /b %run_status%
)

exit /b 0
