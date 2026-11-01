$ErrorActionPreference = "Stop"
Remove-Item Env:ALS_SHOT_PATH -ErrorAction SilentlyContinue
& pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case startup -OutputDirectory docs/audit/bf144/443b/startup-control -CaptureCount 0 -FunctionalOnly
exit $LASTEXITCODE
