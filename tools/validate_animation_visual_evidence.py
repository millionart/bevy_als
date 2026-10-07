import argparse
import copy
import json
import math
import tempfile
from pathlib import Path

from PIL import Image, ImageStat


FORMAT = Path(__file__).with_name("animation_visual_evidence_format.txt").read_text(encoding="utf-8").strip()
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
    "run_left_forward_from_backward",
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
        if root_pose.get("output_present") is not True:
            issues.append("pose sample root pose is missing")
        elif root_pose.get("active_channel_count") == 0:
            issues.append("pose sample root pose is reference or A-pose")
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
        clip_time = window.get("canonical_clip_time")
        if clip_time is not None:
            if not finite(clip_time) or clip_time < 0.0 or canonical_phase is not None:
                issues.append(f"direction signature canonical clip time is invalid: {label}")
            maximum_phase_distance = window.get("maximum_clip_time_distance")
            canonical_phase_distance = window.get("canonical_clip_time_distance")
        else:
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


def source_pose_boundary_failure(boundary):
    label = boundary.get("label")
    sample_count = boundary.get("sample_count")
    available_samples = boundary.get("available_samples")
    unavailable_samples = boundary.get("unavailable_samples")
    translation_span = boundary.get("max_translation_span")
    rotation_span = boundary.get("max_rotation_span_radians")
    translation_floor = boundary.get("minimum_translation_span")
    rotation_floor = boundary.get("minimum_rotation_span_radians")
    if not isinstance(label, str) or not label:
        return None
    if not isinstance(sample_count, int) or sample_count < 2 or not isinstance(available_samples, int) or available_samples < 2:
        return f"{label}: unavailable"
    if not isinstance(unavailable_samples, int) or unavailable_samples > 0:
        return f"{label}: intermittently unavailable"
    if not finite(translation_span) or not finite(rotation_span):
        return f"{label}: unavailable"
    if translation_span <= MOTION_EPSILON and rotation_span <= MOTION_EPSILON:
        return f"{label}: static"
    if (
        finite(translation_floor)
        and finite(rotation_floor)
        and (translation_floor > MOTION_EPSILON or rotation_floor > MOTION_EPSILON)
        and (translation_floor <= MOTION_EPSILON or translation_span < translation_floor)
        and (rotation_floor <= MOTION_EPSILON or rotation_span < rotation_floor)
    ):
        return f"{label}: below source floor"
    return None


def source_pose_chain_issues(window, label):
    issues = []
    required = window.get("requires_source_pose_chain")
    chain = window.get("source_pose_chain")
    reported_failure = window.get("first_failed_source_pose_boundary")
    if not isinstance(required, bool):
        return [f"motion window source pose chain requirement is invalid: {label}"]
    if required is False:
        if chain not in (None, []):
            issues.append(f"motion window has an unexpected source pose chain: {label}")
        if reported_failure is not None:
            issues.append(f"motion window has an unexpected source pose chain failure: {label}")
        return issues
    if not isinstance(chain, list) or not chain:
        return [f"motion window source pose chain is missing: {label}"]
    labels = []
    expected_failure = None
    for boundary in chain:
        if not isinstance(boundary, dict):
            issues.append(f"motion window source pose boundary is invalid: {label}")
            continue
        boundary_label = boundary.get("label")
        graph = boundary.get("graph")
        node = boundary.get("node")
        output = boundary.get("output")
        labels.append(boundary_label)
        for name, value in (("label", boundary_label), ("graph", graph), ("node", node), ("output", output)):
            if not isinstance(value, str) or not value:
                issues.append(f"motion window source pose boundary {name} is invalid: {label}")
        for name in (
            "minimum_translation_span",
            "minimum_rotation_span_radians",
            "max_translation_span",
            "max_rotation_span_radians",
        ):
            value = boundary.get(name)
            if not finite(value) or value < 0.0:
                issues.append(f"motion window source pose boundary {name} is invalid: {label}")
        for name in ("sample_count", "available_samples", "unavailable_samples"):
            value = boundary.get(name)
            if not isinstance(value, int) or value < 0:
                issues.append(f"motion window source pose boundary {name} is invalid: {label}")
        if expected_failure is None:
            expected_failure = source_pose_boundary_failure(boundary)
    if all(isinstance(value, str) and value for value in labels) and len(set(labels)) != len(labels):
        issues.append(f"motion window source pose chain has duplicate boundaries: {label}")
    if reported_failure != expected_failure:
        issues.append(f"motion window source pose chain first failure is inconsistent: {label}")
    if expected_failure is not None:
        issues.append(f"motion window source pose chain failed: {label}: {expected_failure}")
    return issues


