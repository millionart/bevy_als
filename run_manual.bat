@echo off
title bevy_als - ALS Playground
cd /d "%~dp0"

rem Isolated target dir: never contends with the shared target/ lock
rem used by other worktrees building against this repo.
set "CARGO_TARGET_DIR=%~dp0target_game"

cargo run --example manual_playground

if %errorlevel% neq 0 (
    echo.
    echo Exited with error %errorlevel%.
    pause
)
