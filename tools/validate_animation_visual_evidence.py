import argparse
import json
import math
import tempfile
from pathlib import Path

from PIL import Image, ImageStat


FORMAT = "als_animation_visual_evidence_v2"
STATE_MACHINE_DIRECTION_CASES = {
    "walk_forward",
    "run_backward",
    "run_forward_right",
    "run_backward_right",
    "run_backward_left",
    "run_forward_left",
}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def finite(value):
    return isinstance(value, (int, float)) and math.isfinite(value)


def png_report(path):
    with Image.open(path) as image:
        image.verify()
    with Image.open(path) as image:
        rgb = image.convert("RGB")
        width, height = rgb.size
        stats = ImageStat.Stat(rgb)
        variance = max(stats.var)
        extrema = stats.extrema
    return {
        "path": str(path),
        "width": width,
        "height": height,
        "variance": variance,
        "extrema": extrema,
    }


def issues_for_sample(sample, required_bones):
    issues = []
    if not isinstance(sample, dict):
        return ["pose sample is invalid"]
    if not finite(sample.get("elapsed_secs")) or not finite(sample.get("frame_ms")):
        issues.append("pose sample time is invalid")
    bones = sample.get("bones")
    if not isinstance(bones, dict):
        return issues + ["pose sample bones are invalid"]
    missing = sorted(set(required_bones) - set(bones))
    if missing:
        issues.append("pose sample is missing bones: " + ", ".join(missing))
    for name, bone in bones.items():
        if not isinstance(bone, dict):
            issues.append(f"bone is invalid: {name}")
            continue
        for key, count in (("translation", 3), ("rotation", 4), ("scale", 3), ("local_scale", 3)):
            values = bone.get(key)
            if not isinstance(values, list) or len(values) != count or not all(
                finite(value) for value in values
            ):
                issues.append(f"bone {key} is invalid: {name}")
        scale = bone.get("scale")
        if isinstance(scale, list) and len(scale) == 3 and all(finite(value) for value in scale):
            if min(abs(value) for value in scale) < 0.01 or max(abs(value) for value in scale) > 10.0:
                issues.append(f"bone scale is out of range: {name}")
        local_scale = bone.get("local_scale")
        if isinstance(local_scale, list) and len(local_scale) == 3 and all(finite(value) for value in local_scale):
            if min(abs(value) for value in local_scale) < 0.01 or max(abs(value) for value in local_scale) > 10.0:
                issues.append(f"bone local scale is out of range: {name}")
    segments = sample.get("segments")
    if not isinstance(segments, dict) or not segments:
        issues.append("pose sample segments are invalid")
    elif any(not finite(value) or value <= 0.0 for value in segments.values()):
        issues.append("pose sample segments contain invalid values")
    direction_case = sample.get("direction_case")
    if direction_case is not None and (not isinstance(direction_case, str) or not direction_case):
        issues.append("pose sample direction case is invalid")
    phase = sample.get("phase")
    if phase is not None and (not isinstance(phase, str) or not phase):
        issues.append("pose sample phase is invalid")
    phase_elapsed_secs = sample.get("phase_elapsed_secs")
    if phase_elapsed_secs is not None and (not finite(phase_elapsed_secs) or phase_elapsed_secs < 0.0):
        issues.append("pose sample phase elapsed time is invalid")
    for key in ("locomotion", "gait", "stance", "rotation_mode"):
        value = sample.get(key)
        if value is not None and (not isinstance(value, str) or not value):
            issues.append(f"pose sample runtime state is invalid: {key}")
    foot_ik = sample.get("foot_ik")
    if foot_ik is not None:
        if not isinstance(foot_ik, dict):
            issues.append("pose sample foot ik is invalid")
        else:
            if foot_ik.get("source_curves_available") is not True:
                issues.append("foot ik source curve context is unavailable")
            for key in ("pose_moving", "foot_left_ik_curve", "foot_right_ik_curve"):
                if not finite(foot_ik.get(key)):
                    issues.append(f"foot ik source curve is invalid: {key}")
            if not finite(foot_ik.get("standing_pose_moving_global")):
                issues.append("foot ik standing pose moving global is invalid")
            lineage = foot_ik.get("pose_moving_lineage")
            if not isinstance(lineage, list) or not lineage:
                issues.append("foot ik pose moving lineage is invalid")
            for name in ("left", "right"):
                side = foot_ik.get(name)
                if not isinstance(side, dict):
                    issues.append(f"foot ik side is invalid: {name}")
                    continue
                for key in (
                    "trace_target_world",
                    "solver_reference_world",
                    "target_world",
                    "resolved_target_world",
                    "pole_location_world",
                    "effector_world",
                    "hip_world",
                    "knee_world",
                    "ankle_world",
                ):
                    values = side.get(key)
                    if not isinstance(values, list) or len(values) != 3 or not all(finite(value) for value in values):
                        issues.append(f"foot ik vector is invalid: {name}.{key}")
                for key in ("raw_offset", "applied_weight", "pole_sign", "knee_error", "effector_error"):
                    if not finite(side.get(key)):
                        issues.append(f"foot ik scalar is invalid: {name}.{key}")
            for key in ("velocity_blend_forward_amount", "velocity_blend_backward_amount"):
                if not finite(foot_ik.get(key)):
                    issues.append(f"foot ik velocity blend is invalid: {key}")
            teleport = foot_ik.get("teleport")
            if not isinstance(teleport, dict):
                issues.append("foot lock teleport evidence is invalid")
            else:
                for key in ("detected_count", "left_rebase_count", "right_rebase_count"):
                    value = teleport.get(key)
                    if not isinstance(value, int) or value < 0:
                        issues.append(f"foot lock teleport count is invalid: {key}")
                for key in ("last_translation_delta_m", "remaining_seconds"):
                    if not finite(teleport.get(key)):
                        issues.append(f"foot lock teleport scalar is invalid: {key}")
    return issues