def motion_window_issues(report, screenshots, pose_samples, require_screenshots=True):
    issues = []
    windows = report.get("motion_windows")
    if not isinstance(windows, list):
        return ["motion windows are invalid"]
    required_windows = report.get("required_motion_windows")
    if not isinstance(required_windows, list) or not all(
        isinstance(value, str) and value for value in required_windows
    ):
        issues.append("required motion windows are invalid")
        required_windows = []
    elif len(set(required_windows)) != len(required_windows):
        issues.append("required motion windows contain duplicates")
    observed_labels = {
        window.get("label")
        for window in windows
        if isinstance(window, dict) and isinstance(window.get("label"), str)
    }
    missing_required = sorted(set(required_windows) - observed_labels)
    if missing_required:
        issues.append("required motion windows are missing: " + ", ".join(missing_required))
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
        requires_actor_yaw_motion = window.get("requires_actor_yaw_motion")
        requires_actor_yaw_convergence = window.get("requires_actor_yaw_convergence")
        pre_translation_floor = window.get("minimum_pre_rig_translation_span")
        pre_rotation_floor = window.get("minimum_pre_rig_rotation_span_radians")
        final_translation_floor = window.get("minimum_final_translation_span")
        final_rotation_floor = window.get("minimum_final_rotation_span_radians")
        actor_yaw_floor = window.get("minimum_actor_yaw_delta_radians")
        actor_yaw_convergence_floor = window.get("minimum_actor_yaw_convergence_radians")
        actor_yaw_sign = window.get("expected_actor_yaw_delta_sign")
        actor_yaw_baseline = window.get("expected_actor_yaw_baseline")
        expected_animation_variant = window.get("expected_animation_variant")
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
            ("requires_source_pose_chain", window.get("requires_source_pose_chain")),
            ("requires_actor_yaw_motion", requires_actor_yaw_motion),
            ("requires_actor_yaw_convergence", requires_actor_yaw_convergence),
        ):
            if not isinstance(value, bool):
                issues.append(f"motion window requirement is invalid: {label}.{name}")
        for name, value in (
            ("pre-rig translation floor", pre_translation_floor),
            ("pre-rig rotation floor", pre_rotation_floor),
            ("final translation floor", final_translation_floor),
            ("final rotation floor", final_rotation_floor),
            ("actor yaw floor", actor_yaw_floor),
            ("actor yaw convergence floor", actor_yaw_convergence_floor),
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
        if not window.get("direction_signature_group") and window.get("canonical_clip_time") is not None:
            target = window["canonical_clip_time"]
            distance = window.get("canonical_clip_time_distance")
            maximum = window.get("maximum_clip_time_distance")
            if not finite(target) or target < 0.0 or not finite(maximum) or maximum <= MOTION_EPSILON:
                issues.append(f"motion window canonical clip target is invalid: {label}")
            if not finite(distance) or distance < 0.0 or not finite(maximum) or distance > maximum:
                issues.append(f"motion window canonical clip sample is missing or outside tolerance: {label}")
            issues.extend(local_motion_pose_issues(window.get("canonical_pre_rig_pose"), label))
        actor_yaw_driver = window.get("actor_yaw_driver") or "animation_curve"
        if actor_yaw_driver not in ("animation_curve", "locomotion"):
            issues.append(f"motion window actor yaw driver is invalid: {label}")
        if actor_yaw_driver == "locomotion" and (
            not isinstance(window.get("expected_runtime_state"), int)
            or not window.get("source_state")
        ):
            issues.append(f"motion window locomotion yaw state is missing: {label}")
        actor_yaw_sample_count = window.get("actor_yaw_sample_count")
        actor_yaw_delta = window.get("max_actor_yaw_delta_radians")
        actor_yaw_signed_delta = window.get("max_actor_yaw_signed_delta_radians")
        actor_yaw_opposite_delta = window.get("max_actor_yaw_opposite_delta_radians")
        actor_yaw_convergence = window.get("max_actor_yaw_convergence_radians")
        actor_yaw_trace = window.get("actor_yaw_trace")
        if not isinstance(actor_yaw_sample_count, int) or actor_yaw_sample_count < 0:
            issues.append(f"motion window actor yaw sample count is invalid: {label}")
        if not finite(actor_yaw_delta) or actor_yaw_delta < 0.0:
            issues.append(f"motion window actor yaw delta is invalid: {label}")
        if not finite(actor_yaw_signed_delta) or actor_yaw_signed_delta < 0.0:
            issues.append(f"motion window signed actor yaw delta is invalid: {label}")
        if not finite(actor_yaw_opposite_delta) or actor_yaw_opposite_delta < 0.0:
            issues.append(f"motion window opposite actor yaw delta is invalid: {label}")
        if not finite(actor_yaw_convergence) or actor_yaw_convergence < 0.0:
            issues.append(f"motion window actor yaw convergence is invalid: {label}")
        if actor_yaw_sign is not None and (
            not isinstance(actor_yaw_sign, int)
            or isinstance(actor_yaw_sign, bool)
            or actor_yaw_sign not in (-1, 1)
        ):
            issues.append(f"motion window actor yaw sign is invalid: {label}")
        if actor_yaw_sign is not None and not finite(actor_yaw_baseline):
            issues.append(f"motion window actor yaw baseline is invalid: {label}")
        if actor_yaw_sign is not None and actor_yaw_driver == "animation_curve" and (
            not isinstance(expected_animation_variant, str) or not expected_animation_variant
        ):
            issues.append(f"motion window expected animation variant is invalid: {label}")
        if actor_yaw_sign is None and window.get("canonical_clip_time") is not None and not window.get("source_sync_group"):
            if not isinstance(expected_animation_variant, str) or not expected_animation_variant:
                issues.append(f"motion window expected animation variant is invalid: {label}")
            if not isinstance(actor_yaw_trace, list) or len(actor_yaw_trace) < 2:
                issues.append(f"motion window animation clip trace is missing: {label}")
            else:
                previous_elapsed = None
                active_samples = 0
                for sample in actor_yaw_trace:
                    if not isinstance(sample, dict):
                        issues.append(f"motion window animation clip trace sample is invalid: {label}")
                        continue
                    elapsed = sample.get("elapsed_secs")
                    if not finite(elapsed) or (previous_elapsed is not None and elapsed <= previous_elapsed):
                        issues.append(f"motion window animation clip trace is not ordered: {label}")
                    if finite(elapsed):
                        previous_elapsed = elapsed
                    for field in ("animation_clip_time", "animation_weight", "animation_play_rate"):
                        if not finite(sample.get(field)) or sample[field] < 0.0:
                            issues.append(f"motion window animation clip trace {field} is invalid: {label}")
                    weight = sample.get("animation_weight")
                    if finite(weight) and weight > MOTION_EPSILON:
                        active_samples += 1
                        if sample.get("animation_variant") != expected_animation_variant:
                            issues.append(f"motion window selected the wrong animation variant: {label}")
                if active_samples < 2:
                    issues.append(f"motion window animation clip trace has insufficient active samples: {label}")
        if actor_yaw_sign is not None:
            if not isinstance(actor_yaw_trace, list) or len(actor_yaw_trace) < 2:
                issues.append(f"motion window actor yaw trace is missing: {label}")
            else:
                previous_elapsed = None
                has_curve_sample = False
                for sample in actor_yaw_trace:
                    if not isinstance(sample, dict):
                        issues.append(f"motion window actor yaw trace sample is invalid: {label}")
                        continue
                    elapsed = sample.get("elapsed_secs")
                    actor = sample.get("actor_yaw_radians")
                    view = sample.get("view_yaw_radians")
                    curve = sample.get("animation_yaw_speed_degrees")
                    direct_curve = sample.get("animation_curve_direct_yaw_speed_degrees")
                    clip_time = sample.get("animation_clip_time")
                    weight = sample.get("animation_weight")
                    play_rate = sample.get("animation_play_rate")
                    curve_stack_weight = sample.get("animation_curve_stack_weight")
                    curve_stack_entries = sample.get("animation_curve_stack_entries")
                    animation_variant = sample.get("animation_variant")
                    if not finite(elapsed) or not finite(actor):
                        issues.append(f"motion window actor yaw trace values are invalid: {label}")
                    if previous_elapsed is not None and finite(elapsed) and elapsed <= previous_elapsed:
                        issues.append(f"motion window actor yaw trace is not ordered: {label}")
                    if finite(elapsed):
                        previous_elapsed = elapsed
                    if view is not None and not finite(view):
                        issues.append(f"motion window actor yaw trace view is invalid: {label}")
                    if curve is not None:
                        if not finite(curve):
                            issues.append(f"motion window actor yaw trace curve is invalid: {label}")
                        elif abs(curve) > MOTION_EPSILON:
                            has_curve_sample = True
                    if actor_yaw_driver == "locomotion":
                        if not finite(view) or not finite(curve) or abs(curve) > MOTION_EPSILON:
                            issues.append(f"motion window locomotion yaw inputs are invalid: {label}")
                        continue
                    if (
                        finite(curve_stack_weight)
                        and curve_stack_weight > MOTION_EPSILON
                        and not finite(direct_curve)
                    ):
                        issues.append(f"motion window actor yaw trace direct curve is invalid: {label}")
                    if not finite(clip_time) or clip_time < 0.0:
                        issues.append(f"motion window actor yaw trace clip time is invalid: {label}")
                    if not finite(weight) or weight < 0.0:
                        issues.append(f"motion window actor yaw trace weight is invalid: {label}")
                    elif finite(curve) and weight <= MOTION_EPSILON and abs(curve) > MOTION_EPSILON:
                        issues.append(f"motion window inactive clip emitted actor yaw curve: {label}")
                    if not finite(play_rate) or play_rate < 0.0:
                        issues.append(f"motion window actor yaw trace play rate is invalid: {label}")
                    if not finite(curve_stack_weight) or curve_stack_weight < 0.0:
                        issues.append(f"motion window actor yaw trace curve stack weight is invalid: {label}")
                    if not isinstance(curve_stack_entries, list):
                        issues.append(f"motion window actor yaw trace curve stack entries are invalid: {label}")
                    else:
                        for entry in curve_stack_entries:
                            if not isinstance(entry, dict):
                                issues.append(
                                    f"motion window actor yaw trace curve stack entry is invalid: {label}"
                                )
                                continue
                            variant = entry.get("animation_variant")
                            entry_elapsed = entry.get("elapsed_seconds")
                            entry_weight = entry.get("weight")
                            entry_play_rate = entry.get("play_rate")
                            entry_ready = entry.get("sampling_ready")
                            if not isinstance(variant, str) or not variant:
                                issues.append(
                                    f"motion window actor yaw trace curve stack variant is invalid: {label}"
                                )
                            for name, value in (
                                ("elapsed", entry_elapsed),
                                ("weight", entry_weight),
                                ("play rate", entry_play_rate),
                            ):
                                if not finite(value) or value < 0.0:
                                    issues.append(
                                        f"motion window actor yaw trace curve stack {name} is invalid: {label}"
                                    )
                            if not isinstance(entry_ready, bool):
                                issues.append(
                                    f"motion window actor yaw trace curve stack readiness is invalid: {label}"
                                )
                        if (
                            finite(weight)
                            and weight > MOTION_EPSILON
                            and finite(curve_stack_weight)
                            and curve_stack_weight > MOTION_EPSILON
                            and not curve_stack_entries
                        ):
                            issues.append(
                                f"motion window active clip has no curve stack entry: {label}"
                            )
                    if actor_yaw_sign is not None and (
                        not isinstance(animation_variant, str) or not animation_variant
                    ):
                        issues.append(f"motion window actor yaw trace animation variant is invalid: {label}")
                    elif (
                        isinstance(expected_animation_variant, str)
                        and finite(weight)
                        and weight > MOTION_EPSILON
                        and animation_variant != expected_animation_variant
                    ):
                        issues.append(f"motion window selected the wrong animation variant: {label}")
                if actor_yaw_driver == "animation_curve" and not has_curve_sample:
                    issues.append(f"motion window actor yaw trace has no animation curve: {label}")
        if requires_actor_yaw_motion is True and (
            not isinstance(actor_yaw_sample_count, int)
            or actor_yaw_sample_count < 2
            or not finite(actor_yaw_floor)
            or not finite(actor_yaw_delta)
            or actor_yaw_delta < actor_yaw_floor
            or (
                actor_yaw_sign is not None
                and (
                    not finite(actor_yaw_signed_delta)
                    or actor_yaw_signed_delta < actor_yaw_floor
                )
            )
        ):
            issues.append(f"motion window actor yaw did not follow source motion: {label}")
        if requires_actor_yaw_convergence is True and (
            not isinstance(actor_yaw_sample_count, int)
            or actor_yaw_sample_count < 2
            or not finite(actor_yaw_convergence_floor)
            or not finite(actor_yaw_convergence)
            or actor_yaw_convergence < actor_yaw_convergence_floor
        ):
            issues.append(f"motion window actor yaw did not converge toward the source view: {label}")
        if (
            actor_yaw_sign is not None
            and isinstance(actor_yaw_sample_count, int)
            and actor_yaw_sample_count >= 2
            and finite(actor_yaw_floor)
            and finite(actor_yaw_signed_delta)
            and finite(actor_yaw_opposite_delta)
            and actor_yaw_opposite_delta >= actor_yaw_floor
            and actor_yaw_opposite_delta > actor_yaw_signed_delta + MOTION_EPSILON
        ):
            issues.append(f"motion window actor yaw turned opposite to the source action: {label}")
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
        root_translation_delta = window.get("root_pose_max_translation_span")
        root_rotation_delta = window.get("root_pose_max_rotation_span_radians")
        for name, value in (
            ("sample count", root_sample_count),
            ("active samples", root_active_samples),
            ("reference samples", root_reference_samples),
            ("maximum channels", root_max_channels),
        ):
            if not isinstance(value, int) or value < 0:
                issues.append(f"motion window root pose {name} is invalid: {label}")
        for name, value in (
            ("translation delta", root_translation_delta),
            ("rotation delta", root_rotation_delta),
        ):
            if not finite(value) or value < 0.0:
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
        if requires_pre_rig_motion is True and (
            finite(root_translation_delta)
            and finite(root_rotation_delta)
            and root_translation_delta <= MOTION_EPSILON
            and root_rotation_delta <= MOTION_EPSILON
        ):
            issues.append(f"motion window root graph pose is static: {label}")
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
        expected_parent_runtime_state = window.get("expected_parent_runtime_state")
        observed_parent_runtime_states = window.get("observed_parent_runtime_states")
        observed_parent_transition_active = window.get("observed_parent_transition_active")
        observed_requested_parent_runtime_states = window.get(
            "observed_requested_parent_runtime_states"
        )
        observed_requested_parent_transition_active = window.get(
            "observed_requested_parent_transition_active"
        )
        if expected_parent_runtime_state is not None and (
            not isinstance(expected_parent_runtime_state, int) or expected_parent_runtime_state < 0
        ):
            issues.append(f"motion window expected parent runtime state is invalid: {label}")
        if not isinstance(observed_parent_runtime_states, list) or not all(
            isinstance(value, int) and value >= 0 for value in observed_parent_runtime_states
        ):
            issues.append(f"motion window observed parent runtime states are invalid: {label}")
        elif (
            expected_parent_runtime_state is not None
            and observed_parent_runtime_states != [expected_parent_runtime_state]
        ):
            issues.append(f"motion window selected the wrong parent runtime state: {label}")
        if not isinstance(observed_parent_transition_active, list) or not all(
            isinstance(value, bool) for value in observed_parent_transition_active
        ):
            issues.append(f"motion window observed parent transition state is invalid: {label}")
        elif expected_parent_runtime_state is not None and False not in observed_parent_transition_active:
            issues.append(f"motion window parent transition did not settle: {label}")
        if not isinstance(observed_requested_parent_runtime_states, list) or not all(
            isinstance(value, int) and value >= 0
            for value in observed_requested_parent_runtime_states
        ):
            issues.append(f"motion window requested parent runtime states are invalid: {label}")
        elif (
            expected_parent_runtime_state is not None
            and (observed_requested_parent_runtime_states or observed_requested_parent_transition_active)
            and observed_requested_parent_runtime_states != [expected_parent_runtime_state]
        ):
            issues.append(f"motion window requested the wrong parent runtime state: {label}")
        if not isinstance(observed_requested_parent_transition_active, list) or not all(
            isinstance(value, bool) for value in observed_requested_parent_transition_active
        ):
            issues.append(f"motion window requested parent transition state is invalid: {label}")
        elif (
            expected_parent_runtime_state is not None
            and (observed_requested_parent_runtime_states or observed_requested_parent_transition_active)
            and False not in observed_requested_parent_transition_active
        ):
            issues.append(f"motion window requested parent transition did not settle: {label}")
        expected_terminal_parent_runtime_state = window.get(
            "expected_terminal_parent_runtime_state"
        )
        expected_terminal_parent_transition_active = window.get(
            "expected_terminal_parent_transition_active"
        )
        expected_terminal_requested_parent_transition_active = window.get(
            "expected_terminal_requested_parent_transition_active"
        )
        observed_terminal_parent_runtime_states = window.get(
            "observed_terminal_parent_runtime_states"
        )
        observed_terminal_parent_transition_active = window.get(
            "observed_terminal_parent_transition_active"
        )
        observed_terminal_requested_parent_runtime_states = window.get(
            "observed_terminal_requested_parent_runtime_states"
        )
        observed_terminal_requested_parent_transition_active = window.get(
            "observed_terminal_requested_parent_transition_active"
        )
        if expected_terminal_parent_runtime_state is not None and (
            not isinstance(expected_terminal_parent_runtime_state, int)
            or expected_terminal_parent_runtime_state < 0
        ):
            issues.append(f"motion window expected terminal parent state is invalid: {label}")
        if expected_terminal_parent_runtime_state is None and (
            expected_terminal_parent_transition_active is not None
            or expected_terminal_requested_parent_transition_active is not None
        ):
            issues.append(f"motion window terminal transition expectation is invalid: {label}")
        if expected_terminal_parent_runtime_state is not None and (
            not isinstance(expected_terminal_parent_transition_active, bool)
            or (expected_terminal_requested_parent_transition_active is not None
                and not isinstance(expected_terminal_requested_parent_transition_active, bool))
        ):
            issues.append(f"motion window terminal transition expectation is invalid: {label}")
        for name, values, kind in (
            (
                "terminal parent runtime states",
                observed_terminal_parent_runtime_states,
                int,
            ),
            (
                "terminal requested parent runtime states",
                observed_terminal_requested_parent_runtime_states,
                int,
            ),
            (
                "terminal parent transition states",
                observed_terminal_parent_transition_active,
                bool,
            ),
            (
                "terminal requested parent transition states",
                observed_terminal_requested_parent_transition_active,
                bool,
            ),
        ):
            if not isinstance(values, list) or not all(
                isinstance(value, kind)
                and (
                    isinstance(value, bool)
                    if kind is bool
                    else not isinstance(value, bool) and value >= 0
                )
                for value in values
            ):
                issues.append(f"motion window {name} are invalid: {label}")
        if expected_terminal_parent_runtime_state is None:
            if any(
                values
                for values in (
                    observed_terminal_parent_runtime_states,
                    observed_terminal_parent_transition_active,
                    observed_terminal_requested_parent_runtime_states,
                    observed_terminal_requested_parent_transition_active,
                )
                if isinstance(values, list)
            ):
                issues.append(f"motion window has unexpected terminal parent state: {label}")
        elif (
            observed_terminal_parent_runtime_states
            != [expected_terminal_parent_runtime_state]
        ):
            issues.append(f"motion window selected the wrong terminal parent state: {label}")
        elif (
            expected_terminal_requested_parent_transition_active is not None
            and observed_terminal_requested_parent_runtime_states
            != [expected_terminal_parent_runtime_state]
        ):
            issues.append(f"motion window requested the wrong terminal parent state: {label}")
        elif (
            observed_terminal_parent_transition_active
            != [expected_terminal_parent_transition_active]
        ):
            issues.append(f"motion window terminal parent transition is wrong: {label}")
        elif (
            expected_terminal_requested_parent_transition_active is not None
            and observed_terminal_requested_parent_transition_active
            != [expected_terminal_requested_parent_transition_active]
        ):
            issues.append(f"motion window terminal requested parent transition is wrong: {label}")
        expected_lifecycle = window.get("expected_lifecycle_stages")
        observed_lifecycle = window.get("observed_lifecycle_stages")
        if not isinstance(expected_lifecycle, list) or not all(
            isinstance(value, str) and value for value in expected_lifecycle
        ):
            issues.append(f"motion window expected lifecycle is invalid: {label}")
        if not isinstance(observed_lifecycle, list) or not all(
            isinstance(value, str) and value for value in observed_lifecycle
        ):
            issues.append(f"motion window observed lifecycle is invalid: {label}")
        elif isinstance(expected_lifecycle, list) and observed_lifecycle != expected_lifecycle:
            issues.append(f"motion window lifecycle is incomplete: {label}")
        matching_screenshots = [
            value
            for value in screenshots
            if isinstance(value, dict)
            and value.get("kind") == "direction"
            and value.get("direction_case") == label
        ]
        if require_screenshots and not matching_screenshots:
            issues.append(f"motion window screenshot is missing: {label}")
        matching_samples = [
            value
            for value in pose_samples
            if isinstance(value, dict) and value.get("direction_case") == label
        ]
        if len(matching_samples) < 2:
            issues.append(f"motion window pose samples are missing: {label}")
        issues.extend(source_pose_chain_issues(window, label))
    issues.extend(direction_signature_issues(windows))
    return issues


def temporal_anomaly_issues(values):
    if not isinstance(values, list):
        return ["temporal_anomalies is invalid"]
    issues = []
    for value in values:
        if not isinstance(value, dict):
            issues.append("temporal anomaly is invalid")
            continue
        if value.get("stage") not in ("pre_rig", "final"):
            issues.append("temporal anomaly stage is invalid")
        for key in ("kind", "subject", "trigger"):
            if not isinstance(value.get(key), str) or not value.get(key):
                issues.append(f"temporal anomaly field is invalid: {key}")
        if value.get("trigger") not in ("jump", "rate", "ratio", "plane_flip"):
            issues.append("temporal anomaly trigger is invalid")
        for key in ("previous", "current", "threshold", "elapsed_secs", "frame_ms"):
            if not finite(value.get(key)):
                issues.append(f"temporal anomaly value is invalid: {key}")
        for key in ("rate", "rate_threshold"):
            rate = value.get(key)
            if rate is not None and (not finite(rate) or rate < 0.0):
                issues.append(f"temporal anomaly rate is invalid: {key}")
        if value.get("trigger") == "rate" and (
            not finite(value.get("rate")) or not finite(value.get("rate_threshold"))
        ):
            issues.append("temporal anomaly rate trigger is incomplete")
        for key in ("direction_case", "phase"):
            label = value.get(key)
            if label is not None and (not isinstance(label, str) or not label):
                issues.append(f"temporal anomaly label is invalid: {key}")
    source_values = 0
    for value in values:
        if not isinstance(value, dict) or value.get("source_boundary") is None:
            continue
        inputs = value.get("source_selector_inputs")
        source_match = (
            value.get("source_boundary") == "source_fall_heavy_to_light_on_physics_entry"
            and value.get("stage") == "pre_rig"
            and value.get("kind") == "global_position_speed"
            and value.get("subject") in ("hand_l", "hand_r")
            and value.get("trigger") == "rate"
            and value.get("threshold") == 40.0
            and isinstance(inputs, list) and len(inputs) == 2
            and all(isinstance(sample, list) and len(sample) == 2 and all(finite(number) for number in sample) for sample in inputs)
            and inputs[0][0] <= -10.0 and inputs[0][1] == 0.0
            and inputs[1][0] >= -5.0 and inputs[1][1] == 1.0
        )
        if source_match:
            source_values += 1
        else:
            issues.append("temporal anomaly source boundary is invalid")
    if len(values) != source_values:
        issues.append("temporal_anomalies is non-empty")
    return issues


def primary_failure_issues(report):
    graph_errors = report.get("graph_errors")
    failure = report.get("primary_failure")
    if not isinstance(graph_errors, list):
        return ["graph_errors is invalid"], None
    if not graph_errors:
        if failure is not None:
            return ["primary_failure exists without graph error"], None
        return [], None
    if not all(isinstance(value, str) and value for value in graph_errors):
        return ["graph_errors contains an invalid value"], None
    if not isinstance(failure, dict):
        return ["graph failure diagnostics are missing: primary_failure"], None
    issues = []
    if failure.get("category") not in {"graph_evaluation_error", "root_pose_contract_error"}:
        issues.append("primary failure category is invalid")
    for key in ("raw_error", "error_kind", "root_graph_recursive_dependencies", "case", "pose_classification", "screenshot_status", "candidate_resolution"):
        if not isinstance(failure.get(key), str) or not failure.get(key):
            issues.append(f"primary failure field is invalid: {key}")
    if not isinstance(failure.get("reported_before_ready"), bool):
        issues.append("primary failure reported_before_ready is invalid")
    for key in ("direction_case", "phase", "locomotion", "gait", "stance", "rotation_mode", "root_graph_path", "screenshot_path"):
        value = failure.get(key)
        if value is not None and (not isinstance(value, str) or not value):
            issues.append(f"primary failure label is invalid: {key}")
    phase_elapsed = failure.get("phase_elapsed_secs")
    if phase_elapsed is not None and (not finite(phase_elapsed) or phase_elapsed < 0.0):
        issues.append("primary failure phase elapsed time is invalid")
    pin = failure.get("pin")
    if pin is not None:
        if not isinstance(pin, dict):
            issues.append("primary failure pin is invalid")
        else:
            if not isinstance(pin.get("kind"), str) or not pin.get("kind"):
                issues.append("primary failure pin kind is invalid")
            for key in ("node_id", "pin_id", "graph_context"):
                value = pin.get(key)
                if value is not None and (not isinstance(value, str) or not value):
                    issues.append(f"primary failure pin field is invalid: {key}")
    candidates = failure.get("node_candidates")
    if not isinstance(candidates, list):
        issues.append("primary failure node candidates are invalid")
        candidates = []
    active_candidates = []
    for candidate in candidates:
        if not isinstance(candidate, dict):
            issues.append("primary failure node candidate is invalid")
            continue
        for key in ("graph_asset_id", "node_id", "node_name", "node_kind"):
            if not isinstance(candidate.get(key), str) or not candidate.get(key):
                issues.append(f"primary failure candidate field is invalid: {key}")
        for key in ("graph_path", "graph_context"):
            value = candidate.get(key)
            if value is not None and (not isinstance(value, str) or not value):
                issues.append(f"primary failure candidate field is invalid: {key}")
        context_curves = candidate.get("context_curves")
        if not isinstance(context_curves, dict) or not all(
            isinstance(name, str) and name and finite(value)
            for name, value in context_curves.items()
        ):
            issues.append("primary failure candidate context_curves is invalid")
        if not isinstance(candidate.get("active_context"), bool):
            issues.append("primary failure candidate active_context is invalid")
        elif candidate["active_context"]:
            active_candidates.append(candidate)
    context_nodes = failure.get("active_context_nodes")
    if not isinstance(context_nodes, list):
        issues.append("primary failure active_context_nodes is invalid")
        context_nodes = []
    for context_node in context_nodes:
        if not isinstance(context_node, dict):
            issues.append("primary failure context node is invalid")
            continue
        for key in ("graph_context", "node_id", "node_name", "node_kind"):
            if not isinstance(context_node.get(key), str) or not context_node.get(key):
                issues.append(f"primary failure context node field is invalid: {key}")
        graph_path = context_node.get("graph_path")
        if graph_path is not None and (not isinstance(graph_path, str) or not graph_path):
            issues.append("primary failure context node graph_path is invalid")
        context_curves = context_node.get("context_curves")
        if not isinstance(context_curves, dict) or not all(
            isinstance(name, str) and name and finite(value)
            for name, value in context_curves.items()
        ):
            issues.append("primary failure context node curves are invalid")
    active_count = failure.get("active_node_candidate_count")
    if not isinstance(active_count, int) or active_count < 0:
        issues.append("primary failure active candidate count is invalid")
        active_count = -1
    elif active_count != len(active_candidates):
        issues.append("primary failure active candidate count disagrees with candidates")
    resolution = failure.get("candidate_resolution")
    has_node = bool(candidates) or isinstance(pin, dict) and pin.get("node_id") is not None
    if has_node:
        expected_resolution = (
            "resolved_active_context"
            if active_count == 1
            else "ambiguous_active_context"
            if active_count > 1
            else "not_resolved_from_active_context"
        )
        if resolution != expected_resolution:
            issues.append("primary failure candidate resolution is invalid")
    elif resolution != "not_applicable":
        issues.append("primary failure candidate resolution is invalid")
    root_pose = failure.get("root_pose")
    if not isinstance(root_pose, dict):
        issues.append("primary failure root pose is invalid")
    else:
        if not isinstance(root_pose.get("output_present"), bool):
            issues.append("primary failure root pose output flag is invalid")
        for key in ("bone_count", "active_channel_count"):
            value = root_pose.get(key)
            if not isinstance(value, int) or value < 0:
                issues.append(f"primary failure root pose metric is invalid: {key}")
        classification = failure.get("pose_classification")
        if root_pose.get("output_present") is False and classification != "missing_root_pose_likely_reference_or_a_pose":
            issues.append("primary failure missing root pose classification is invalid")
        elif root_pose.get("output_present") is True and root_pose.get("active_channel_count") == 0 and classification != "reference_root_pose_likely_a_pose":
            issues.append("primary failure reference root pose classification is invalid")
        elif root_pose.get("output_present") is True and root_pose.get("active_channel_count", 0) > 0 and classification != "root_pose_channels_present_graph_error":
            issues.append("primary failure active root pose classification is invalid")
        if failure.get("category") == "root_pose_contract_error":
            expected_kind = "EmptyRootPose" if root_pose.get("output_present") else "MissingRootPose"
            if failure.get("error_kind") != expected_kind or root_pose.get("active_channel_count") != 0:
                issues.append("primary failure root pose contract is inconsistent")
    status = failure.get("screenshot_status")
    allowed_status = {
        "capture_scheduled",
        "captured",
        "capture_not_materialized_before_exit",
        "unavailable_no_camera",
        "disabled_by_configuration",
    }
    if status not in allowed_status:
        issues.append("primary failure screenshot status is invalid")
    elif status in ("unavailable_no_camera", "disabled_by_configuration") and failure.get("screenshot_path") is not None:
        issues.append("primary failure unavailable screenshot has a path")
    elif status not in ("unavailable_no_camera", "disabled_by_configuration") and not isinstance(failure.get("screenshot_path"), str):
        issues.append("primary failure screenshot path is missing")
    candidate_locations = [
        f"{candidate.get('graph_path') or candidate.get('graph_asset_id')}:{candidate.get('node_name')}"
        for candidate in active_candidates
    ]
    summary = f"primary graph failure: {failure.get('error_kind', 'unknown')}; {failure.get('raw_error', 'unknown')}"
    if candidate_locations:
        summary += "; active candidates=" + ", ".join(candidate_locations)
    context_names = sorted(
        {
            name
            for candidate in active_candidates
            for name in candidate.get("context_curves", {})
        }
    )
    if context_names:
        summary += "; active context curves=" + ", ".join(context_names)
    if context_nodes:
        summary += "; context nodes=" + ", ".join(
            sorted(context_node.get("node_name", "") for context_node in context_nodes)
        )
    if resolution == "ambiguous_active_context":
        summary += "; active candidate resolution is ambiguous"
    return issues, {
        "summary": summary,
        "error_kind": failure.get("error_kind"),
        "candidate_resolution": resolution,
        "pose_classification": failure.get("pose_classification"),
        "reported_before_ready": failure.get("reported_before_ready"),
        "active_context_node_count": len(context_nodes),
    }


def camera_trace_issues(report):
    if report.get("case") not in ("camera", "ragdoll"):
        return []
    trace = (report.get("manual_integration") or {}).get("camera_trace")
    if not isinstance(trace, list) or len(trace) < 2:
        return ["camera trace evidence is missing"]
    previous = -1.0
    collision = False
    recovery = False
    ragdoll_baseline = False
    ragdoll_frames = 0
    for sample in trace:
        if not isinstance(sample, dict) or not isinstance(sample.get("camera"), dict):
            return ["camera trace sample is invalid"]
        time = sample.get("phase_elapsed_seconds")
        camera = sample["camera"]
        if not finite(time) or time < previous:
            return ["camera trace time is invalid"]
        previous = time
        if any(not isinstance(sample.get(key), bool) for key in ["obstacles_present", "obstacles_ignored", "first_person_requested", "actual_first_person"]):
            return ["camera trace lifecycle is invalid"]
        if any(not finite(camera.get(key)) for key in ["trace_distance_ratio", "trace_radius", "first_person_override", "unscaled_delta_seconds"]):
            return ["camera trace numeric data is invalid"]
        for key in ["trace_start", "trace_end"]:
            vector = camera.get(key)
            if not isinstance(vector, list) or len(vector) != 3 or not all(finite(value) for value in vector):
                return ["camera trace position is invalid"]
        curves = camera.get("curve_values")
        if not isinstance(curves, list) or len(curves) != 13 or not all(finite(value) for value in curves):
            return ["camera trace curves are invalid"]
        if report.get("case") == "ragdoll":
            active = sample.get("ragdoll_active")
            if not isinstance(active, bool):
                return ["ragdoll camera lifecycle evidence is invalid"]
            clear_third_person = (camera.get("valid") is True and not sample["actual_first_person"]
                and not sample["first_person_requested"] and camera["first_person_override"] <= 1e-5
                and camera.get("trace_blocking_hit") is False and camera["trace_distance_ratio"] >= 0.99)
            if active:
                if not ragdoll_baseline:
                    return ["ragdoll camera clear third-person precondition is missing"]
                if not clear_third_person:
                    return ["ragdoll entry changed or retracted the clear third-person camera"]
                ragdoll_frames += 1
            else:
                ragdoll_baseline |= clear_third_person
            continue
        collision |= (sample["obstacles_present"] and camera.get("valid") is True
            and camera.get("trace_blocking_hit") is True and camera.get("trace_adjustment_resolved") is True
            and isinstance(camera.get("trace_adjustment_count"), int) and camera["trace_adjustment_count"] >= 2
            and camera["trace_distance_ratio"] < 0.98)
        blocked = sample.get("blocked_ratio")
        recovery |= (collision and sample["obstacles_ignored"] and sample["obstacles_present"]
            and camera.get("valid") is True and camera.get("trace_blocking_hit") is False
            and camera["first_person_override"] < 1.0 - 1e-5 and finite(blocked)
            and camera["trace_distance_ratio"] > blocked + 0.02)
    issues = []
    if report.get("case") == "ragdoll":
        return [] if ragdoll_frames >= 2 else ["ragdoll camera active frame evidence is missing"]
    if not collision:
        issues.append("camera trace does not prove collision with resolved MTDs")
    if not recovery:
        issues.append("camera trace does not prove third-person recovery after camera-layer exclusion with obstacles still present")
    return issues


def stairs_trace_issues(report):
    if report.get("case") != "stairs":
        return []
    samples = report.get("manual_integration", {}).get("stairs_trace")
    if not isinstance(samples, list) or not samples or not all(isinstance(value, dict) for value in samples):
        return ["stairs frame evidence is missing"]
    issues = []
    speeds = {"Walking": 1.75, "Running": 3.75, "Sprinting": 6.5}
    windows = {value.get("label") for value in report.get("motion_windows", [])}
    for route in range(6):
        frames = [value for value in samples if value.get("route_index") == route]
        prefix = f"stairs_{route // 2 + 1:02}_{'up' if route % 2 == 0 else 'down'}"
        if not frames:
            issues.append(f"{prefix}: route evidence is missing")
            continue
        if not all(value.get("mesh") == f"SM_Als_Stairs_{route // 2 + 1:02}" for value in frames):
            issues.append(f"{prefix}: source mesh does not match")
        counts = dict.fromkeys(speeds, 0)
        modes = set()
        hotkeys = set()
        previous = None
        gait_started = 0.0
        distances = []
        route_length = None
        for frame in frames:
            required = ("elapsed_seconds", "simulation_delta_seconds", "distance_m", "route_length_m", "expected_speed", "requested_speed", "animation_speed")
            if not all(finite(frame.get(key)) for key in required) or not all(
                isinstance(frame.get(key), list) and len(frame[key]) == length and all(finite(value) for value in frame[key])
                for key, length in (("position", 3), ("velocity", 3), ("input", 2))
            ):
                issues.append(f"{prefix}: frame values are invalid")
                break
            now = frame["elapsed_seconds"]
            delta = frame["simulation_delta_seconds"]
            if delta <= 0.0 or frame["route_length_m"] <= 0.0 or (previous and now <= previous["elapsed_seconds"]):
                issues.append(f"{prefix}: frame time or route length is invalid")
                break
            route_length = frame["route_length_m"]
            distances.append(frame["distance_m"])
            desired = frame.get("desired_gait")
            gait_name = desired.removeprefix("Als.Gait.") if isinstance(desired, str) else desired
            traversing = frame.get("stage") == "Traverse"
            if not previous or previous.get("desired_gait") != desired or previous.get("stage") != "Traverse" or not traversing:
                gait_started = now
            if traversing and frame.get("locomotion") != "Grounded":
                issues.append(f"{prefix}: stair traversal became airborne")
                break
            if frame.get("stage") in ("VelocityMode", "ViewMode"):
                mode = frame.get("current_mode")
                if mode == frame.get("desired_mode") and frame.get("locomotion") == "Grounded":
                    mode = mode.removeprefix("Als.RotationMode.") if isinstance(mode, str) else mode
                    modes.add(mode)
                    if frame.get("rotation_hotkey") is True:
                        hotkeys.add(mode)
            if traversing and math.hypot(*frame["input"]) >= 0.999 and now - gait_started >= 0.5 - 1e-8:
                expected = speeds.get(gait_name)
                if expected is None or abs(frame["expected_speed"] - expected) > 1e-5:
                    issues.append(f"{prefix}: source gait speed does not match")
                    break
                if frame.get("actual_gait") != desired or frame.get("movement_state") != 0 or frame.get("movement_pose_ready") is not True or frame.get("standing_pose_ready") is not True:
                    issues.append(f"{prefix}: {desired} animation state or pose is invalid")
                    break
                speed = math.hypot(frame["velocity"][0], frame["velocity"][2])
                moved = math.hypot(frame["position"][0] - previous["position"][0], frame["position"][2] - previous["position"][2]) / delta
                requested_speed = frame["requested_speed"]
                if not 0.0 <= requested_speed <= expected + 0.1:
                    issues.append(f"{prefix}: requested speed exceeds the source gait limit")
                    break
                minimum_speed = requested_speed - 0.1
                maximum_speed = min(expected, requested_speed) + 0.1
                if frame.get("safe_move") is True:
                    prefix_speed = frame.get("safe_move_prefix_speed")
                    if not finite(prefix_speed) or not 0.0 <= prefix_speed <= requested_speed + 0.1:
                        issues.append(f"{prefix}: source SafeMove prefix evidence is missing or invalid")
                        break
                    minimum_speed = prefix_speed - 0.001
                if not finite(frame.get("displacement_speed")) or abs(frame["displacement_speed"] - moved) > 0.001 or abs(frame["animation_speed"] - speed) > 0.001:
                    issues.append(f"{prefix}: displacement, physics and animation speeds disagree")
                    break
                if not all(minimum_speed <= value <= maximum_speed for value in (speed, moved)) or (frame.get("safe_move") is True and abs(speed - moved) > 0.001):
                    issues.append(f"{prefix}: {desired} speed exceeds the source sweep bounds")
                    break
                counts[gait_name] += 1
            previous = frame
        if not distances or min(distances) > 0.15 or max(distances) < route_length - 0.15:
            issues.append(f"{prefix}: complete stair travel is missing")
        if modes != {"VelocityDirection", "ViewDirection"} or hotkeys != modes:
            issues.append(f"{prefix}: rotation hotkey recovery is missing")
        if any(count < 3 for count in counts.values()):
            issues.append(f"{prefix}: stable walk, run or sprint evidence is missing")
        if not all(f"{prefix}_{gait}" in windows for gait in ("walk", "run", "sprint")):
            issues.append(f"{prefix}: source pose windows are missing")
    return issues


def uphill_trace_issues(report):
    if report.get("case") != "uphill_jump":
        return []
    frames = report.get("manual_integration", {}).get("uphill_trace")
    if not isinstance(frames, list) or not frames or not all(isinstance(frame, dict) for frame in frames):
        return ["uphill jump frame evidence is missing"]
    windows = {window.get("label") for window in report.get("motion_windows", []) if isinstance(window, dict)}
    issues = []
    for index in range(18):
        samples = [frame for frame in frames if frame.get("case_index") == index]
        prefix = f"uphill_{index // 6 + 1:02}_{'run' if index % 6 < 3 else 'sprint'}_{index % 3}"
        if not samples:
            issues.append(f"{prefix}: frame evidence is missing")
            continue
        if prefix not in windows:
            issues.append(f"{prefix}: source pose window is missing")
        airborne = False
        launch_y = None
        landed_at = None
        landing_pose = False
        completed = False
        supported_air = 0
        previous_time = -math.inf
        for frame in samples:
            graph = frame.get("graph")
            if not finite(frame.get("time")) or frame["time"] <= previous_time or not isinstance(graph, dict) or not all(
                isinstance(frame.get(key), list) and len(frame[key]) == size and all(finite(value) for value in frame[key])
                for key, size in (("position", 3), ("velocity", 3), ("input", 2))
            ) or not isinstance(graph.get("outer_weights"), list) or len(graph["outer_weights"]) != 5 or not all(finite(value) for value in graph["outer_weights"]):
                issues.append(f"{prefix}: frame values are invalid")
                break
            previous_time = frame["time"]
            gait = "Running" if index % 6 < 3 else "Sprinting"
            requested = frame.get("requested_gait")
            requested = requested.removeprefix("Als.Gait.") if isinstance(requested, str) else requested
            if frame.get("mesh") != f"SM_Als_Stairs_{index // 6 + 1:02}" or requested != gait:
                issues.append(f"{prefix}: original stair or requested gait is invalid")
                break
            if frame.get("jump_pressed") is True:
                if not finite(frame.get("launch_y")):
                    issues.append(f"{prefix}: launch height is missing")
                    break
                launch_y = frame["launch_y"]
            air_weight = graph["outer_weights"][1] + graph["outer_weights"][2]
            airborne |= launch_y is not None and frame.get("mode") == "Air" and air_weight > 0.05
            supported_air = supported_air + 1 if airborne and frame.get("supported") is True and frame.get("mode") == "Air" and abs(frame["velocity"][1]) < 0.001 else 0
            if supported_air >= 2:
                issues.append(f"{prefix}: supported capsule remains in Air")
                break
            if airborne and frame.get("mode") == "Grounded" and frame.get("supported") is True and landed_at is None:
                landed_at = frame["time"]
                if frame["position"][1] <= launch_y + 0.05:
                    issues.append(f"{prefix}: higher supported landing is missing")
                    break
            if landed_at is not None:
                landing_pose |= graph["outer_weights"][3] + graph["outer_weights"][4] > 0.01
                if frame.get("mode") != "Grounded":
                    issues.append(f"{prefix}: landed capsule returned to Air")
                    break
                if frame["time"] - landed_at >= 0.35:
                    if air_weight > 0.01:
                        issues.append(f"{prefix}: airborne animation persists after landing")
                        break
                    completed |= landing_pose and graph.get("outer") == 0 and graph.get("outer_transition") is False and frame.get("controller_action") == "None"
        if not airborne or not completed:
            issues.append(f"{prefix}: actual air, landing and grounded recovery evidence is incomplete")
    return issues


def quaternion_product(a, b):
    x, y, z, w = a
    xx, yy, zz, ww = b
    return [w*xx+x*ww+y*zz-z*yy, w*yy-x*zz+y*ww+z*xx, w*zz+x*yy-y*xx+z*ww, w*ww-x*xx-y*yy-z*zz]


def quaternion_vector(rotation, vector):
    length = math.sqrt(sum(value*value for value in rotation))
    q = [value / length for value in rotation]
    return quaternion_product(quaternion_product(q, [*vector, 0.0]), [-q[0], -q[1], -q[2], q[3]])[:3]


def rotating_source_transform(actor, time):
    rotation = actor["actor"]["rotation"]
    for axis, rate in zip((1, 2, 0), actor["rotation_rate_yzx_radians"]):
        delta = [0.0, 0.0, 0.0, math.cos(rate * time / 2.0)]
        delta[axis] = math.sin(rate * time / 2.0)
        rotation = quaternion_product(rotation, delta)
    offset = quaternion_vector(actor["actor"]["rotation"], [value*scale for value, scale in zip(actor["primitive"]["translation"], actor["actor"]["scale"])])
    position = [value+delta for value, delta in zip(actor["actor"]["translation"], offset)]
    scale = [a*b for a, b in zip(actor["actor"]["scale"], actor["primitive"]["scale"])]
    return position, rotation, scale


def rotating_step_issues(rows):
    released, unlocked, completed = [False]*2, [False]*2, [False]*2
    release_times = []
    for side in ("Left", "Right"):
        asset = read_json(Path(__file__).resolve().parents[1] / "assets/ALS/Animations/Transitions" / f"A_Als_Stand_DynamicTransition_{side}.gltf")
        curve = next(curve for curve in asset["animations"][0]["extensions"]["ALS_float_curves"]["curves"] if curve["name"] == "AllowTransitions")
        release_times.append(max(time for time, value in zip(curve["times"], curve["values"]) if abs(value) <= 1e-6))
    issues = []
    for row in rows:
        if row.get("stage") != "Ride":
            continue
        feet, step, slot = row.get("feet"), row.get("step"), row.get("slot")
        if not isinstance(feet, list) or len(feet) != 2 or any(not isinstance(foot, dict)
            or not finite(foot.get("lock_amount")) or not 0.0 <= foot["lock_amount"] <= 1.0
            or any(not isinstance(foot.get(key), list) or len(foot[key]) != 3 or not all(finite(value) for value in foot[key]) for key in ("target", "lock")) for foot in feet):
            return ["rotating base foot evidence is invalid"]
        if step is not None and not isinstance(step, dict):
            return ["rotating base step playback evidence is invalid"]
        if step is not None and step.get("kind") == "StandingDynamic":
            if step.get("side") not in (0.0, 1.0) or any(not finite(step.get(key)) for key in ("clip_time", "duration", "play_rate", "start_time", "blend_in", "blend_out")):
                return ["rotating base step playback evidence is invalid"]
            if any(abs(step[key] - expected) > 1e-6 for key, expected in (("play_rate", 1.5), ("start_time", 0.0), ("blend_in", 0.2), ("blend_out", 0.2))) \
                or not 0.0 <= step["clip_time"] <= step["duration"] + 1e-5:
                issues.append("rotating base step playback differs from source")
            side = int(step["side"])
            ready = isinstance(slot, dict) and slot.get("active") is True and slot.get("pose_ready") is True and finite(slot.get("weight")) and slot["weight"] > 0.05
            if step.get("active") is True and ready:
                unlocked[side] |= feet[side]["lock_amount"] < 0.1
                released[side] |= unlocked[side] and step["clip_time"] >= release_times[side] - 1e-5
        for side in range(2):
            completed[side] |= released[side] and feet[side]["lock_amount"] >= 1.0-1e-5 \
                and math.dist(feet[side]["target"], feet[side]["lock"]) <= 0.08
    if not all(completed):
        issues.append("rotating base left and right step lifecycles are incomplete")
    return issues


def transition_overlap_issues(rows):
    overlap = False
    for row in rows:
        if row.get("stage") != "Ride":
            continue
        entries = row.get("transitions")
        if not isinstance(entries, list):
            return ["rotating base transition instances are missing"]
        for entry in entries:
            if not isinstance(entry, dict) or any(not finite(entry.get(key)) for key in
                ("elapsed", "clip_time", "weight", "blend_in", "stop_start_weight", "stop_duration")):
                return ["rotating base transition instance is invalid"]
            stopped = entry.get("stop_elapsed") is not None
            elapsed = entry["stop_elapsed"] if stopped else entry["elapsed"]
            if not finite(elapsed) or elapsed < 0:
                return ["rotating base transition blend time is invalid"]
            duration = entry["stop_duration"] if stopped else entry["blend_in"]
            alpha = min(1.0, max(0.0, elapsed / duration)) if duration > 0 else 1.0
            blend = alpha * alpha * (3.0 - 2.0 * alpha)
            expected = entry["stop_start_weight"] * (1.0 - blend) if stopped else blend
            if abs(entry["weight"] - expected) > 1e-4:
                return ["rotating base transition weight differs from source Hermite blend"]
        total = min(1.0, sum(entry["weight"] for entry in entries))
        slot = row.get("slot")
        if total > 1e-5 and (not isinstance(slot, dict) or slot.get("pose_ready") is not True
            or not finite(slot.get("weight")) or abs(slot["weight"] - total) > 1e-4):
            return ["rotating base slot discarded an overlapping transition contribution"]
        overlap |= (any(entry.get("stop_elapsed") is not None and entry["weight"] > 0.1 for entry in entries)
            and any(entry.get("stop_elapsed") is None and entry["weight"] > 0.1 for entry in entries))
    return [] if overlap else ["rotating base overlapping transition fade was not observed"]


def frame_hitch_issues(report, baseline, hitch):
    manual = report.get("manual_integration")
    rows = manual.get("moving_base_trace") if isinstance(manual, dict) else None
    if not isinstance(rows, list) or len(rows) < 3:
        return ["frame hitch trace is missing"]
    hits = []
    for previous, current in zip(rows, rows[1:]):
        if not isinstance(previous, dict) or not isinstance(current, dict):
            return ["frame hitch trace sample is invalid"]
        if not finite(previous.get("time")) or not finite(current.get("time")):
            return ["frame hitch trace time is invalid"]
        delta = current["time"] - previous["time"]
        if abs(delta - hitch) < 1e-4:
            hits.append((previous, current))
        elif abs(delta - baseline) >= 1e-4:
            return [f"unexpected frame delta during hitch gate: {delta:.6f}"]
    if len(hits) != 1:
        return [f"expected one frame hitch, observed {len(hits)}"]
    previous, current = hits[0]
    if current.get("stage") != "Flight" or current.get("mode") != "Air":
        return ["frame hitch did not occur during the airborne moving-base window"]
    before = previous.get("position")
    after = current.get("position")
    velocity = current.get("own_velocity")
    if (not isinstance(before, list) or not isinstance(after, list) or not isinstance(velocity, list)
            or len(before) != 3 or len(after) != 3 or len(velocity) != 3
            or not all(finite(value) for value in before + after + velocity)
            or math.dist(before, after) <= 0.01):
        return ["character did not move during the frame hitch"]
    horizontal = [after[0] - before[0], after[2] - before[2]]
    expected = [velocity[0] * hitch, velocity[2] * hitch]
    if math.dist(horizontal, expected) > 0.075:
        return ["character horizontal movement did not consume the full frame hitch"]
    return []


def moving_base_trace_issues(report):
    if report.get("case") not in ("moving_base", "rotating_base"):
        return []
    rotating = report.get("case") == "rotating_base"
    rows = report.get("manual_integration", {}).get("moving_base_trace")
    if not isinstance(rows, list) or len(rows) < 2:
        return ["moving base frame evidence is missing"]
    vectors = ("platform", "position", "local", "velocity", "own_velocity", "platform_velocity", "target_velocity")
    if rotating:
        vectors += ("motion_point",)
    for row in rows:
        if not isinstance(row, dict) or not finite(row.get("time")) or any(
            not isinstance(row.get(key), list) or len(row[key]) != 3 or not all(finite(value) for value in row[key]) for key in vectors
        ):
            return ["moving base frame data is invalid"]
    if any(second["time"] <= first["time"] for first, second in zip(rows, rows[1:])):
        return ["moving base frame time is not increasing"]
    source = read_json(Path(__file__).resolve().parents[1] / "assets/ALSExtras/Levels/L_Als_Playground" / ("rotating_actors.runtime.json" if rotating else "moving_actors.runtime.json"))
    actor = next(actor for actor in source["actors"] if actor["name"] == ("BP_SimpleRotatingObject_2" if rotating else "BP_SimpleMovingObject2"))
    period = 2.0 * math.pi / abs(actor["rotation_rate_yzx_radians"][0]) if rotating else actor["duration"] * 2.0
    span = 2.0 if rotating else math.dist(actor["start"], actor["end"])
    issues = []
    stages = {name: index for index, name in enumerate(("Place", "Settle", "Ride", "Exit", "Flight", "PrepareMantle", "WaitMantle", "Mantle", "AfterMantle", "Done"))}
    if any(row.get("stage") not in stages for row in rows) or any(stages[second["stage"]] < stages[first["stage"]] for first, second in zip(rows, rows[1:])):
        issues.append("moving base phase order differs")
    object_name = "Rotating:BP_SimpleRotatingObject_2" if rotating else "Moving:BP_SimpleMovingObject2"
    if any(row.get("object") != object_name or row.get("body") != rows[0].get("body")
           or row.get("collider") != rows[0].get("collider") or not row.get("body") or not row.get("collider") for row in rows):
        issues.append("moving base source object identity differs")
    if any(not finite(row.get("period")) or abs(row["period"] - period) > 1e-5
           or not finite(row.get("span")) or abs(row["span"] - span) > 1e-5 for row in rows):
        issues.append("moving base source period or span differs")
    if rotating:
        for row in rows:
            platform, rotation, scale = rotating_source_transform(actor, row["time"])
            angle = quaternion_distance(row.get("platform_rotation"), rotation)
            if angle is None or angle > 0.001 or math.dist(row["platform"], platform) > 0.001:
                issues.append("rotating base differs from the source world clock and transform")
                break
            offset = quaternion_vector(rotation, [0.0, 0.0, scale[2]])
            expected_point = [a+b for a,b in zip(platform, offset)]
            local = [value / size for value, size in zip(quaternion_vector([-rotation[0], -rotation[1], -rotation[2], rotation[3]], [a-b for a,b in zip(row["position"], platform)]), scale)]
            if math.dist(row["motion_point"], expected_point) > 0.001 or math.dist(row["local"], local) > 0.001:
                issues.append("rotating base coordinate evidence differs")
                break
        initial_facing = next((row.get("rotation") for row in rows if row.get("stage") == "Ride"), None)
        for row in rows:
            if row.get("stage") == "Ride" and ((angle := quaternion_distance(row.get("rotation"), initial_facing)) is None or angle > 0.001
                or row.get("input") != [0.0, 0.0] or row.get("jump_pressed") is not False):
                issues.append("rotating base changed the source world-facing idle character")
                break
        issues.extend(rotating_step_issues(rows))
        issues.extend(transition_overlap_issues(rows))
    elif any(math.dist([p - b for p, b in zip(row["position"], row["platform"])], row["local"]) > 0.001
           or (angle := quaternion_distance(row.get("platform_rotation"), [0.0, 0.0, 0.0, 1.0])) is None or angle > 1e-5 for row in rows):
        issues.append("moving base coordinate evidence differs")
    if not rotating:
        direction = [end - start for start, end in zip(actor["start"], actor["end"])]
        for row in rows:
            fraction = sum((value - start) * delta for value, start, delta in zip(row["platform"], actor["start"], direction)) / (span * span)
            if not -1e-5 <= fraction <= 1.00001 or math.dist(row["platform"], [start + delta * fraction for start, delta in zip(actor["start"], direction)]) > 0.001:
                issues.append("moving base left the source spline")
                break
    for stage in ("Ride", "AfterMantle"):
        selected = [row for row in rows if row.get("stage") == stage]
        if not selected or any(row.get("base_matches") is not True or row.get("mode") != "Grounded" for row in selected):
            issues.append(f"moving base {stage} support evidence is incomplete")
            continue
        if stage == "AfterMantle":
            last_moving = max((index for index, row in enumerate(selected) if math.hypot(row["own_velocity"][0], row["own_velocity"][2]) >= 0.01), default=-1)
            selected = selected[last_moving + 1:]
        if not selected or selected[-1]["time"] - selected[0]["time"] < period:
            issues.append(f"moving base {stage} did not cover a full source cycle")
            continue
        point = "motion_point" if rotating else "platform"
        travel = math.sqrt(sum((max(row[point][axis] for row in selected) - min(row[point][axis] for row in selected)) ** 2 for axis in range(3)))
        anchor = selected[0]["local"]
        if travel < span * 0.9 or any(math.hypot(row["local"][0] - anchor[0], row["local"][2] - anchor[2]) > 0.001
            or math.hypot(row["own_velocity"][0], row["own_velocity"][2]) > 0.01 for row in selected):
            issues.append(f"moving base {stage} transport differs")
    flight = [row for row in rows if row.get("stage") == "Flight"]
    if sum(row.get("jump_pressed") is True for row in flight) != 1 or not any(
        row.get("mode") == "Air" and row.get("has_base") is False for row in flight
    ) or not any(row.get("mode") == "Grounded" and row.get("has_base") is True and row.get("base_matches") is False for row in flight):
        issues.append("moving base jump and new support evidence is incomplete")
    mantle = [row for row in rows if row.get("stage") == "Mantle"]
    triggers = [row for row in mantle if row.get("jump_pressed") is True]
    if len(triggers) != 1 or math.dist(triggers[0]["target_velocity"], [0.0, 0.0, 0.0]) > 0.10001:
        issues.append("moving base mantle source input or speed differs")
    if not rotating and triggers and math.dist(triggers[0]["platform_velocity"], [0.0, 0.0, 0.0]) <= 1.0:
        issues.append("moving base mantle only covered an endpoint speed window")
    active = [row for row in mantle if row.get("root_motion_ready") is True and row.get("mode") == "Custom"]
    if len(active) < 2 or active[-1]["time"] - active[0]["time"] < 0.2 or max(
        (math.dist(row["motion_point" if rotating else "platform"], active[0]["motion_point" if rotating else "platform"]) for row in active), default=0.0
    ) < 0.25:
        issues.append("mantle did not follow a moving source target")
    for row in active:
        expected = row.get("expected_position")
        angle = quaternion_distance(row.get("rotation"), row.get("expected_rotation"))
        if not isinstance(expected, list) or len(expected) != 3 or not all(finite(value) for value in expected) \
            or math.dist(row["position"], expected) > 0.001 or angle is None or angle > 0.001:
            issues.append("mantle differs from the current target root transform")
            break
    if not any(row.get("mode") == "Grounded" and row.get("root_motion_ready") is False and row.get("base_matches") is True for row in mantle):
        issues.append("mantle did not hand off to the current target")
    labels = {window.get("label") for window in report.get("motion_windows", [])}
    if not {"moving_base_jump", "mantle"}.issubset(labels):
        issues.append("moving base source animation windows are missing")
    if rotating and not {"rotating_base_step_left", "rotating_base_step_right"}.issubset(labels):
        issues.append("rotating base source step windows are missing")
    return issues


def manual_integration_issues(report):
    if report.get("case") == "startup":
        return []
    value = report.get("manual_integration")
    if not isinstance(value, dict):
        return ["manual integration verdict is missing"]
    issues = []
    if not isinstance(value.get("scenario"), str) or not value.get("scenario"):
        issues.append("manual integration scenario is invalid")
    if value.get("timed_out") is not False:
        issues.append("manual integration timed_out is invalid")
    if value.get("completed") is not True:
        issues.append("manual integration did not complete")
    if value.get("passed") is not True:
        issues.append("manual integration verdict is not pass")
    results = value.get("results")
    if not isinstance(results, list) or not results:
        issues.append("manual integration results are missing")
    else:
        for result in results:
            if not isinstance(result, dict):
                issues.append("manual integration result is invalid")
                continue
            if not isinstance(result.get("name"), str) or not result.get("name"):
                issues.append("manual integration result name is invalid")
            if not isinstance(result.get("detail"), str) or not result.get("detail"):
                issues.append("manual integration result detail is invalid")
            if result.get("covered") is not True:
                issues.append("manual integration result is not covered")
    issues.extend(camera_trace_issues(report))
    issues.extend(stairs_trace_issues(report))
    issues.extend(uphill_trace_issues(report))
    issues.extend(moving_base_trace_issues(report))
    return issues


def automation_watchdog_issues(report):
    value = report.get("automation_watchdog")
    if not isinstance(value, dict):
        return ["automation watchdog record is missing"]
    issues = []
    if not isinstance(value.get("triggered"), bool):
        issues.append("automation watchdog triggered is invalid")
    if not isinstance(value.get("reason"), str):
        issues.append("automation watchdog reason is invalid")
    if not finite(value.get("stalled_seconds")) or value.get("stalled_seconds") < 0.0:
        issues.append("automation watchdog stalled seconds is invalid")
    for key in ("capture_count", "required_capture_count"):
        if not isinstance(value.get(key), int) or value.get(key) < 0:
            issues.append(f"automation watchdog {key} is invalid")
    if value.get("triggered") is True:
        issues.append("automation watchdog triggered")
    return issues


def audit(report, min_screenshots):
    issues = []
    images = []
    if not isinstance(report, dict) or report.get("format") != FORMAT:
        return {"pass": False, "issues": ["visual report format is invalid"], "images": []}
    primary_issues, primary_failure = primary_failure_issues(report)
    graph_errors = report.get("graph_errors")
    if primary_failure is not None or isinstance(graph_errors, list) and graph_errors:
        issues.extend(primary_issues)
        if primary_failure is not None:
            issues.append(primary_failure["summary"])
        return {
            "pass": False,
            "issues": list(dict.fromkeys(issues)),
            "images": [],
            "primary_failure": primary_failure,
        }
    issues.extend(primary_issues)
    if report.get("ready") is not True:
        issues.append("visual report is not ready")
    if report.get("pass") is not True:
        issues.append("runtime visual verdict is not pass")
    issues.extend(manual_integration_issues(report))
    issues.extend(automation_watchdog_issues(report))
    for key in ("faults", "graph_errors", "missing_bones"):
        values = report.get(key)
        if not isinstance(values, list):
            issues.append(f"{key} is invalid")
        elif values:
            issues.append(f"{key} is non-empty")
    issues.extend(temporal_anomaly_issues(report.get("temporal_anomalies")))
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
        for key in (
            "temporal_global_position_speed_mps",
            "temporal_local_translation_jump_m",
            "temporal_local_rotation_jump_radians",
            "temporal_large_angular_change_rate_radps",
            "temporal_scale_ratio",
            "temporal_segment_ratio",
            "temporal_limb_plane_flip_dot",
        ):
            if not finite(thresholds.get(key)):
                issues.append(f"temporal threshold is invalid: {key}")
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
        if min_screenshots > 0 and missing_screenshots:
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
        if min_screenshots > 0 and not teleport_screenshots:
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
    issues.extend(motion_window_issues(report, screenshots, pose_samples, min_screenshots > 0))
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
        "required_motion_windows": [],
        "temporal_anomalies": [],
        "faults": [],
        "graph_errors": [],
        "primary_failure": None,
        "manual_integration": {
            "scenario": "Fixture",
            "timed_out": False,
            "completed": True,
            "passed": True,
            "results": [{"name": "fixture", "covered": True, "detail": "covered"}],
        },
        "automation_watchdog": {
            "triggered": False,
            "reason": "",
            "stalled_seconds": 0.0,
            "capture_count": 2,
            "required_capture_count": 2,
        },
        "thresholds": {
            "max_frame_p95_ms": 25.0,
            "max_frame_ms": 75.0,
            "max_eval_p95_ms": 4.0,
            "max_physics_steps_per_render": 3.0,
            "temporal_global_position_speed_mps": 32.0,
            "temporal_local_translation_jump_m": 0.35,
            "temporal_local_rotation_jump_radians": 2.2,
            "temporal_large_angular_change_rate_radps": 30.0,
            "temporal_local_rotation_speed_radps": 64.0,
            "temporal_scale_ratio": 1.5,
            "temporal_segment_ratio": 0.35,
            "temporal_limb_plane_flip_dot": -0.5,
        },
    }


