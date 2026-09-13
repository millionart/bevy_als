[CmdletBinding()]
param(
    [ValidateSet("startup", "base", "grounded", "standing_direction", "standing_response", "standing_stop", "standing_turn", "standing_dynamic", "crouching", "crouching_actions", "locomotion", "control_rig", "default_overlay", "head", "state_machine", "transition_notify", "foot_ik", "sprint", "ragdoll", "get_up", "camera", "stairs", "interface", "notify")]
    [string]$Case = "state_machine",
    [string]$OutputDirectory,
    [int]$CaptureCount = 0,
    [double]$CaptureIntervalSeconds = 1.5,
    [double]$MaxFrameP95Ms = 25.0,
    [double]$MaxFrameMs = 75.0,
    [double]$MaxEvalP95Ms = 4.0,
    [int]$MaxPhysicsStepsPerRender = 3,
    [switch]$FunctionalOnly
)

if ($CaptureCount -lt 0 -or $CaptureIntervalSeconds -le 0 -or $MaxFrameP95Ms -le 0 -or $MaxFrameMs -le 0 -or $MaxEvalP95Ms -le 0 -or $MaxPhysicsStepsPerRender -lt 1) {
    throw "CaptureCount must be nonnegative and other validation arguments must be positive."
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
            manual_exit_code = $manualExitCode
            report_path = $reportPath
            report_exists = Test-Path -LiteralPath $reportPath -PathType Leaf
            validation_exit_code = $null
            validation_scope = if ($FunctionalOnly) { "functional" } else { "full" }
        }
        if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
            $runner | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $runnerPath -Encoding utf8
            throw "manual_playground exited with code $manualExitCode and did not write $reportPath."
        }
        $validationArguments = @('tools/validate_animation_visual_evidence.py', '--report', $reportPath, '--min-screenshots', $CaptureCount, '--check')
        if ($FunctionalOnly) { $validationArguments += '--functional-only' }
        python @validationArguments
        $validationExitCode = $LASTEXITCODE
        $runner.validation_exit_code = $validationExitCode
        $runner | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $runnerPath -Encoding utf8
        if ($manualExitCode -ne 0) {
            throw "manual_playground exited with code $manualExitCode; visual diagnosis was written to $reportPath."
        }
        if ($validationExitCode -ne 0) {
            throw "Animation visual evidence validation failed."
        }
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
