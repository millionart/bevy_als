import argparse
import copy
import json
import math
import tempfile
from pathlib import Path

from PIL import Image, ImageStat


FORMAT = "als_animation_visual_evidence_v4"
MOTION_EPSILON = 1e-5
MOTION_BONES = (
    "thigh_l",
    "calf_l",
    "foot_l",
    "thigh_r",
    "calf_r",
    "foot_r",
)
STATE_MACHINE_DIRECTION_CASES = {
    "walk_forward",
    "run_backward",
    "run_right_from_backward",
    "run_left_from_right_backward",
    "run_backward_from_left_forward",
    "run_right_from_left_backward",
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
    root_pose = sample.get("root_pose")
    if not isinstance(root_pose, dict):
        issues.append("pose sample root pose is invalid")
    else:
        if not isinstance(root_pose.get("output_present"), bool):
            issues.append("pose sample root pose output flag is invalid")
        for key in ("bone_count", "active_channel_count"):
            value = root_pose.get(key)
            if not isinstance(value, int) or value < 0:
                issues.append(f"pose sample root pose metric is invalid: {key}")
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


def local_motion_pose_issues(pose, label):
    issues = []
    if not isinstance(pose, dict):
        return [f"motion window canonical pose is invalid: {label}"]
    missing = sorted(set(MOTION_BONES) - set(pose))
    if missing:
        issues.append(f"motion window canonical pose is missing bones: {label}: " + ", ".join(missing))
    for name in MOTION_BONES:
        value = pose.get(name)
        if not isinstance(value, dict):
            continue
        translation = value.get("translation")
        rotation = value.get("rotation")
        if not isinstance(translation, list) or len(translation) != 3 or not all(finite(item) for item in translation):
            issues.append(f"motion window canonical translation is invalid: {label}.{name}")
        if not isinstance(rotation, list) or len(rotation) != 4 or not all(finite(item) for item in rotation):
            issues.append(f"motion window canonical rotation is invalid: {label}.{name}")
        elif sum(item * item for item in rotation) < 0.25:
            issues.append(f"motion window canonical rotation is degenerate: {label}.{name}")
    return issues


def quaternion_distance(left, right):
    if not isinstance(left, list) or not isinstance(right, list) or len(left) != 4 or len(right) != 4:
        return None
    if not all(finite(value) for value in left + right):
        return None
    left_length = math.sqrt(sum(value * value for value in left))
    right_length = math.sqrt(sum(value * value for value in right))
    if left_length <= MOTION_EPSILON or right_length <= MOTION_EPSILON:
        return None
    dot = abs(sum(a * b for a, b in zip(left, right)) / (left_length * right_length))
    return 2.0 * math.acos(min(1.0, max(0.0, dot)))


def canonical_rotation_distance(left, right):
    maximum = 0.0
    compared = 0
    for name in MOTION_BONES:
        left_rotation = left.get(name, {}).get("rotation") if isinstance(left, dict) else None
        right_rotation = right.get(name, {}).get("rotation") if isinstance(right, dict) else None
        distance = quaternion_distance(left_rotation, right_rotation)
        if distance is None:
            continue
        maximum = max(maximum, distance)
        compared += 1
    return maximum if compared == len(MOTION_BONES) else None


def direction_signature_issues(windows):
    issues = []
    groups = {}
    for window in windows:
        if not isinstance(window, dict):
            continue
        group = window.get("direction_signature_group")
        if group in (None, ""):
            continue
        label = window.get("label")
        if not isinstance(group, str) or not group or not isinstance(label, str) or not label:
            issues.append("direction signature group is invalid")
            continue
        source_state = window.get("source_state")
        expected_direction = window.get("expected_direction")
        observed_directions = window.get("observed_directions")
        expected_runtime_state = window.get("expected_runtime_state")
        observed_runtime_states = window.get("observed_runtime_states")
        source_sync_group = window.get("source_sync_group")
        sync_phase_global = window.get("sync_phase_global")
        canonical_phase = window.get("canonical_sync_phase")
        maximum_phase_distance = window.get("maximum_sync_phase_distance")
        canonical_phase_distance = window.get("canonical_sync_phase_distance")
        required_members = window.get("required_direction_signature_members")
        minimum_rotation = window.get("minimum_direction_signature_rotation_radians")
        if not isinstance(source_state, str) or not source_state:
            issues.append(f"direction signature source state is missing: {label}")
        if not isinstance(expected_direction, str) or not expected_direction:
            issues.append(f"direction signature expected direction is missing: {label}")
        if not isinstance(observed_directions, list) or observed_directions != [expected_direction]:
            issues.append(f"direction signature observed direction does not match: {label}")
        if not isinstance(expected_runtime_state, int) or expected_runtime_state < 0:
            issues.append(f"direction signature expected runtime state is invalid: {label}")
        if not isinstance(observed_runtime_states, list) or observed_runtime_states != [expected_runtime_state]:
            issues.append(f"direction signature observed runtime state does not match: {label}")
        if not isinstance(source_sync_group, str) or not source_sync_group:
            issues.append(f"direction signature source sync group is missing: {label}")
        if not isinstance(sync_phase_global, str) or not sync_phase_global:
            issues.append(f"direction signature sync phase input is missing: {label}")
        if not finite(canonical_phase) or not 0.0 <= canonical_phase < 1.0:
            issues.append(f"direction signature canonical phase is invalid: {label}")
        if not finite(maximum_phase_distance) or maximum_phase_distance <= MOTION_EPSILON:
            issues.append(f"direction signature phase tolerance is invalid: {label}")
        if not finite(canonical_phase_distance) or canonical_phase_distance < 0.0:
            issues.append(f"direction signature canonical sample is missing: {label}")
        elif finite(maximum_phase_distance) and canonical_phase_distance > maximum_phase_distance:
            issues.append(f"direction signature canonical sample is outside tolerance: {label}")
        if not isinstance(required_members, int) or required_members < 2:
            issues.append(f"direction signature member count is invalid: {label}")
        if not finite(minimum_rotation) or minimum_rotation <= MOTION_EPSILON:
            issues.append(f"direction signature rotation floor is invalid: {label}")
        issues.extend(local_motion_pose_issues(window.get("canonical_pre_rig_pose"), label))
        groups.setdefault(group, []).append(window)
    for group, members in groups.items():
        expected_counts = {member.get("required_direction_signature_members") for member in members}
        expected_count = next(iter(expected_counts)) if len(expected_counts) == 1 else None
        if expected_count is None:
            issues.append(f"direction signature group has inconsistent member counts: {group}")
            continue
        if len(members) != expected_count:
            issues.append(
                f"direction signature group has incomplete source coverage: {group}: {len(members)}/{expected_count}"
            )
        labels = [member.get("label") for member in members]
        if len(set(labels)) != len(labels):
            issues.append(f"direction signature group has duplicate labels: {group}")
        floors = {member.get("minimum_direction_signature_rotation_radians") for member in members}
        if len(floors) != 1:
            issues.append(f"direction signature group has inconsistent rotation floors: {group}")
            continue
        floor = next(iter(floors))
        if not finite(floor) or floor <= MOTION_EPSILON:
            continue
        for index, left in enumerate(members):
            for right in members[index + 1 :]:
                distance = canonical_rotation_distance(
                    left.get("canonical_pre_rig_pose"), right.get("canonical_pre_rig_pose")
                )
                if distance is None:
                    continue
                if distance < floor:
                    issues.append(
                        "direction signature is not distinct: "
                        + f"{group}: {left.get('label')} vs {right.get('label')}"
                    )
    return issues


def motion_window_issues(report, screenshots, pose_samples):
    issues = []
    windows = report.get("motion_windows")
    if not isinstance(windows, list):
        return ["motion windows are invalid"]
    for window in windows:
        if not isinstance(window, dict):
            issues.append("motion window is invalid")
            continue
        label = window.get("label")
        source_node = window.get("source_node")
        sample_count = window.get("sample_count")
        requires_pre_rig_motion = window.get("requires_pre_rig_motion")
        requires_final_pose_motion = window.get("requires_final_pose_motion")
        requires_non_reference_root_pose = window.get("requires_non_reference_root_pose")
        pre_translation_floor = window.get("minimum_pre_rig_translation_span")
        pre_rotation_floor = window.get("minimum_pre_rig_rotation_span_radians")
        final_translation_floor = window.get("minimum_final_translation_span")
        final_rotation_floor = window.get("minimum_final_rotation_span_radians")
        minimum_observation_seconds = window.get("minimum_observation_seconds")
        observation_seconds = window.get("observation_seconds")
        pre_translation_delta = window.get("max_pre_rig_translation_span")
        pre_rotation_delta = window.get("max_pre_rig_rotation_span_radians")
        final_translation_delta = window.get("max_final_translation_span")
        final_rotation_delta = window.get("max_final_rotation_span_radians")
        if not isinstance(label, str) or not label:
            issues.append("motion window label is invalid")
            continue
        if not isinstance(source_node, str) or not source_node:
            issues.append(f"motion window source is missing: {label}")
        if not isinstance(sample_count, int) or sample_count < 2:
            issues.append(f"motion window has insufficient pose samples: {label}")
        for name, value in (
            ("requires_pre_rig_motion", requires_pre_rig_motion),
            ("requires_final_pose_motion", requires_final_pose_motion),
            ("requires_non_reference_root_pose", requires_non_reference_root_pose),
        ):
            if not isinstance(value, bool):
                issues.append(f"motion window requirement is invalid: {label}.{name}")
        for name, value in (
            ("pre-rig translation floor", pre_translation_floor),
            ("pre-rig rotation floor", pre_rotation_floor),
            ("final translation floor", final_translation_floor),
            ("final rotation floor", final_rotation_floor),
            ("observation floor", minimum_observation_seconds),
            ("observation span", observation_seconds),
            ("pre-rig translation delta", pre_translation_delta),
            ("pre-rig rotation delta", pre_rotation_delta),
            ("final translation delta", final_translation_delta),
            ("final rotation delta", final_rotation_delta),
        ):
            if not finite(value) or value < 0.0:
                issues.append(f"motion window {name} is invalid: {label}")
        if requires_pre_rig_motion is True:
            if finite(pre_translation_delta) and finite(pre_rotation_delta) and (
                pre_translation_delta <= MOTION_EPSILON and pre_rotation_delta <= MOTION_EPSILON
            ):
                issues.append(f"motion window is static before rig application: {label}")
            if (
                finite(pre_translation_floor)
                and finite(pre_rotation_floor)
                and finite(pre_translation_delta)
                and finite(pre_rotation_delta)
                and (pre_translation_floor > MOTION_EPSILON or pre_rotation_floor > MOTION_EPSILON)
                and (pre_translation_floor <= MOTION_EPSILON or pre_translation_delta < pre_translation_floor)
                and (pre_rotation_floor <= MOTION_EPSILON or pre_rotation_delta < pre_rotation_floor)
            ):
                issues.append(f"motion window is below its source-derived pre-rig floor: {label}")
        if requires_final_pose_motion is True:
            if finite(final_translation_delta) and finite(final_rotation_delta) and (
                final_translation_delta <= MOTION_EPSILON and final_rotation_delta <= MOTION_EPSILON
            ):
                issues.append(f"motion window is static after rig application: {label}")
            if (
                finite(final_translation_floor)
                and finite(final_rotation_floor)
                and finite(final_translation_delta)
                and finite(final_rotation_delta)
                and (final_translation_floor > MOTION_EPSILON or final_rotation_floor > MOTION_EPSILON)
                and (final_translation_floor <= MOTION_EPSILON or final_translation_delta < final_translation_floor)
                and (final_rotation_floor <= MOTION_EPSILON or final_rotation_delta < final_rotation_floor)
            ):
                issues.append(f"motion window is below its source-derived final floor: {label}")
        if (
            finite(minimum_observation_seconds)
            and finite(observation_seconds)
            and observation_seconds < minimum_observation_seconds
        ):
            issues.append(f"motion window observation is too short: {label}")
        root_sample_count = window.get("root_pose_sample_count")
        root_active_samples = window.get("root_pose_active_samples")
        root_reference_samples = window.get("root_pose_reference_samples")
        root_max_channels = window.get("root_pose_max_active_channels")
        for name, value in (
            ("sample count", root_sample_count),
            ("active samples", root_active_samples),
            ("reference samples", root_reference_samples),
            ("maximum channels", root_max_channels),
        ):
            if not isinstance(value, int) or value < 0:
                issues.append(f"motion window root pose {name} is invalid: {label}")
        if requires_non_reference_root_pose is True and (
            not isinstance(root_sample_count, int)
            or not isinstance(root_active_samples, int)
            or not isinstance(root_reference_samples, int)
            or not isinstance(root_max_channels, int)
            or root_sample_count < 2
            or root_sample_count != sample_count
            or root_active_samples != root_sample_count
            or root_reference_samples != 0
            or root_max_channels <= 0
        ):
            issues.append(f"motion window resolved to a reference root pose: {label}")
        expected_direction = window.get("expected_direction")
        observed_directions = window.get("observed_directions")
        if not isinstance(expected_direction, str):
            issues.append(f"motion window expected direction is invalid: {label}")
        if not isinstance(observed_directions, list) or not all(
            isinstance(value, str) and value for value in observed_directions
        ):
            issues.append(f"motion window observed directions are invalid: {label}")
        elif expected_direction and observed_directions != [expected_direction]:
            issues.append(f"motion window selected the wrong direction: {label}")
        expected_runtime_state = window.get("expected_runtime_state")
        observed_runtime_states = window.get("observed_runtime_states")
        if expected_runtime_state is not None and (
            not isinstance(expected_runtime_state, int) or expected_runtime_state < 0
        ):
            issues.append(f"motion window expected runtime state is invalid: {label}")
        if not isinstance(observed_runtime_states, list) or not all(
            isinstance(value, int) and value >= 0 for value in observed_runtime_states
        ):
            issues.append(f"motion window observed runtime states are invalid: {label}")
        elif expected_runtime_state is not None and observed_runtime_states != [expected_runtime_state]:
            issues.append(f"motion window selected the wrong runtime state: {label}")
        matching_screenshots = [
            value
            for value in screenshots
            if isinstance(value, dict)
            and value.get("kind") == "direction"
            and value.get("direction_case") == label
        ]
        if not matching_screenshots:
            issues.append(f"motion window screenshot is missing: {label}")
        matching_samples = [
            value
            for value in pose_samples
            if isinstance(value, dict) and value.get("direction_case") == label
        ]
        if len(matching_samples) < 2:
            issues.append(f"motion window pose samples are missing: {label}")
    issues.extend(direction_signature_issues(windows))
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
    issues.extend(motion_window_issues(report, screenshots, pose_samples))
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
                "root_pose": {"output_present": True, "bone_count": 2, "active_channel_count": 6},
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
        "motion_windows": [],
        "faults": [],
        "graph_errors": [],
        "thresholds": {"max_frame_p95_ms": 25.0, "max_frame_ms": 75.0, "max_eval_p95_ms": 4.0, "max_physics_steps_per_render": 3.0},
    }