def fixture_graph_failure():
    return {
        "category": "graph_evaluation_error",
        "raw_error": "MissingEdgeToSource(NodeTime(NodeId(17150000-0000-4000-8000-000000000001)))",
        "error_kind": "missing_edge_to_source",
        "pin": {
            "kind": "node_time",
            "node_id": "NodeId(17150000-0000-4000-8000-000000000001)",
            "pin_id": None,
            "graph_context": None,
        },
        "node_candidates": [
            {
                "graph_path": "graphs/fixture/runtime.animgraph.ron",
                "graph_asset_id": "AssetId(fixture_runtime)",
                "graph_context": "GraphContextId(1)",
                "node_id": "NodeId(17150000-0000-4000-8000-000000000001)",
                "node_name": "fixture runtime",
                "node_kind": "Interruptible FSM",
                "active_context": True,
                "context_curves": {"PoseStanding": 1.0, "PoseCrouching": 0.0},
            }
        ],
        "active_context_nodes": [],
        "active_node_candidate_count": 1,
        "candidate_resolution": "resolved_active_context",
        "root_graph_path": "graphs/fixture/root.animgraph.ron",
        "root_graph_recursive_dependencies": "Loaded",
        "root_pose": {"output_present": False, "bone_count": 0, "active_channel_count": 0},
        "pose_classification": "missing_root_pose_likely_reference_or_a_pose",
        "case": "fixture_graph_error",
        "direction_case": None,
        "phase": None,
        "phase_elapsed_secs": None,
        "locomotion": None,
        "gait": None,
        "stance": None,
        "rotation_mode": None,
        "screenshot_status": "unavailable_no_camera",
        "screenshot_path": None,
        "reported_before_ready": True,
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
        "expected_parent_runtime_state": None,
        "expected_terminal_parent_runtime_state": None,
        "expected_terminal_parent_transition_active": None,
        "expected_terminal_requested_parent_transition_active": None,
        "observed_parent_runtime_states": [],
        "observed_parent_transition_active": [],
        "observed_requested_parent_runtime_states": [],
        "observed_requested_parent_transition_active": [],
        "observed_terminal_parent_runtime_states": [],
        "observed_terminal_parent_transition_active": [],
        "observed_terminal_requested_parent_runtime_states": [],
        "observed_terminal_requested_parent_transition_active": [],
        "sample_count": 2,
        "requires_pre_rig_motion": True,
        "requires_final_pose_motion": True,
        "requires_non_reference_root_pose": True,
        "requires_source_pose_chain": False,
        "requires_actor_yaw_motion": False,
        "requires_actor_yaw_convergence": False,
        "expected_actor_yaw_delta_sign": None,
        "expected_actor_yaw_baseline": None,
        "expected_animation_variant": None,
        "minimum_pre_rig_translation_span": 0.0,
        "minimum_pre_rig_rotation_span_radians": 0.0,
        "minimum_final_translation_span": 0.0,
        "minimum_final_rotation_span_radians": 0.0,
        "minimum_actor_yaw_delta_radians": 0.0,
        "minimum_actor_yaw_convergence_radians": 0.0,
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
        "root_pose_max_translation_span": 0.01,
        "root_pose_max_rotation_span_radians": 0.01,
        "actor_yaw_sample_count": 2,
        "actor_yaw_trace": [],
        "max_actor_yaw_delta_radians": 0.0,
        "max_actor_yaw_signed_delta_radians": 0.0,
        "max_actor_yaw_opposite_delta_radians": 0.0,
        "max_actor_yaw_convergence_radians": 0.0,
        "source_sync_group": "Movement" if is_group_member else "",
        "sync_phase_global": "marker_phase" if is_group_member else "",
        "canonical_sync_phase": 0.5 if is_group_member else None,
        "maximum_sync_phase_distance": 0.1 if is_group_member else 0.0,
        "canonical_sync_phase_distance": 0.01 if is_group_member else None,
        "direction_signature_group": group or "",
        "required_direction_signature_members": 6 if is_group_member else 0,
        "minimum_direction_signature_rotation_radians": 0.01 if is_group_member else 0.0,
        "canonical_pre_rig_pose": fixture_canonical_pose(rotation_radians) if is_group_member else {},
        "source_pose_chain": [],
        "expected_lifecycle_stages": [],
        "observed_lifecycle_stages": [],
        "first_failed_source_pose_boundary": None,
    }


