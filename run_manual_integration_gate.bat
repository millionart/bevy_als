@echo off
setlocal EnableDelayedExpansion
cd /d "%~dp0"

set "ALS_MANUAL_INTEGRATION_GATE=1"
set "RUST_LOG=info"
if "%CARGO_BUILD_JOBS%"=="" set "CARGO_BUILD_JOBS=2"

for %%C in (base ragdoll camera interface) do (
    set "ALS_MANUAL_INTEGRATION_CASE=%%C"
    call run_manual.bat
    if errorlevel 1 exit /b !errorlevel!
)

exit /b 0