def audit(report, min_screenshots):
    issues = []
    images = []
    if not isinstance(report, dict) or report.get("format") != FORMAT:
        return {"pass": False, "issues": ["visual report format is invalid"], "images": []}
    if report.get("ready") is not True:
        issues.append("visual report is not ready")
    if report.get("pass") is not True:
        issues.append("runtime visual verdict is not pass")
    for key in ("faults", "graph_errors", "missing_bones"):
        values = report.get(key)
        if not isinstance(values, list):
            issues.append(f"{key} is invalid")
        elif values:
            issues.append(f"{key} is non-empty")
    required_bones = report.get("required_bones")
    if not isinstance(required_bones, list) or not required_bones or not all(
        isinstance(name, str) and name for name in required_bones
    ):
        issues.append("required_bones is invalid")
        required_bones = []
    pose_samples = report.get("pose_samples")
    if not isinstance(pose_samples, list) or not pose_samples:
        issues.append("pose_samples is invalid")
        pose_samples = []
    else:
        for sample in pose_samples:
            issues.extend(issues_for_sample(sample, required_bones))
    thresholds = report.get("thresholds")
    frame_time = report.get("frame_time")
    eval_time = report.get("animation_eval_time")
    physics_steps = report.get("physics_steps_per_render")
    stage_time = report.get("stage_time")
    if (
        not isinstance(thresholds, dict)
        or not isinstance(frame_time, dict)
        or not isinstance(eval_time, dict)
        or not isinstance(physics_steps, dict)
    ):
        issues.append("performance payload is invalid")
    else:
        checks = (
            (frame_time, "max_frame_p95_ms", "p95_ms"),
            (frame_time, "max_frame_ms", "max_ms"),
            (eval_time, "max_eval_p95_ms", "p95_ms"),
        )
        for values, threshold_key, value_key in checks:
            threshold = thresholds.get(threshold_key)
            value = values.get(value_key)
            samples = values.get("samples")
            if not finite(threshold) or not finite(value) or not isinstance(samples, int) or samples <= 0:
                issues.append(f"performance value is invalid: {threshold_key}")
            elif value > threshold:
                issues.append(f"performance budget exceeded: {threshold_key}")
        physics_threshold = thresholds.get("max_physics_steps_per_render")
        physics_value = physics_steps.get("max")
        physics_samples = physics_steps.get("samples")
        if (
            not finite(physics_threshold)
            or not finite(physics_value)
            or not isinstance(physics_samples, int)
            or physics_samples <= 0
        ):
            issues.append("physics step value is invalid")
        elif physics_value > physics_threshold:
            issues.append("physics step budget exceeded")
    if not isinstance(stage_time, dict):
        issues.append("stage timing payload is invalid")
    else:
        for stage_name in ("physics", "physics_step"):
            values = stage_time.get(stage_name)
            if not isinstance(values, dict):
                issues.append(f"stage timing is missing: {stage_name}")
                continue
            samples = values.get("samples")
            if not isinstance(samples, int) or samples <= 0:
                issues.append(f"stage timing sample count is invalid: {stage_name}")
                continue
            for value_name in ("p50_ms", "p95_ms", "max_ms"):
                if not finite(values.get(value_name)):
                    issues.append(f"stage timing value is invalid: {stage_name}.{value_name}")
    screenshots = report.get("screenshots")
    if not isinstance(screenshots, list) or len(screenshots) < min_screenshots:
        issues.append("insufficient screenshots")
        screenshots = []
    else:
        for value in screenshots:
            path_value = value if isinstance(value, str) else value.get("path") if isinstance(value, dict) else None
            if not isinstance(path_value, str) or not path_value:
                issues.append("screenshot record is invalid")
                continue
            phase = value.get("phase") if isinstance(value, dict) else None
            if phase is not None and (not isinstance(phase, str) or not phase):
                issues.append("screenshot phase is invalid")
            phase_elapsed_secs = value.get("phase_elapsed_secs") if isinstance(value, dict) else None
            if phase_elapsed_secs is not None and (not finite(phase_elapsed_secs) or phase_elapsed_secs < 0.0):
                issues.append("screenshot phase elapsed time is invalid")
            if isinstance(value, dict):
                for key in ("locomotion", "gait", "stance", "rotation_mode"):
                    runtime_value = value.get(key)
                    if runtime_value is not None and (not isinstance(runtime_value, str) or not runtime_value):
                        issues.append(f"screenshot runtime state is invalid: {key}")
            path = Path(path_value)
            if not path.is_file():
                issues.append(f"screenshot is missing: {path}")
                continue
            try:
                image = png_report(path)
            except Exception as error:
                issues.append(f"screenshot is invalid: {path}: {error}")
                continue
            images.append(image)
            if image["width"] < 256 or image["height"] < 256:
                issues.append(f"screenshot dimensions are too small: {path}")
            if image["variance"] < 2.0:
                issues.append(f"screenshot has no visible scene content: {path}")
    if report.get("case") == "state_machine":
        direction_screenshots = {
            value.get("direction_case")
            for value in screenshots
            if isinstance(value, dict)
            and value.get("kind") == "direction"
            and isinstance(value.get("direction_case"), str)
        }
        direction_samples = {
            sample.get("direction_case")
            for sample in pose_samples
            if isinstance(sample, dict)
            and isinstance(sample.get("direction_case"), str)
            and isinstance(sample.get("foot_ik"), dict)
        }
        missing_screenshots = sorted(STATE_MACHINE_DIRECTION_CASES - direction_screenshots)
        missing_samples = sorted(STATE_MACHINE_DIRECTION_CASES - direction_samples)
        if missing_screenshots:
            issues.append("state machine direction screenshots are missing: " + ", ".join(missing_screenshots))
        if missing_samples:
            issues.append("state machine direction foot ik samples are missing: " + ", ".join(missing_samples))
    if report.get("case") == "foot_ik":
        sprint_samples = [
            sample.get("foot_ik", {})
            for sample in pose_samples
            if isinstance(sample, dict)
            and sample.get("direction_case") == "sprint_allowed_forward"
            and isinstance(sample.get("gait"), str)
            and "Sprinting" in sample.get("gait")
            and isinstance(sample.get("foot_ik"), dict)
        ]
        if not sprint_samples:
            issues.append("foot ik sprint source curve sample is missing")
        elif not any(
            sample.get("source_curves_available") is True
            and sample.get("pose_moving", 0.0) >= 0.999
            and sample.get("standing_pose_moving_global", 0.0) >= 0.999
            and sample.get("foot_left_ik_curve", 0.0) >= 0.999
            and sample.get("foot_right_ik_curve", 0.0) >= 0.999
            for sample in sprint_samples
        ):
            issues.append("foot ik sprint source curve relay is missing")
        teleport_screenshots = [
            value
            for value in screenshots
            if isinstance(value, dict) and value.get("kind") == "teleport"
        ]
        teleport_samples = [
            sample.get("foot_ik", {}).get("teleport", {})
            for sample in pose_samples
            if isinstance(sample, dict)
            and isinstance(sample.get("foot_ik"), dict)
            and isinstance(sample.get("foot_ik", {}).get("teleport"), dict)
            and sample.get("foot_ik", {}).get("teleport", {}).get("detected_count", 0) > 0
        ]
        if not teleport_screenshots:
            issues.append("foot ik teleport screenshot is missing")
        if not teleport_samples:
            issues.append("foot ik teleport pose sample is missing")
        elif not any(
            sample.get("left_rebase_count", 0) > 0 and sample.get("right_rebase_count", 0) > 0
            for sample in teleport_samples
        ):
            issues.append("foot ik teleport foot-lock rebase evidence is missing")
    if report.get("case") == "sprint":
        sprint_samples = [
            sample.get("foot_ik", {})
            for sample in pose_samples
            if isinstance(sample, dict)
            and sample.get("phase") == "Sprint"
            and isinstance(sample.get("gait"), str)
            and "Sprinting" in sample.get("gait")
            and isinstance(sample.get("foot_ik"), dict)
        ]
        if not sprint_samples:
            issues.append("sprint source curve sample is missing")
        elif not any(
            sample.get("source_curves_available") is True
            and sample.get("pose_moving", 0.0) >= 0.999
            and sample.get("standing_pose_moving_global", 0.0) >= 0.999
            and sample.get("foot_left_ik_curve", 0.0) >= 0.999
            and sample.get("foot_right_ik_curve", 0.0) >= 0.999
            and sample.get("left", {}).get("applied_weight", 0.0) >= 0.999
            and sample.get("right", {}).get("applied_weight", 0.0) >= 0.999
            for sample in sprint_samples
        ):
            issues.append("sprint source curve relay is missing")
    issues = list(dict.fromkeys(issues))
    return {"pass": not issues, "issues": issues, "images": images}