def fixture_source_pose_boundary(label):
    return {
        "label": label,
        "graph": "graphs/fixture.animgraph.ron",
        "node": f"{label} node",
        "output": "Pose",
        "minimum_translation_span": 0.0,
        "minimum_rotation_span_radians": 0.01,
        "sample_count": 2,
        "available_samples": 2,
        "unavailable_samples": 0,
        "max_translation_span": 0.0,
        "max_rotation_span_radians": 0.02,
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


def fixture_stairs_report():
    frames = []
    windows = []
    for route in range(6):
        now = route * 20.0
        distance = 0.0
        base = {
            "route_index": route, "mesh": f"SM_Als_Stairs_{route // 2 + 1:02}",
            "stage": "Ready", "elapsed_seconds": now, "simulation_delta_seconds": 0.1,
            "distance_m": 0.0, "route_length_m": 17.3, "input": [0.0, 0.0],
            "rotation_hotkey": False, "actual_gait": "Walking", "desired_gait": "Walking",
            "position": [0.0, 0.9, 0.0], "velocity": [0.0, 0.0, 0.0],
            "locomotion": "Grounded", "desired_mode": "ViewDirection", "current_mode": "ViewDirection",
            "expected_speed": 1.75, "requested_speed": 1.75, "animation_speed": 0.0, "safe_move": False,
            "displacement_speed": None, "movement_state": 0, "movement_pose_ready": True, "standing_pose_ready": True,
        }
        frames.append(base.copy())
        for gait, speed, count in (("Walking", 1.75, 22), ("Running", 3.75, 12), ("Sprinting", 6.5, 10), ("Walking", 1.75, 14)):
            for _ in range(count):
                now += 0.1
                distance += speed * 0.1
                frames.append({**base, "stage": "Traverse", "elapsed_seconds": now, "distance_m": distance,
                    "input": [0.0, 1.0], "actual_gait": gait, "desired_gait": gait, "expected_speed": speed, "requested_speed": speed,
                    "position": [0.0, 0.9, -distance], "velocity": [0.0, 0.0, -speed],
                    "animation_speed": speed, "displacement_speed": speed})
        for mode in ("VelocityDirection", "ViewDirection"):
            now += 0.1
            frames.append({**base, "stage": "VelocityMode" if mode == "VelocityDirection" else "ViewMode",
                "elapsed_seconds": now, "distance_m": distance, "position": [0.0, 0.9, -distance],
                "desired_mode": mode, "current_mode": mode, "rotation_hotkey": True})
        prefix = f"stairs_{route // 2 + 1:02}_{'up' if route % 2 == 0 else 'down'}"
        windows.extend({"label": f"{prefix}_{gait}"} for gait in ("walk", "run", "sprint"))
    return {"case": "stairs", "manual_integration": {"scenario": "Stairs", "timed_out": False,
        "completed": True, "passed": True, "results": [{"name": "stairs_progress", "covered": True, "detail": "covered"}],
        "stairs_trace": frames}, "motion_windows": windows}


def fixture_uphill_report():
    frames = []
    windows = []
    for index in range(18):
        gait = "Running" if index % 6 < 3 else "Sprinting"
        prefix = f"uphill_{index // 6 + 1:02}_{'run' if index % 6 < 3 else 'sprint'}_{index % 3}"
        windows.append({"label": prefix})
        for time, mode, state, height, jumped, supported in ((0.0,"Grounded",0,0.92,False,True),
            (0.2,"Air",2,1.1,True,False),(0.3,"Air",1,1.5,False,False),
            (0.4,"Grounded",4,1.6,False,True),(0.8,"Grounded",0,1.6,False,True)):
            weights = [0.0] * 5
            weights[state] = 1.0
            frames.append({"case_index":index,"mesh":f"SM_Als_Stairs_{index // 6 + 1:02}","requested_gait":gait,
                "time":index * 2.0 + time,"position":[0.0,height,-time],"velocity":[0.0,0.0 if supported else 1.0,0.0],
                "input":[0.0,1.0],"jump_pressed":jumped,"launch_y":0.92,"supported":supported,"mode":mode,
                "controller_action":"None" if supported else "Some(Jump)",
                "graph":{"outer":state,"outer_transition":False,"outer_weights":weights}})
    return {"case":"uphill_jump","manual_integration":{"scenario":"UphillJump","timed_out":False,"completed":True,
        "passed":True,"results":[{"name":"uphill_landing_consistency","covered":True,"detail":"covered"}],"uphill_trace":frames},
        "motion_windows":windows}


def fixture_moving_base_report():
    rows = []
    def add(stage, time, distance, **changes):
        platform = [19.0 - distance, 0.0, -28.75]
        position = [platform[0], 1.924, platform[2] + 0.7]
        row = dict(stage=stage, time=time, body="body", collider="collider", object="Moving:BP_SimpleMovingObject2",
            period=3.5, span=6.5, platform=platform, platform_rotation=[0.0, 0.0, 0.0, 1.0], position=position,
            rotation=[0.0, 0.0, 0.0, 1.0], velocity=[0.0, 0.0, 0.0], own_velocity=[0.0, 0.0, 0.0],
            platform_velocity=[4.0, 0.0, 0.0], target_velocity=[0.0, 0.0, 0.0], mode="Grounded", base_matches=True, has_base=True,
            jump_pressed=False, root_motion_ready=False, expected_position=position[:], expected_rotation=[0.0, 0.0, 0.0, 1.0])
        row.update(changes)
        row["local"] = [value - base for value, base in zip(row["position"], row["platform"])]
        rows.append(row)
    for time, distance in enumerate([0.0, 3.25, 6.5, 3.25, 0.0]):
        add("Ride", time, distance)
    add("Flight", 5.0, 0.2, jump_pressed=True, mode="Air", has_base=False, base_matches=False)
    add("Flight", 5.5, 0.5, mode="Air", has_base=False, base_matches=False)
    add("Flight", 6.0, 1.0, has_base=True, base_matches=False)
    add("Mantle", 7.0, 1.1, jump_pressed=True, root_motion_ready=True, mode="Custom")
    add("Mantle", 7.3, 1.5, root_motion_ready=True, mode="Custom")
    add("Mantle", 7.8, 2.0, root_motion_ready=True, mode="Custom")
    add("Mantle", 8.0, 2.2)
    for time, distance in enumerate([2.2, 0.0, 4.0, 6.5, 2.2], 9):
        add("AfterMantle", time, distance)
    return {"case":"moving_base", "manual_integration":{"scenario":"MovingBase", "timed_out":False,
        "completed":True, "passed":True, "results":[{"name":"moving_base_progress","covered":True,"detail":"covered"}],
        "moving_base_trace":rows}, "motion_windows":[{"label":"moving_base_jump"},{"label":"mantle"}]}


def fixture_rotating_base_report():
    report = fixture_moving_base_report()
    report["case"] = "rotating_base"
    report["manual_integration"]["scenario"] = "RotatingBase"
    report["motion_windows"].extend([{"label":"rotating_base_step_left"}, {"label":"rotating_base_step_right"}])
    for index, row in enumerate(report["manual_integration"]["moving_base_trace"]):
        angle = -math.radians(100.0) * row["time"]
        sine, cosine = math.sin(angle), math.cos(angle)
        row.update(object="Rotating:BP_SimpleRotatingObject_2", period=3.6, span=2.0,
            platform=[13.5, -0.5, -24.0], platform_rotation=[0.0, math.sin(angle/2), 0.0, math.cos(angle/2)],
            platform_velocity=[0.0, 0.0, 0.0], input=[0.0, 0.0], local=[0.65, 2.424, 0.0],
            motion_point=[13.5+sine, -0.5, -24.0+cosine], position=[13.5+0.65*cosine, 1.924, -24.0-0.65*sine],
            feet=[{"lock_amount":1.0, "target":[0.0, 0.0, 0.0], "lock":[0.0, 0.0, 0.0]} for _ in range(2)], step=None, slot=None, transitions=[])
        if row["stage"] == "Ride":
            row["rotation"] = [0.0, math.sin(-math.radians(4.72)/2), 0.0, math.cos(-math.radians(4.72)/2)]
        row["expected_position"] = row["position"][:]
        if row["stage"] == "Mantle":
            row["rotation"] = row["platform_rotation"][:]
            row["expected_rotation"] = row["rotation"][:]
        if index < 4:
            side, beginning = index // 2, index % 2 == 0
            row["step"] = dict(kind="StandingDynamic", side=float(side), active=True, clip_time=0.4 if beginning else 0.55,
                duration=1.533333, play_rate=1.5, start_time=0.0, blend_in=0.2, blend_out=0.2)
            row["slot"] = dict(active=True, pose_ready=True, weight=1.0)
            row["transitions"] = [dict(elapsed=0.3, clip_time=0.45, weight=1.0, blend_in=0.2,
                stop_elapsed=None, stop_start_weight=0.0, stop_duration=0.0)]
            if index == 2:
                row["transitions"].insert(0, dict(elapsed=0.5, clip_time=0.75, weight=0.5,
                    blend_in=0.2, stop_elapsed=0.1, stop_start_weight=1.0, stop_duration=0.2))
            row["feet"][side]["lock_amount"] = 0.0 if beginning else 1.0
    return report


def run_self_test():
    baseline = 1.0 / 60.0
    hitch = 0.2
    rows = [
        {"time": 0.0, "stage": "Flight", "mode": "Air", "position": [0.0, 0.0, 0.0]},
        {"time": baseline, "stage": "Flight", "mode": "Air", "position": [0.1, 0.0, 0.0]},
        {"time": baseline + hitch, "stage": "Flight", "mode": "Air", "position": [0.5, 0.0, 0.0], "own_velocity": [2.0, 0.0, 0.0]},
        {"time": 2.0 * baseline + hitch, "stage": "Flight", "mode": "Air", "position": [0.52, 0.0, 0.0]},
    ]
    frame_report = {"manual_integration": {"moving_base_trace": rows}}
    assert not frame_hitch_issues(frame_report, baseline, hitch)
    missing = copy.deepcopy(frame_report)
    missing["manual_integration"]["moving_base_trace"][2]["time"] = 2.0 * baseline
    missing["manual_integration"]["moving_base_trace"][3]["time"] = 3.0 * baseline
    assert frame_hitch_issues(missing, baseline, hitch)
    duplicate = copy.deepcopy(frame_report)
    duplicate["manual_integration"]["moving_base_trace"].append({"time": 2.0 * baseline + 2.0 * hitch, "stage": "Flight", "mode": "Air", "position": [0.9, 0.0, 0.0]})
    assert frame_hitch_issues(duplicate, baseline, hitch)
    stationary = copy.deepcopy(frame_report)
    stationary["manual_integration"]["moving_base_trace"][2]["position"] = [0.1, 0.0, 0.0]
    assert frame_hitch_issues(stationary, baseline, hitch)
    capped = copy.deepcopy(frame_report)
    capped["manual_integration"]["moving_base_trace"][2]["position"] = [0.17, 0.0, 0.0]
    assert frame_hitch_issues(capped, baseline, hitch)
    grounded = copy.deepcopy(frame_report)
    grounded["manual_integration"]["moving_base_trace"][2]["mode"] = "Grounded"
    assert frame_hitch_issues(grounded, baseline, hitch)
    half = math.sqrt(0.5)
    transform_fixture = {"actor":{"translation":[10.0, 20.0, 30.0], "rotation":[0.0, half, 0.0, half], "scale":[2.0]*3},
        "primitive":{"translation":[1.0, 0.0, 0.0], "scale":[0.5]*3}, "rotation_rate_yzx_radians":[-math.pi/2, 0.0, 0.0]}
    position, rotation, scale = rotating_source_transform(transform_fixture, 1.0)
    assert math.dist(position, [10.0, 20.0, 28.0]) < 1e-10 and quaternion_distance(rotation, [0.0, 0.0, 0.0, 1.0]) < 1e-7 and scale == [1.0]*3
    transform_fixture["actor"]["rotation"] = [0.0, 0.0, 0.0, 1.0]
    transform_fixture["rotation_rate_yzx_radians"] = [math.pi/2]*3
    rotation = rotating_source_transform(transform_fixture, 1.0)[1]
    for axis, expected in zip(([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0])):
        assert math.dist(quaternion_vector(rotation, axis), expected) < 1e-10
    rotating = fixture_rotating_base_report()
    assert not manual_integration_issues(rotating), manual_integration_issues(rotating)
    for issue in ("missing", "discarded", "weight", "no_overlap"):
        invalid = copy.deepcopy(rotating["manual_integration"]["moving_base_trace"])
        if issue == "missing":
            invalid[0].pop("transitions")
        elif issue == "discarded":
            invalid[2]["slot"]["weight"] = 0.5
        elif issue == "weight":
            invalid[2]["transitions"][0]["weight"] = 0.8
        else:
            invalid[2]["transitions"].pop(0)
        assert transition_overlap_issues(invalid), issue
    for issue in ("missing", "identity", "frozen", "yaw_direction", "clock", "pivot", "coordinates", "motion_point",
                  "facing", "input", "own_velocity", "missing_step", "missing_feet", "invalid_step", "pose", "unlock", "relock", "release", "release_pose", "source_rate",
                  "frozen_mantle", "mantle_rotation", "no_handoff", "step_window", "cycle", "both_sides", "duration", "release_inactive", "lock_amount"):
        invalid = copy.deepcopy(rotating)
        rows = invalid["manual_integration"]["moving_base_trace"]
        if issue == "missing":
            invalid["manual_integration"].pop("moving_base_trace")
        elif issue == "identity":
            rows[0]["object"] = "replacement"
        elif issue == "frozen":
            for row in rows[:5]:
                row["platform_rotation"] = rows[0]["platform_rotation"][:]
                row["motion_point"] = rows[0]["motion_point"][:]
        elif issue == "yaw_direction":
            rows[1]["platform_rotation"][1] *= -1
        elif issue == "clock":
            rows[1]["time"] += 0.05
        elif issue == "pivot":
            rows[0]["platform"][1] += 0.1
        elif issue == "coordinates":
            rows[1]["local"][0] += 0.1
        elif issue == "motion_point":
            rows[1]["motion_point"] = rows[1]["platform"][:]
        elif issue == "facing":
            rows[1]["rotation"] = rows[1]["platform_rotation"][:]
        elif issue == "input":
            rows[1]["input"] = [1.0, 0.0]
        elif issue == "own_velocity":
            rows[1]["own_velocity"][0] = 0.5
        elif issue == "missing_step":
            rows[0]["step"] = None
        elif issue == "missing_feet":
            rows[0]["feet"] = None
        elif issue == "invalid_step":
            rows[0]["step"] = []
        elif issue == "pose":
            rows[0]["slot"]["pose_ready"] = False
        elif issue == "unlock":
            rows[0]["feet"][0]["lock_amount"] = 1.0
        elif issue == "relock":
            for row in rows[1:5]:
                row["feet"][0]["lock"][0] = 0.2
        elif issue == "release":
            rows[1]["step"]["clip_time"] = 0.5
        elif issue == "release_pose":
            rows[1]["slot"]["pose_ready"] = False
        elif issue == "source_rate":
            rows[0]["step"]["play_rate"] = 1.0
        elif issue == "frozen_mantle":
            rows[9]["position"] = rows[8]["position"][:]
            angle = -math.radians(100.0) * rows[9]["time"]
            delta = [a-b for a,b in zip(rows[9]["position"], rows[9]["platform"])]
            rows[9]["local"] = [math.cos(angle)*delta[0]-math.sin(angle)*delta[2], delta[1], math.sin(angle)*delta[0]+math.cos(angle)*delta[2]]
        elif issue == "mantle_rotation":
            rows[9]["rotation"] = rows[8]["rotation"][:]
        elif issue == "no_handoff":
            rows[11]["base_matches"] = False
        elif issue == "step_window":
            invalid["motion_windows"].pop()
        elif issue == "cycle":
            rows.pop()
        elif issue == "both_sides":
            rows[2]["step"]["side"] = rows[3]["step"]["side"] = 0.0
        elif issue == "release_inactive":
            rows[1]["step"]["active"] = False
        elif issue == "lock_amount":
            for row in rows[1:5]:
                row["feet"][0]["lock_amount"] = 0.25
        else:
            rows[0]["step"]["duration"] = 0.0
        assert manual_integration_issues(invalid), issue
    moving = fixture_moving_base_report()
    assert not manual_integration_issues(moving), manual_integration_issues(moving)
    for issue in ("missing", "identity", "stationary", "drift", "support", "no_jump", "no_new_base", "speed",
                  "frozen_actor", "missing_expected", "rotation", "no_handoff", "duplicate_input", "short_cycle", "window", "source_path", "phase_order", "source_period", "invalid_rotation", "endpoint_speed", "missing_target_velocity"):
        invalid = copy.deepcopy(moving)
        rows = invalid["manual_integration"]["moving_base_trace"]
        if issue == "missing":
            invalid["manual_integration"].pop("moving_base_trace")
        elif issue == "identity":
            rows[0]["object"] = "replacement"
        elif issue == "stationary":
            for row in rows[:5]:
                row["platform"] = rows[0]["platform"][:]
                row["position"] = rows[0]["position"][:]
        elif issue == "drift":
            rows[1]["position"][0] += 0.1
            rows[1]["local"][0] += 0.1
        elif issue == "support":
            rows[1]["base_matches"] = False
        elif issue == "no_jump":
            rows[5]["jump_pressed"] = False
        elif issue == "no_new_base":
            rows[7]["base_matches"] = True
        elif issue == "speed":
            rows[8]["target_velocity"] = [1.0, 0.0, 0.0]
        elif issue == "frozen_actor":
            rows[9]["position"] = rows[8]["position"][:]
            rows[9]["local"] = [p-b for p,b in zip(rows[9]["position"], rows[9]["platform"])]
        elif issue == "missing_expected":
            rows[9]["expected_position"] = None
        elif issue == "rotation":
            rows[9]["rotation"] = [0.0, 1.0, 0.0, 0.0]
        elif issue == "no_handoff":
            rows[11]["base_matches"] = False
        elif issue == "duplicate_input":
            rows[9]["jump_pressed"] = True
        elif issue == "short_cycle":
            rows.pop()
        elif issue == "window":
            invalid["motion_windows"].pop()
        elif issue == "source_path":
            rows[0]["platform"][2] += 10.0
            rows[0]["position"][2] += 10.0
        elif issue == "phase_order":
            rows[3]["stage"] = "AfterMantle"
        elif issue == "source_period":
            rows[0]["period"] = 0.1
        elif issue == "endpoint_speed":
            rows[8]["platform_velocity"] = [0.09, 0.0, 0.0]
        elif issue == "missing_target_velocity":
            rows[8].pop("target_velocity")
        else:
            rows[0]["platform_rotation"] = None
        assert manual_integration_issues(invalid), issue
    uphill = fixture_uphill_report()
    assert not manual_integration_issues(uphill), manual_integration_issues(uphill)
    delayed_support = copy.deepcopy(uphill)
    early_grounded = copy.deepcopy(delayed_support["manual_integration"]["uphill_trace"][3])
    early_grounded.update(time=0.38,supported=False)
    delayed_support["manual_integration"]["uphill_trace"].insert(3,early_grounded)
    assert not uphill_trace_issues(delayed_support), uphill_trace_issues(delayed_support)
    for issue in ("supported_air", "fall_animation", "lower_landing", "missing_case", "missing_window", "action_stuck", "no_jump_input"):
        invalid = copy.deepcopy(uphill)
        frames = invalid["manual_integration"]["uphill_trace"]
        if issue == "supported_air":
            for frame in frames[1:3]:
                frame.update(supported=True,velocity=[0.0,0.0,0.0])
        elif issue == "fall_animation":
            frames[4]["graph"].update(outer=1,outer_weights=[0.0,1.0,0.0,0.0,0.0])
        elif issue == "lower_landing":
            frames[3]["position"][1] = 0.9
        elif issue == "missing_case":
            frames[:] = [frame for frame in frames if frame["case_index"] != 17]
        elif issue == "missing_window":
            invalid["motion_windows"].pop()
        elif issue == "action_stuck":
            frames[4]["controller_action"] = "Some(Jump)"
        else:
            frames[1]["jump_pressed"] = False
        assert manual_integration_issues(invalid), issue
    stairs = fixture_stairs_report()
    assert not stairs_trace_issues(stairs), stairs_trace_issues(stairs)
    tagged_stairs = copy.deepcopy(stairs)
    for frame in tagged_stairs["manual_integration"]["stairs_trace"]:
        for key in ("desired_gait", "actual_gait"):
            frame[key] = "Als.Gait." + frame[key]
        for key in ("desired_mode", "current_mode"):
            frame[key] = "Als.RotationMode." + frame[key]
    assert not stairs_trace_issues(tagged_stairs), stairs_trace_issues(tagged_stairs)
    tagged_uphill = fixture_uphill_report()
    for frame in tagged_uphill["manual_integration"]["uphill_trace"]:
        frame["requested_gait"] = "Als.Gait." + frame["requested_gait"]
    assert not uphill_trace_issues(tagged_uphill), uphill_trace_issues(tagged_uphill)
    assert not manual_integration_issues(stairs), manual_integration_issues(stairs)
    for speed in (1.42, 1.40):
        clipped = copy.deepcopy(stairs)
        trace = clipped["manual_integration"]["stairs_trace"]
        frame = trace[8]
        frame.update(safe_move=True, safe_move_prefix_speed=1.42, velocity=[0.0, 0.0, -speed], animation_speed=speed, displacement_speed=speed)
        for later in trace[8:]:
            if later["route_index"] == 0:
                later["position"][2] += (1.75 - speed) * 0.1
                later["distance_m"] -= (1.75 - speed) * 0.1
        assert bool(stairs_trace_issues(clipped)) == (speed < 1.42), stairs_trace_issues(clipped)
        if speed == 1.42:
            frame.pop("safe_move_prefix_speed")
            assert stairs_trace_issues(clipped)
    for key, value in (("animation_speed", 0.0), ("movement_pose_ready", False),
        ("locomotion", "Air"), ("position", [0.0, 0.9, -100.0]), ("expected_speed", 100.0),
        ("requested_speed", 0.5), ("velocity", [0.0, 0.0, 0.0])):
        invalid = copy.deepcopy(stairs)
        invalid["manual_integration"]["stairs_trace"][8][key] = value
        assert stairs_trace_issues(invalid), key
        assert manual_integration_issues(invalid), key
    for missing in ("route", "sprint", "hotkey", "window", "frames"):
        invalid = copy.deepcopy(stairs)
        trace = invalid["manual_integration"]["stairs_trace"]
        if missing == "route":
            trace[:] = [frame for frame in trace if frame["route_index"] != 5]
        elif missing == "sprint":
            trace[:] = [frame for frame in trace if frame["desired_gait"] != "Sprinting"]
        elif missing == "hotkey":
            for frame in trace:
                frame["rotation_hotkey"] = False
        elif missing == "window":
            invalid["motion_windows"].pop()
        else:
            trace.clear()
        assert stairs_trace_issues(invalid), missing
    camera = {"valid": True, "trace_blocking_hit": True, "trace_adjustment_resolved": True,
        "trace_adjustment_count": 2, "trace_distance_ratio": 0.4, "trace_radius": 0.15,
        "first_person_override": 0.0, "unscaled_delta_seconds": 0.016,
        "trace_start": [0.0, 1.6, 0.0], "trace_end": [0.0, 1.6, 2.0], "curve_values": [0.0] * 13}
    collision = {"phase_elapsed_seconds": 0.5, "obstacles_present": True, "obstacles_ignored": False,
        "first_person_requested": False, "actual_first_person": False, "blocked_ratio": None, "camera": camera}
    recovered = {**collision, "phase_elapsed_seconds": 0.7, "obstacles_present": True, "obstacles_ignored": True,
        "blocked_ratio": 0.4, "camera": {**camera, "trace_distance_ratio": 0.7, "trace_blocking_hit": False}}
    camera_report = {"case": "camera", "manual_integration": {"camera_trace": [collision, recovered]}}
    ragdoll_baseline = {**collision, "ragdoll_active": False,
        "camera": {**camera, "trace_distance_ratio": 1.0, "trace_blocking_hit": False}}
    ragdoll_active = {**ragdoll_baseline, "phase_elapsed_seconds": 0.6, "ragdoll_active": True,
        "camera": dict(ragdoll_baseline["camera"])}
    ragdoll_report = {"case": "ragdoll", "manual_integration": {"camera_trace": [ragdoll_baseline, ragdoll_active, ragdoll_active]}}
    assert not camera_trace_issues(ragdoll_report)
    ragdoll_active["camera"]["trace_distance_ratio"] = 0.09
    assert camera_trace_issues(ragdoll_report)
    ragdoll_active["camera"]["trace_distance_ratio"] = 1.0
    ragdoll_active["actual_first_person"] = True
    assert camera_trace_issues(ragdoll_report)
    ragdoll_active["actual_first_person"] = False
    assert camera_trace_issues({"case": "ragdoll", "manual_integration": {"camera_trace": [ragdoll_active, ragdoll_active]}})
    assert camera_trace_issues({"case": "ragdoll", "manual_integration": {}})
    assert not camera_trace_issues(camera_report)
    recovered["obstacles_present"] = False
    assert camera_trace_issues(camera_report)
    recovered["obstacles_present"] = True
    recovered["camera"]["first_person_override"] = 1.0
    assert camera_trace_issues(camera_report)
    recovered["camera"]["first_person_override"] = 0.0
    recovered["camera"]["trace_end"][1] = float("nan")
    assert camera_trace_issues(camera_report)
    assert camera_trace_issues({"case": "camera", "manual_integration": {}})
    clip_windows = [
        fixture_motion_window(label, rotation, "Slot variants")
        for label, rotation in [("front", 0.0), ("back", 1.0)]
    ]
    for window in clip_windows:
        window.update(
            expected_runtime_state=None,
            observed_runtime_states=[],
            source_sync_group="",
            sync_phase_global="",
            canonical_sync_phase=None,
            canonical_sync_phase_distance=None,
            maximum_sync_phase_distance=0.0,
            canonical_clip_time=0.6,
            maximum_clip_time_distance=0.1,
            canonical_clip_time_distance=0.02,
            required_direction_signature_members=2,
        )
    assert not direction_signature_issues(clip_windows)
    clip_windows[1]["canonical_pre_rig_pose"] = clip_windows[0]["canonical_pre_rig_pose"]
    assert any("not distinct" in issue for issue in direction_signature_issues(clip_windows))
    clip_windows[1]["canonical_pre_rig_pose"] = fixture_canonical_pose(1.0)
    clip_windows[1]["canonical_clip_time_distance"] = 0.2
    assert any("outside tolerance" in issue for issue in direction_signature_issues(clip_windows))
    clip_windows[1]["canonical_clip_time_distance"] = 0.02
    clip_windows[1]["observed_directions"] = clip_windows[0]["observed_directions"]
    assert any("does not match" in issue for issue in direction_signature_issues(clip_windows))
    clip = fixture_motion_window("mantle")
    clip.update(expected_runtime_state=None, observed_runtime_states=[], source_sync_group="",
                sync_phase_global="", canonical_sync_phase=None, canonical_sync_phase_distance=None,
                canonical_clip_time=0.6, maximum_clip_time_distance=0.1, canonical_clip_time_distance=0.02,
                expected_animation_variant="FixtureClip", canonical_pre_rig_pose=fixture_canonical_pose())
    clip_report = {"motion_windows": [clip], "required_motion_windows": ["mantle"]}
    clip_issues = lambda: motion_window_issues(clip_report, [], [{"direction_case": "mantle"}] * 2, False)
    assert any("animation clip trace is missing" in issue for issue in clip_issues())
    clip["actor_yaw_trace"] = [dict(elapsed_secs=time, actor_yaw_radians=None, animation_clip_time=time,
                                   animation_weight=1.0, animation_play_rate=1.25, animation_variant="FixtureClip")
                              for time in (0.4, 0.6)]
    assert not clip_issues(), clip_issues()
    for field, value in (("animation_clip_time", None), ("animation_weight", -1.0), ("animation_play_rate", float("nan")),
                         ("animation_variant", "OtherClip"), ("elapsed_secs", 0.4)):
        saved = clip["actor_yaw_trace"][1][field]
        clip["actor_yaw_trace"][1][field] = value
        assert clip_issues(), field
        clip["actor_yaw_trace"][1][field] = saved
    for sample in clip["actor_yaw_trace"]:
        sample["animation_weight"] = 0.0
    assert any("insufficient active samples" in issue for issue in clip_issues())
    for sample in clip["actor_yaw_trace"]:
        sample["animation_weight"] = 1.0
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
        deferred = fixture_report(paths)
        deferred["animation_eval_time"]["p95_ms"] = deferred["thresholds"]["max_eval_p95_ms"] + 1.0
        deferred["pass"] = False
        assert not audit(deferred, 2)["pass"]
        result = audit_functional(deferred, 2)
        assert result["pass"] and not result["full_pass"]
        assert result["deferred_performance_issues"] == ["performance budget exceeded: max_eval_p95_ms"]
        assert deferred["pass"] is False
        deferred["faults"] = ["functional failure"]
        assert not audit_functional(deferred, 2)["pass"]
        deferred["faults"] = []
        deferred["physics_steps_per_render"]["max"] = deferred["thresholds"]["max_physics_steps_per_render"] + 1
        assert not audit_functional(deferred, 2)["pass"]
        deferred = fixture_report(paths)
        deferred["pass"] = False
        assert not audit_functional(deferred, 2)["pass"]
        deferred["animation_eval_time"]["p95_ms"] = None
        assert not audit_functional(deferred, 2)["pass"]
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
        for sample in report["pose_samples"]:
            sample["root_pose"]["active_channel_count"] = 0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        for sample in report["pose_samples"]:
            sample["root_pose"]["output_present"] = False
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["faults"] = ["fault"]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["manual_integration"]["results"][0]["covered"] = False
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["manual_integration"]["results"][0].pop("detail")
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report.pop("automation_watchdog")
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["automation_watchdog"]["triggered"] = True
        report["automation_watchdog"]["reason"] = "capture_progress_stalled"
        report["automation_watchdog"]["stalled_seconds"] = 12.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["temporal_anomalies"] = [
            {
                "stage": "final",
                "kind": "limb_plane_flip",
                "subject": "left_leg",
                "previous": 1.0,
                "current": -1.0,
                "threshold": -0.5,
                "trigger": "plane_flip",
                "rate": None,
                "rate_threshold": None,
                "elapsed_secs": 1.0,
                "frame_ms": 16.0,
                "direction_case": "crouch_left_forward",
                "phase": "CrouchDirectionRegression",
            }
        ]
        assert not audit(report, 2)["pass"]
        missing_stage = copy.deepcopy(report["temporal_anomalies"])
        missing_stage[0].pop("stage")
        assert any("stage" in issue for issue in temporal_anomaly_issues(missing_stage))
        missing_trigger = copy.deepcopy(report["temporal_anomalies"])
        missing_trigger[0].pop("trigger")
        assert any("trigger" in issue for issue in temporal_anomaly_issues(missing_trigger))
        source_metric = {
            "stage": "pre_rig", "kind": "global_position_speed", "subject": "hand_l",
            "previous": 0.0, "current": 46.0, "threshold": 40.0, "trigger": "rate",
            "rate": 46.0, "rate_threshold": 40.0, "elapsed_secs": 1.36, "frame_ms": 12.21,
            "source_boundary": "source_fall_heavy_to_light_on_physics_entry",
            "source_selector_inputs": [[-13.295677, 0.0], [0.0, 1.0]],
        }
        assert not temporal_anomaly_issues([source_metric])
        for key, value in (("stage", "final"), ("subject", "head"), ("kind", "local_rotation_spike"),
            ("source_boundary", "unknown"), ("source_selector_inputs", None),
            ("source_selector_inputs", [[-9.0, 0.0], [0.0, 1.0]]),
            ("source_selector_inputs", [[-13.0, 0.0], [0.0, 0.8]])):
            changed = copy.deepcopy(source_metric)
            changed[key] = value
            assert temporal_anomaly_issues([changed])
        assert temporal_anomaly_issues([source_metric, report["temporal_anomalies"][0]])
        report = fixture_report(paths)
        report.pop("temporal_anomalies")
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["graph_errors"] = ["ClipMissing"]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        report["ready"] = False
        report["pass"] = False
        report["graph_errors"] = [
            "MissingEdgeToSource(NodeTime(NodeId(17150000-0000-4000-8000-000000000001)))"
        ]
        report["primary_failure"] = fixture_graph_failure()
        result = audit(report, 2)
        assert not result["pass"]
        assert result["primary_failure"]["error_kind"] == "missing_edge_to_source"
        assert "fixture runtime" in result["primary_failure"]["summary"]
        report["primary_failure"].pop("active_context_nodes")
        result = audit(report, 2)
        assert any("active_context_nodes" in issue for issue in result["issues"])
        report["primary_failure"] = fixture_graph_failure()
        report["primary_failure"] = None
        result = audit(report, 2)
        assert any("graph failure diagnostics are missing" in issue for issue in result["issues"])
        report["primary_failure"] = fixture_graph_failure()
        report["primary_failure"]["node_candidates"].append(
            {
                "graph_path": "graphs/fixture/alternate_runtime.animgraph.ron",
                "graph_asset_id": "AssetId(alternate_runtime)",
                "graph_context": "GraphContextId(4)",
                "node_id": "NodeId(17150000-0000-4000-8000-000000000001)",
                "node_name": "alternate runtime",
                "node_kind": "Interruptible FSM",
                "active_context": True,
                "context_curves": {},
            }
        )
        report["primary_failure"]["active_node_candidate_count"] = 2
        report["primary_failure"]["candidate_resolution"] = "ambiguous_active_context"
        result = audit(report, 2)
        assert result["primary_failure"]["candidate_resolution"] == "ambiguous_active_context"
        assert "ambiguous" in result["primary_failure"]["summary"]
        report["primary_failure"] = fixture_graph_failure()
        report["primary_failure"]["pose_classification"] = "root_pose_channels_present_graph_error"
        result = audit(report, 2)
        assert any("missing root pose classification" in issue for issue in result["issues"])
        for present, kind in [(False, "MissingRootPose"), (True, "EmptyRootPose")]:
            report = fixture_report(paths)
            report["graph_errors"] = ["Root pose contract failed after evaluation"]
            report["primary_failure"] = fixture_graph_failure()
            failure = report["primary_failure"]
            failure.update(category="root_pose_contract_error", error_kind=kind,
                raw_error="Root pose contract failed after evaluation", pin=None,
                node_candidates=[], active_node_candidate_count=0, candidate_resolution="not_applicable")
            failure["root_pose"]["output_present"] = present
            failure["pose_classification"] = "reference_root_pose_likely_a_pose" if present else "missing_root_pose_likely_reference_or_a_pose"
            assert not primary_failure_issues(report)[0]
            assert not audit(report, 2)["pass"]
            failure["root_pose"]["active_channel_count"] = 1
            assert primary_failure_issues(report)[0]
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
        report["screenshots"] = []
        assert audit(report, 0)["pass"]
        assert not audit(report, 2)["pass"]
        report["pose_samples"][0]["root_pose"]["active_channel_count"] = 0
        assert not audit(report, 0)["pass"]
        report = fixture_report([])
        report["graph_errors"] = ["fixture"]
        report["primary_failure"] = fixture_graph_failure()
        report["primary_failure"]["screenshot_status"] = "disabled_by_configuration"
        diagnosis = audit(report, 0)
        assert not diagnosis["pass"]
        assert not any("screenshot" in issue for issue in diagnosis["issues"])
        report = fixture_report(paths)
        report["required_motion_windows"] = ["missing_dynamic"]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_source_pose_chain"] = True
        report["motion_windows"][0]["source_pose_chain"] = [
            fixture_source_pose_boundary("sequence_player"),
            fixture_source_pose_boundary("cache_producer"),
            fixture_source_pose_boundary("movement_fsm"),
        ]
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["expected_parent_runtime_state"] = 1
        report["motion_windows"][0]["observed_parent_runtime_states"] = [1]
        report["motion_windows"][0]["observed_parent_transition_active"] = [False]
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_requested_parent_runtime_states"] = [1]
        report["motion_windows"][0]["observed_requested_parent_transition_active"] = [False]
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_parent_transition_active"] = [False, True]
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_parent_runtime_states"] = [0]
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_parent_runtime_states"] = [1]
        report["motion_windows"][0]["observed_requested_parent_runtime_states"] = [0]
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_requested_parent_runtime_states"] = [1]
        report["motion_windows"][0]["expected_terminal_parent_runtime_state"] = 0
        report["motion_windows"][0]["expected_terminal_parent_transition_active"] = False
        report["motion_windows"][0][
            "expected_terminal_requested_parent_transition_active"
        ] = True
        report["motion_windows"][0]["observed_terminal_parent_runtime_states"] = [0]
        report["motion_windows"][0]["observed_terminal_parent_transition_active"] = [False]
        requested_expectation = report["motion_windows"][0].pop("expected_terminal_requested_parent_transition_active")
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["expected_terminal_requested_parent_transition_active"] = requested_expectation
        report["motion_windows"][0]["observed_terminal_requested_parent_runtime_states"] = [0]
        report["motion_windows"][0][
            "observed_terminal_requested_parent_transition_active"
        ] = [True]
        assert audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_terminal_parent_runtime_states"] = [1]
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_terminal_parent_runtime_states"] = [0]
        report["motion_windows"][0]["observed_terminal_requested_parent_runtime_states"] = [1]
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["observed_terminal_requested_parent_runtime_states"] = [0]
        report["motion_windows"][0][
            "observed_terminal_requested_parent_transition_active"
        ] = [False]
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0][
            "observed_terminal_requested_parent_transition_active"
        ] = [True]
        report["motion_windows"][0]["source_pose_chain"][1]["max_rotation_span_radians"] = 0.0
        report["motion_windows"][0]["first_failed_source_pose_boundary"] = "cache_producer: static"
        result = audit(report, 2)
        assert not result["pass"]
        assert any("cache_producer: static" in issue for issue in result["issues"])
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_source_pose_chain"] = True
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["max_pre_rig_translation_span"] = 0.0
        report["motion_windows"][0]["max_pre_rig_rotation_span_radians"] = 0.0
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["root_pose_max_translation_span"] = 0.0
        report["motion_windows"][0]["root_pose_max_rotation_span_radians"] = 0.0
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
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_actor_yaw_motion"] = True
        report["motion_windows"][0]["minimum_actor_yaw_delta_radians"] = 0.05
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_actor_yaw_convergence"] = True
        report["motion_windows"][0]["minimum_actor_yaw_convergence_radians"] = 0.05
        report["motion_windows"][0]["max_actor_yaw_delta_radians"] = 0.1
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_actor_yaw_motion"] = True
        report["motion_windows"][0]["requires_actor_yaw_convergence"] = True
        report["motion_windows"][0]["minimum_actor_yaw_delta_radians"] = 1.4
        report["motion_windows"][0]["minimum_actor_yaw_convergence_radians"] = 1.4
        report["motion_windows"][0]["max_actor_yaw_delta_radians"] = 0.7
        report["motion_windows"][0]["max_actor_yaw_convergence_radians"] = 0.7
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_actor_yaw_motion"] = True
        report["motion_windows"][0]["minimum_actor_yaw_delta_radians"] = 0.05
        report["motion_windows"][0]["expected_actor_yaw_delta_sign"] = -1
        report["motion_windows"][0]["expected_actor_yaw_baseline"] = 0.0
        report["motion_windows"][0]["expected_animation_variant"] = "FixtureTurn"
        report["motion_windows"][0]["max_actor_yaw_delta_radians"] = 0.1
        report["motion_windows"][0]["max_actor_yaw_signed_delta_radians"] = 0.1
        report["motion_windows"][0]["actor_yaw_trace"] = [
            {
                "elapsed_secs": 0.0,
                "actor_yaw_radians": 0.0,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.0,
                "animation_weight": 1.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 1.0,
            },
            {
                "elapsed_secs": 1.0,
                "actor_yaw_radians": -0.1,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.1,
                "animation_weight": 1.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 1.0,
            },
        ]
        for sample in report["motion_windows"][0]["actor_yaw_trace"]:
            sample["animation_variant"] = "FixtureTurn"
            sample["animation_curve_direct_yaw_speed_degrees"] = sample[
                "animation_yaw_speed_degrees"
            ]
            sample["animation_curve_stack_entries"] = [
                {
                    "animation_variant": "FixtureTurn",
                    "elapsed_seconds": sample["animation_clip_time"],
                    "weight": 1.0,
                    "play_rate": 1.0,
                    "sampling_ready": True,
                }
            ]
        assert audit(report, 2)["pass"]
        native = copy.deepcopy(report)
        window = native["motion_windows"][0]
        window["actor_yaw_driver"] = "locomotion"
        window["expected_animation_variant"] = None
        window["source_node"] = "Source/ActorResponse"
        window["source_state"] = "Grounded"
        window["expected_runtime_state"] = 0
        window["observed_runtime_states"] = [0]
        for sample in window["actor_yaw_trace"]:
            for key in list(sample):
                if key.startswith("animation_"):
                    del sample[key]
            sample["animation_yaw_speed_degrees"] = 0.0
        assert audit(native, 2)["pass"], audit(native, 2)["issues"]
        window["canonical_clip_time"] = 0.6
        window["maximum_clip_time_distance"] = 0.05
        window["canonical_pre_rig_pose"] = fixture_canonical_pose()
        assert not audit(native, 2)["pass"]
        window["canonical_clip_time_distance"] = 0.01
        assert audit(native, 2)["pass"]
        window["canonical_clip_time_distance"] = 0.1
        assert not audit(native, 2)["pass"]
        window["canonical_clip_time_distance"] = 0.01
        window["actor_yaw_trace"][0]["animation_yaw_speed_degrees"] = 45.0
        assert not audit(native, 2)["pass"]
        window["actor_yaw_driver"] = "unknown"
        assert not audit(native, 2)["pass"]
        report["motion_windows"][0]["actor_yaw_trace"][1]["animation_curve_stack_entries"] = []
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["actor_yaw_trace"][1]["animation_curve_stack_entries"] = [
            {
                "animation_variant": "FixtureTurn",
                "elapsed_seconds": 0.1,
                "weight": 1.0,
                "play_rate": 1.0,
                "sampling_ready": True,
            }
        ]
        report["motion_windows"][0]["actor_yaw_trace"][1]["animation_variant"] = "WrongTurn"
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["actor_yaw_trace"][1]["animation_variant"] = "FixtureTurn"
        report["motion_windows"][0]["actor_yaw_trace"] = []
        assert not audit(report, 2)["pass"]
        report["motion_windows"][0]["actor_yaw_trace"] = [
            {
                "elapsed_secs": 0.0,
                "actor_yaw_radians": 0.0,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.0,
                "animation_weight": 1.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 1.0,
            },
            {
                "elapsed_secs": 1.0,
                "actor_yaw_radians": -0.1,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.1,
                "animation_weight": 1.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 1.0,
            },
        ]
        for sample in report["motion_windows"][0]["actor_yaw_trace"]:
            sample["animation_variant"] = "FixtureTurn"
            sample["animation_curve_direct_yaw_speed_degrees"] = sample[
                "animation_yaw_speed_degrees"
            ]
            sample["animation_curve_stack_entries"] = [
                {
                    "animation_variant": "FixtureTurn",
                    "elapsed_seconds": sample["animation_clip_time"],
                    "weight": 1.0,
                    "play_rate": 1.0,
                    "sampling_ready": True,
                }
            ]
        report["motion_windows"][0]["max_actor_yaw_signed_delta_radians"] = 0.0
        result = audit(report, 2)
        assert not result["pass"]
        assert not any("turned opposite" in issue for issue in result["issues"])
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_actor_yaw_motion"] = True
        report["motion_windows"][0]["minimum_actor_yaw_delta_radians"] = 0.05
        report["motion_windows"][0]["expected_actor_yaw_delta_sign"] = -1
        report["motion_windows"][0]["expected_actor_yaw_baseline"] = 0.0
        report["motion_windows"][0]["expected_animation_variant"] = "FixtureTurn"
        report["motion_windows"][0]["max_actor_yaw_delta_radians"] = 0.1
        report["motion_windows"][0]["max_actor_yaw_signed_delta_radians"] = 0.0
        report["motion_windows"][0]["max_actor_yaw_opposite_delta_radians"] = 0.1
        report["motion_windows"][0]["actor_yaw_trace"] = [
            {
                "elapsed_secs": 0.0,
                "actor_yaw_radians": 0.0,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_curve_direct_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.0,
                "animation_weight": 1.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 1.0,
                "animation_variant": "FixtureTurn",
                "animation_curve_stack_entries": [
                    {
                        "animation_variant": "FixtureTurn",
                        "elapsed_seconds": 0.0,
                        "weight": 1.0,
                        "play_rate": 1.0,
                        "sampling_ready": True,
                    }
                ],
            },
            {
                "elapsed_secs": 1.0,
                "actor_yaw_radians": 0.1,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_curve_direct_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.1,
                "animation_weight": 1.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 1.0,
                "animation_variant": "FixtureTurn",
                "animation_curve_stack_entries": [
                    {
                        "animation_variant": "FixtureTurn",
                        "elapsed_seconds": 0.1,
                        "weight": 1.0,
                        "play_rate": 1.0,
                        "sampling_ready": True,
                    }
                ],
            },
        ]
        result = audit(report, 2)
        assert not result["pass"]
        assert any("turned opposite" in issue for issue in result["issues"])
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["requires_actor_yaw_motion"] = True
        report["motion_windows"][0]["minimum_actor_yaw_delta_radians"] = 0.05
        report["motion_windows"][0]["expected_actor_yaw_delta_sign"] = -1
        report["motion_windows"][0]["expected_actor_yaw_baseline"] = 0.0
        report["motion_windows"][0]["expected_animation_variant"] = "FixtureTurn"
        report["motion_windows"][0]["max_actor_yaw_delta_radians"] = 0.1
        report["motion_windows"][0]["max_actor_yaw_signed_delta_radians"] = 0.1
        report["motion_windows"][0]["actor_yaw_trace"] = [
            {
                "elapsed_secs": 0.0,
                "actor_yaw_radians": 0.0,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.0,
                "animation_weight": 0.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 0.0,
            },
            {
                "elapsed_secs": 1.0,
                "actor_yaw_radians": -0.1,
                "view_yaw_radians": -0.2,
                "animation_yaw_speed_degrees": -45.0,
                "animation_clip_time": 0.1,
                "animation_weight": 0.0,
                "animation_play_rate": 1.0,
                "animation_curve_stack_weight": 0.0,
            },
        ]
        for sample in report["motion_windows"][0]["actor_yaw_trace"]:
            sample["animation_variant"] = "FixtureTurn"
            sample["animation_curve_direct_yaw_speed_degrees"] = sample[
                "animation_yaw_speed_degrees"
            ]
            sample["animation_curve_stack_entries"] = [
                {
                    "animation_variant": "FixtureTurn",
                    "elapsed_seconds": sample["animation_clip_time"],
                    "weight": 0.0,
                    "play_rate": 1.0,
                    "sampling_ready": True,
                }
            ]
        assert not audit(report, 2)["pass"]
        report = fixture_report(paths)
        add_dynamic_window_samples(report, ["dynamic"])
        report["motion_windows"] = [fixture_motion_window()]
        report["motion_windows"][0]["expected_lifecycle_stages"] = ["active", "completed"]
        report["motion_windows"][0]["observed_lifecycle_stages"] = ["active"]
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
        "stairs_source_speed_and_pose_windows_checked": "PASS",
        "uphill_capsule_landing_and_animation_recovery_checked": "PASS",
        "moving_base_source_path_ride_jump_mantle_and_handoff_checked": "PASS",
        "rotating_base_clock_coordinates_steps_and_mantle_checked": "PASS",
        "stairs_missing_routes_gaits_and_hotkey_recovery_rejected": "PASS",
        "fault_rejected": "PASS",
        "startup_graph_error_rejected": "PASS",
        "structured_graph_failure_reported": "PASS",
        "graph_failure_context_diagnostic_required": "PASS",
        "missing_graph_failure_diagnostic_rejected": "PASS",
        "automation_watchdog_record_required": "PASS",
        "ambiguous_graph_candidate_preserved": "PASS",
        "a_pose_failure_classification_rejected": "PASS",
        "temporal_anomaly_schema_required": "PASS",
        "instant_limb_deformation_rejected": "PASS",
        "temporal_anomaly_stage_required": "PASS",
        "temporal_anomaly_trigger_required": "PASS",
        "foot_lock_teleport_evidence_required": "PASS",
        "foot_ik_teleport_case_required": "PASS",
        "runtime_phase_evidence_required": "PASS",
        "physics_step_stage_required": "PASS",
        "state_machine_direction_evidence_required": "PASS",
        "sprint_source_curve_relay_required": "PASS",
        "shared_pre_rig_motion_window_required": "PASS",
        "missing_required_motion_window_rejected": "PASS",
        "source_pose_chain_required": "PASS",
        "first_static_source_boundary_reported": "PASS",
        "wrong_parent_fsm_state_rejected": "PASS",
        "wrong_requested_parent_fsm_state_rejected": "PASS",
        "wrong_terminal_parent_fsm_state_rejected": "PASS",
        "automatic_transition_tail_allowed": "PASS",
        "static_dynamic_window_rejected": "PASS",
        "source_motion_floor_required": "PASS",
        "motion_window_span_required": "PASS",
        "static_root_graph_pose_rejected": "PASS",
        "reference_root_pose_rejected": "PASS",
        "static_final_pose_rejected": "PASS",
        "actor_yaw_motion_required": "PASS",
        "actor_yaw_convergence_required": "PASS",
        "positive_but_insufficient_actor_yaw_rejected": "PASS",
        "actor_yaw_trace_required": "PASS",
        "inactive_clip_yaw_curve_rejected": "PASS",
        "active_curve_stack_entry_required": "PASS",
        "wrong_animation_variant_rejected": "PASS",
        "insufficient_signed_yaw_not_mislabeled_opposite": "PASS",
        "opposite_actor_yaw_rejected": "PASS",
        "turn_lifecycle_completion_required": "PASS",
        "collapsed_direction_signatures_rejected": "PASS",
        "montage_clip_signature_contract_checked": "PASS",
        "montage_clip_trace_without_actor_yaw_checked": "PASS",
        "wrong_direction_state_rejected": "PASS",
        "single_airborne_frame_hitch_verified": "PASS",
        "source_landing_selector_classification_checked": "PASS",
    }


