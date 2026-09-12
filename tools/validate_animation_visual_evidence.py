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
    if values:
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
            "temporal_global_position_speed_mps": 24.0,
            "temporal_local_translation_jump_m": 0.35,
            "temporal_local_rotation_jump_radians": 2.2,
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


def run_self_test():
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
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.report is None or not args.check or args.min_screenshots < 0:
        raise ValueError("report, --check, and nonnegative min-screenshots are required")
    if not args.report.is_file():
        raise ValueError("visual report is missing")
    audit_report = audit_functional if args.functional_only else audit
    result = audit_report(read_json(args.report), args.min_screenshots)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
