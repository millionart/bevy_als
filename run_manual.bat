@echo off
title bevy_als - ALS Playground Manual Test
cd /d "%~dp0"

echo ============================================
echo   ALS Playground Manual Test
echo ============================================
echo.
echo   Controls:
echo     WASD       Move
echo     Mouse      Look
echo     Space      Jump
echo     L-Ctrl     Crouch toggle
echo     L-Shift    Sprint (hold)
echo     R-Mouse    Aim (hold)
echo     L-Alt      Roll / Dodge
echo.
echo   Close window or press Ctrl+C to exit.
echo ============================================
echo.

echo [1/2] Building (release)...
cargo build --release --example manual_playground 2>nul
if %errorlevel% neq 0 (
    echo Build failed! Check errors above.
    pause
    exit /b 1
)

echo [2/2] Running...
echo.
cargo run --release --example manual_playground --quiet
pause