def audit_functional(report, min_screenshots):
    result = audit(report, min_screenshots)
    deferred = [issue for issue in result["issues"] if issue in {
        "performance budget exceeded: max_frame_p95_ms",
        "performance budget exceeded: max_frame_ms",
        "performance budget exceeded: max_eval_p95_ms",
    }]
    issues = [issue for issue in result["issues"] if issue not in deferred]
    if deferred and report.get("pass") is False and "runtime visual verdict is not pass" in issues:
        issues.remove("runtime visual verdict is not pass")
    return {**result, "pass": not issues, "issues": issues,
            "validation_scope": "functional", "full_pass": result["pass"], "deferred_performance_issues": deferred}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    parser.add_argument("--min-screenshots", type=int, default=0)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--functional-only", action="store_true")
    parser.add_argument("--baseline-frame-delta-seconds", type=float, default=0.0)
    parser.add_argument("--expected-frame-hitch-seconds", type=float, default=0.0)
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.report is None or not args.check or args.min_screenshots < 0:
        raise ValueError("report, --check, and nonnegative min-screenshots are required")
    if not args.report.is_file():
        raise ValueError("visual report is missing")
    audit_report = audit_functional if args.functional_only else audit
    report = read_json(args.report)
    result = audit_report(report, args.min_screenshots)
    if args.expected_frame_hitch_seconds:
        if not (math.isfinite(args.baseline_frame_delta_seconds) and math.isfinite(args.expected_frame_hitch_seconds)
                and 0.0 < args.baseline_frame_delta_seconds < args.expected_frame_hitch_seconds <= 0.25):
            raise ValueError("invalid frame hitch request")
        issues = frame_hitch_issues(report, args.baseline_frame_delta_seconds, args.expected_frame_hitch_seconds)
        result["issues"].extend(issues)
        result["pass"] = not result["issues"]
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
