[CmdletBinding()]
param(
    [ValidateSet("startup", "base", "grounded", "standing_direction", "standing_response", "standing_stop", "standing_turn", "standing_dynamic", "crouching", "crouching_actions", "locomotion", "control_rig", "default_overlay", "bow", "overlays", "footprint", "head", "state_machine", "transition_notify", "foot_ik", "sprint", "ragdoll", "get_up", "camera", "stairs", "uphill_jump", "moving_base", "rotating_base", "interface", "notify")]
    [string]$Case = "state_machine",
    [string]$OutputDirectory,
    [int]$CaptureCount = 0,
    [double]$FrameDeltaSeconds = 0.0,
    [double]$FrameHitchSeconds = 0.0,
    [double]$CaptureIntervalSeconds = 1.5,
    [double]$MaxFrameP95Ms = 25.0,
    [double]$MaxFrameMs = 75.0,
    [double]$MaxEvalP95Ms = 4.0,
    [int]$MaxPhysicsStepsPerRender = 3,
    [switch]$FunctionalOnly
)

if ($CaptureCount -lt 0 -or $CaptureIntervalSeconds -le 0 -or $MaxFrameP95Ms -le 0 -or $MaxFrameMs -le 0 -or $MaxEvalP95Ms -le 0 -or $MaxPhysicsStepsPerRender -lt 1 -or -not [double]::IsFinite($FrameDeltaSeconds) -or $FrameDeltaSeconds -lt 0 -or $FrameDeltaSeconds -gt 0.25 -or ($Case -eq "startup" -and $FrameDeltaSeconds -gt 0) -or -not [double]::IsFinite($FrameHitchSeconds) -or $FrameHitchSeconds -lt 0 -or $FrameHitchSeconds -gt 0.25 -or ($FrameHitchSeconds -gt 0 -and ($FrameDeltaSeconds -le 0 -or $FrameHitchSeconds -le $FrameDeltaSeconds -or $Case -ne "moving_base"))) {
    throw "Invalid visual validation arguments."
}

