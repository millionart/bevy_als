@echo off
title bevy_als - ALS Playground
cd /d "%~dp0"

set "CARGO_TARGET_DIR=%~dp0target_game"

cargo run --profile play --example manual_playground
set "run_status=%errorlevel%"
for /r "%CARGO_TARGET_DIR%\play" %%F in (*.pdb) do @del /q "%%F"

if %run_status% neq 0 (
    echo.
    echo Exited with error %run_status%.
    pause
)
