@echo off
title bevy_als - ALS Playground
cd /d "%~dp0"

cargo run --example manual_playground

if %errorlevel% neq 0 (
    echo.
    echo Exited with error %errorlevel%.
    pause
)