$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$evidenceRoot = [IO.Path]::GetFullPath((Join-Path $workspaceRoot "target\animation_visual"))
$candidate = if ($OutputDirectory) {
    [IO.Path]::GetFullPath((Join-Path $workspaceRoot $OutputDirectory))
} else {
    [IO.Path]::GetFullPath((Join-Path $evidenceRoot $Case))
}
$rootPrefix = $evidenceRoot.TrimEnd('\') + '\'
if (-not $candidate.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputDirectory must stay under target\\animation_visual."
}
if (Test-Path -LiteralPath $candidate) {
    Remove-Item -LiteralPath $candidate -Recurse -Force
}
New-Item -ItemType Directory -Path $candidate -Force | Out-Null

$variables = @(
    "ALS_MANUAL_INTEGRATION_GATE",
    "ALS_MANUAL_INTEGRATION_CASE",
    "ALS_MANUAL_INTEGRATION_FRAME_DELTA_SECS",
    "ALS_MANUAL_INTEGRATION_FRAME_HITCH_SECS",
    "ALS_ANIMATION_VISUAL_EVIDENCE_DIR",
    "ALS_ANIMATION_VISUAL_CAPTURE_COUNT",
    "ALS_ANIMATION_VISUAL_CAPTURE_INTERVAL_SECS",
    "ALS_ANIMATION_VISUAL_MAX_FRAME_P95_MS",
    "ALS_ANIMATION_VISUAL_MAX_FRAME_MS",
    "ALS_ANIMATION_VISUAL_MAX_EVAL_P95_MS",
    "ALS_ANIMATION_VISUAL_MAX_PHYSICS_STEPS_PER_RENDER",
    "ALS_BOOT_AUTO_QUIT_SECS"
)
$previous = @{}
foreach ($name in $variables) {
    $item = Get-Item -LiteralPath ("Env:" + $name) -ErrorAction SilentlyContinue
    $previous[$name] = if ($null -eq $item) { $null } else { $item.Value }
}

try {
    if ($Case -eq "startup") {
        Remove-Item Env:ALS_MANUAL_INTEGRATION_GATE -ErrorAction SilentlyContinue
        $minimumStartupSeconds = 5.0 + $CaptureCount * $CaptureIntervalSeconds
        $env:ALS_BOOT_AUTO_QUIT_SECS = [Math]::Max(10.0, $minimumStartupSeconds).ToString([Globalization.CultureInfo]::InvariantCulture)
    } else {
        $env:ALS_MANUAL_INTEGRATION_GATE = "1"
    }
    $env:ALS_MANUAL_INTEGRATION_CASE = $Case
    if ($FrameDeltaSeconds -gt 0) {
        $env:ALS_MANUAL_INTEGRATION_FRAME_DELTA_SECS = $FrameDeltaSeconds.ToString([Globalization.CultureInfo]::InvariantCulture)
    } else {
        Remove-Item Env:ALS_MANUAL_INTEGRATION_FRAME_DELTA_SECS -ErrorAction SilentlyContinue
    }
    if ($FrameHitchSeconds -gt 0) {
        $env:ALS_MANUAL_INTEGRATION_FRAME_HITCH_SECS = $FrameHitchSeconds.ToString([Globalization.CultureInfo]::InvariantCulture)
    } else {
        Remove-Item Env:ALS_MANUAL_INTEGRATION_FRAME_HITCH_SECS -ErrorAction SilentlyContinue
    }
    $env:ALS_ANIMATION_VISUAL_EVIDENCE_DIR = $candidate
    $env:ALS_ANIMATION_VISUAL_CAPTURE_COUNT = $CaptureCount.ToString([Globalization.CultureInfo]::InvariantCulture)
    $env:ALS_ANIMATION_VISUAL_CAPTURE_INTERVAL_SECS = $CaptureIntervalSeconds.ToString([Globalization.CultureInfo]::InvariantCulture)
    $env:ALS_ANIMATION_VISUAL_MAX_FRAME_P95_MS = $MaxFrameP95Ms.ToString([Globalization.CultureInfo]::InvariantCulture)
    $env:ALS_ANIMATION_VISUAL_MAX_FRAME_MS = $MaxFrameMs.ToString([Globalization.CultureInfo]::InvariantCulture)
    $env:ALS_ANIMATION_VISUAL_MAX_EVAL_P95_MS = $MaxEvalP95Ms.ToString([Globalization.CultureInfo]::InvariantCulture)
    $env:ALS_ANIMATION_VISUAL_MAX_PHYSICS_STEPS_PER_RENDER = $MaxPhysicsStepsPerRender.ToString([Globalization.CultureInfo]::InvariantCulture)
    Push-Location $workspaceRoot
    try {
        & cmd.exe /d /c "run_manual.bat"
        $manualExitCode = $LASTEXITCODE
        $reportPath = Join-Path $candidate "report.json"
        $runnerPath = Join-Path $candidate "runner.json"
        $runner = [ordered]@{
            format = "als_animation_visual_runner_v1"
            case = $Case
            frame_delta_seconds = if ($FrameDeltaSeconds -gt 0) { $FrameDeltaSeconds } else { $null }
            frame_hitch_seconds = if ($FrameHitchSeconds -gt 0) { $FrameHitchSeconds } else { $null }
            manual_exit_code = $manualExitCode
            report_path = $reportPath
            report_exists = Test-Path -LiteralPath $reportPath -PathType Leaf
            validation_exit_code = $null
            validation_scope = if ($FunctionalOnly) { "functional" } else { "full" }
            render_effects_passed = $false
            decal_renderer_passed = $false
        }
        if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
            $runner | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $runnerPath -Encoding utf8
            throw "manual_playground exited with code $manualExitCode and did not write $reportPath."
        }
        $validationArguments = @('tools/validate_animation_visual_evidence.py', '--report', $reportPath, '--min-screenshots', $CaptureCount, '--check')
        if ($FunctionalOnly) { $validationArguments += '--functional-only' }
        if ($FrameHitchSeconds -gt 0) {
            $validationArguments += @('--expected-frame-hitch-seconds', $FrameHitchSeconds.ToString([Globalization.CultureInfo]::InvariantCulture), '--baseline-frame-delta-seconds', $FrameDeltaSeconds.ToString([Globalization.CultureInfo]::InvariantCulture))
        }
        python @validationArguments
        $validationExitCode = $LASTEXITCODE
        $runner.validation_exit_code = $validationExitCode
        $effectsPath = Join-Path $candidate "render_effects.json"
        if (Test-Path -LiteralPath $effectsPath -PathType Leaf) {
            $effects = Get-Content -Raw -LiteralPath $effectsPath | ConvertFrom-Json
            $runner.render_effects_passed = $effects.passed -eq $true
        }
        $decalPath = Join-Path $candidate "decal_render.json"
        if (Test-Path -LiteralPath $decalPath -PathType Leaf) {
            $decal = Get-Content -Raw -LiteralPath $decalPath | ConvertFrom-Json
            $runner.decal_renderer_passed = $decal.receiver_ready -eq $true -and @($decal.errors).Count -eq 0 -and ($Case -ne 'footprint' -or $decal.decal_ready -eq $true)
            if ($Case -eq 'footprint') {
                python tools/validate_decal_evidence.py --report $decalPath
                $runner.decal_renderer_passed = $runner.decal_renderer_passed -and $LASTEXITCODE -eq 0
            }
        }
        $runner | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $runnerPath -Encoding utf8
        if ($manualExitCode -ne 0) {
            throw "manual_playground exited with code $manualExitCode; visual diagnosis was written to $reportPath."
        }
        if ($validationExitCode -ne 0) {
            throw "Animation visual evidence validation failed."
        }
        if (-not $runner.render_effects_passed) {
            throw "SSAO/TAA component and executed pass validation failed or is missing."
        }
        if (-not $runner.decal_renderer_passed) { throw "Decal receiver or footprint GPU pipeline validation failed or is missing." }
    } finally {
        Pop-Location
    }
} finally {
    foreach ($name in $variables) {
        if ($null -eq $previous[$name]) {
            Remove-Item -LiteralPath ("Env:" + $name) -ErrorAction SilentlyContinue
        } else {
            Set-Item -LiteralPath ("Env:" + $name) -Value $previous[$name]
        }
    }
}

Write-Output ("Animation visual " + $(if ($FunctionalOnly) { "functional " } else { "" }) + "validation passed: " + $candidate)