def fixture_report(paths):
    return {
        "format": FORMAT,
        "case": "fixture",
        "ready": True,
        "pass": True,
        "observed_bones": 2,
        "required_bones": ["pelvis", "head"],
        "missing_bones": [],
        "frame_time": {"samples": 4, "p50_ms": 10.0, "p95_ms": 12.0, "max_ms": 14.0},
        "animation_eval_time": {"samples": 4, "p50_ms": 1.0, "p95_ms": 2.0, "max_ms": 3.0},
        "physics_steps_per_render": {"samples": 4, "p50": 1.0, "p95": 2.0, "max": 2.0},
        "stage_time": {
            "physics": {"samples": 4, "p50_ms": 3.0, "p95_ms": 4.0, "max_ms": 5.0},
            "physics_step": {"samples": 4, "p50_ms": 2.0, "p95_ms": 3.0, "max_ms": 4.0},
        },
        "pose_samples": [
            {
                "elapsed_secs": 1.0,
                "frame_ms": 10.0,
                "bones": {
                    "pelvis": {"translation": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0], "local_scale": [1.0, 1.0, 1.0]},
                    "head": {"translation": [0.0, 1.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0], "local_scale": [1.0, 1.0, 1.0]},
                },
                "segments": {"pelvis->head": 1.0},
                "foot_ik": {
                    "source_curves_available": True,
                    "pose_moving": 1.0,
                    "foot_left_ik_curve": 1.0,
                    "foot_right_ik_curve": 1.0,
                    "standing_pose_moving_global": 1.0,
                    "pose_moving_lineage": [{"node": "AB_Als_Standing", "pose_moving": 1.0}],
                    "left": {
                        "valid": True,
                        "has_ground_hit": True,
                        "trace_target_world": [0.0, 0.0, 0.0],
                        "solver_reference_world": [0.0, 0.0, 0.0],
                        "target_world": [0.0, 0.0, 0.0],
                        "resolved_target_world": [0.0, 0.0, 0.0],
                        "pole_location_world": [0.0, 0.5, 0.2],
                        "effector_world": [0.0, 0.0, 0.0],
                        "hip_world": [0.0, 1.0, 0.0],
                        "knee_world": [0.0, 0.5, 0.0],
                        "ankle_world": [0.0, 0.0, 0.0],
                        "raw_offset": 0.0,
                        "applied_weight": 1.0,
                        "pole_sign": 1.0,
                        "knee_error": 0.0,
                        "effector_error": 0.0,
                    },
                    "right": {
                        "valid": True,
                        "has_ground_hit": True,
                        "trace_target_world": [0.0, 0.0, 0.0],
                        "solver_reference_world": [0.0, 0.0, 0.0],
                        "target_world": [0.0, 0.0, 0.0],
                        "resolved_target_world": [0.0, 0.0, 0.0],
                        "pole_location_world": [0.0, 0.5, -0.2],
                        "effector_world": [0.0, 0.0, 0.0],
                        "hip_world": [0.0, 1.0, 0.0],
                        "knee_world": [0.0, 0.5, 0.0],
                        "ankle_world": [0.0, 0.0, 0.0],
                        "raw_offset": 0.0,
                        "applied_weight": 1.0,
                        "pole_sign": 1.0,
                        "knee_error": 0.0,
                        "effector_error": 0.0,
                    },
                    "velocity_blend_forward_amount": 1.0,
                    "velocity_blend_backward_amount": 0.0,
                    "teleport": {
                        "detected_count": 1,
                        "left_rebase_count": 1,
                        "right_rebase_count": 1,
                        "last_translation_delta_m": 0.6,
                        "remaining_seconds": 0.1,
                    },
                },
            }
        ],
        "screenshots": [{"path": str(path), "elapsed_secs": 1.0, "kind": "fixture", "faults": []} for path in paths],
        "faults": [],
        "graph_errors": [],
        "thresholds": {"max_frame_p95_ms": 25.0, "max_frame_ms": 75.0, "max_eval_p95_ms": 4.0, "max_physics_steps_per_render": 3.0},
    }


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        paths = []
        for index in range(2):
            path = root / f"frame_{index}.png"
            image = Image.new("RGB", (320, 320), (16, 24, 32))
            for coordinate in range(32, 288):
                image.putpixel((coordinate, coordinate), (220, 160, 64))
            image.save(path)
            paths.append(path)
        report = fixture_report(paths)
        assert audit(report, 2)["pass"]
        report["pose_samples"][0]["foot_ik"]["teleport"].pop("detected_count")
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["pose_samples"][0]["phase"] = ""
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["pose_samples"][0]["phase_elapsed_secs"] = -1.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["pose_samples"][0]["gait"] = ""
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["faults"] = ["fault"]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["stage_time"].pop("physics_step")
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["case"] = "state_machine"
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["case"] = "foot_ik"
        report["pose_samples"][0]["direction_case"] = "sprint_allowed_forward"
        report["pose_samples"][0]["gait"] = "Sprinting"
        report["screenshots"][0]["kind"] = "teleport"
        assert audit(report, 2)["pass"]
        report["pose_samples"][0]["foot_ik"]["pose_moving"] = 0.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["case"] = "sprint"
        report["pose_samples"][0]["phase"] = "Sprint"
        report["pose_samples"][0]["gait"] = "Sprinting"
        assert audit(report, 2)["pass"]
        report["pose_samples"][0]["foot_ik"]["standing_pose_moving_global"] = 0.0
        assert not audit(report, 2)["pass"]
    return {
        "runtime_report_and_png_checked": "PASS",
        "fault_rejected": "PASS",
        "foot_lock_teleport_evidence_required": "PASS",
        "foot_ik_teleport_case_required": "PASS",
        "runtime_phase_evidence_required": "PASS",
        "physics_step_stage_required": "PASS",
        "state_machine_direction_evidence_required": "PASS",
        "sprint_source_curve_relay_required": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    parser.add_argument("--min-screenshots", type=int, default=6)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.report is None or not args.check or args.min_screenshots < 1:
        raise ValueError("report, --check, and a positive min-screenshots are required")
    if not args.report.is_file():
        raise ValueError("visual report is missing")
    result = audit(read_json(args.report), args.min_screenshots)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