def fixture_canonical_pose(rotation_radians=0.0):
    half_angle = rotation_radians * 0.5
    return {
        name: {
            "translation": [0.0, 0.0, 0.0],
            "rotation": [math.sin(half_angle), 0.0, 0.0, math.cos(half_angle)],
        }
        for name in MOTION_BONES
    }


def fixture_motion_window(label="dynamic", rotation_radians=0.0, group=None):
    is_group_member = group is not None
    group_state = {
        "forward": 0,
        "backward": 1,
        "right_forward": 2,
        "right_backward": 3,
        "left_forward": 4,
        "left_backward": 5,
    }.get(label, 0)
    group_direction = f"Direction {label}"
    return {
        "label": label,
        "source_node": "Source/Sequence Player",
        "source_state": f"State {label}" if is_group_member else "",
        "expected_direction": group_direction if is_group_member else "",
        "observed_directions": [group_direction] if is_group_member else [],
        "expected_runtime_state": group_state if is_group_member else None,
        "observed_runtime_states": [group_state] if is_group_member else [],
        "sample_count": 2,
        "requires_pre_rig_motion": True,
        "requires_final_pose_motion": True,
        "requires_non_reference_root_pose": True,
        "minimum_pre_rig_translation_span": 0.0,
        "minimum_pre_rig_rotation_span_radians": 0.0,
        "minimum_final_translation_span": 0.0,
        "minimum_final_rotation_span_radians": 0.0,
        "minimum_observation_seconds": 0.0,
        "observation_seconds": 1.0,
        "max_pre_rig_translation_span": 0.01,
        "max_pre_rig_rotation_span_radians": 0.01,
        "max_final_translation_span": 0.01,
        "max_final_rotation_span_radians": 0.01,
        "root_pose_sample_count": 2,
        "root_pose_active_samples": 2,
        "root_pose_reference_samples": 0,
        "root_pose_max_active_channels": 6,
        "source_sync_group": "Movement" if is_group_member else "",
        "sync_phase_global": "marker_phase" if is_group_member else "",
        "canonical_sync_phase": 0.5 if is_group_member else None,
        "maximum_sync_phase_distance": 0.1 if is_group_member else 0.0,
        "canonical_sync_phase_distance": 0.01 if is_group_member else None,
        "direction_signature_group": group or "",
        "required_direction_signature_members": 6 if is_group_member else 0,
        "minimum_direction_signature_rotation_radians": 0.01 if is_group_member else 0.0,
        "canonical_pre_rig_pose": fixture_canonical_pose(rotation_radians) if is_group_member else {},
    }


