@echo off
title bevy_als - ALS Playground
cd /d "%~dp0"

if exist "target\release\examples\manual_playground.exe" (
    echo Starting ALS Playground...
    start "" "target\release\examples\manual_playground.exe"
) else (
    echo Executable not found. Building first time...
    cargo build --release --example manual_playground
    if %errorlevel% neq 0 (
        echo Build failed!
        pause
        exit /b 1
    )
    start "" "target\release\examples\manual_playground.exe"
)