def add_dynamic_window_samples(report, labels):
    for label in labels:
        sample = copy.deepcopy(report["pose_samples"][0])
        sample["direction_case"] = label
        report["pose_samples"].append(sample)
        report["pose_samples"].append(copy.deepcopy(sample))
        report["screenshots"].append(
            {
                "path": report["screenshots"][0]["path"],
                "elapsed_secs": 1.0,
                "kind": "direction",
                "direction_case": label,
                "faults": [],
            }
        )


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
        report["graph_errors"] = ["ClipMissing"]
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
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["max_pre_rig_translation_span"] = 0.0
        report["motion_windows"][0]["max_pre_rig_rotation_span_radians"] = 0.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["screenshots"] = [
            value
            for value in report["screenshots"]
            if value.get("direction_case") != "dynamic"
        ]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["minimum_observation_seconds"] = 2.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["minimum_pre_rig_rotation_span_radians"] = 0.1
        report["motion_windows"][0]["max_pre_rig_translation_span"] = 0.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["root_pose_active_samples"] = 0
        report["motion_windows"][0]["root_pose_reference_samples"] = 2
        report["motion_windows"][0]["root_pose_max_active_channels"] = 0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["max_final_translation_span"] = 0.0
        report["motion_windows"][0]["max_final_rotation_span_radians"] = 0.0
        assert not audit(report, 2)["pass"]
        labels = [
            "forward",
            "backward",
            "right_forward",
            "right_backward",
            "left_backward",
            "left_forward",
        ]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, labels)
        report["motion_windows"] = [
            fixture_motion_window(label, rotation_radians=index * 0.1, group="movement")
            for index, label in enumerate(labels)
        ]
        assert audit(report, 2)["pass"]
        report["motion_windows"] = [
            fixture_motion_window(label, rotation_radians=0.0, group="movement")
            for label in labels
        ]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, labels)
        report["motion_windows"] = [
            fixture_motion_window(label, rotation_radians=index * 0.1, group="movement")
            for index, label in enumerate(labels)
        ]
        report["motion_windows"][0]["observed_runtime_states"] = [5]
        assert not audit(report, 2)["pass"]
    return {
        "runtime_report_and_png_checked": "PASS",
        "fault_rejected": "PASS",
        "startup_graph_error_rejected": "PASS",
        "foot_lock_teleport_evidence_required": "PASS",
        "foot_ik_teleport_case_required": "PASS",
        "runtime_phase_evidence_required": "PASS",
        "physics_step_stage_required": "PASS",
        "state_machine_direction_evidence_required": "PASS",
        "sprint_source_curve_relay_required": "PASS",
        "shared_pre_rig_motion_window_required": "PASS",
        "static_dynamic_window_rejected": "PASS",
        "source_motion_floor_required": "PASS",
        "motion_window_span_required": "PASS",
        "reference_root_pose_rejected": "PASS",
        "static_final_pose_rejected": "PASS",
        "collapsed_direction_signatures_rejected": "PASS",
        "wrong_direction_state_rejected": "PASS",
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
