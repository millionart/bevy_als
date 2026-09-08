import copy
import json
import math
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "import"))

import generate_ue_transition_expression_graph as scalar_renderer
from coordinate_basis import source_heading_yaw_degrees_to_runtime


class PoseRenderError(ValueError):
    pass


SUPPORTED_RENDERERS = {
    "sequence_player",
    "sequence_evaluator",
    "two_way_blend",
    "apply_additive",
    "apply_mesh_space_additive",
    "make_dynamic_additive",
    "curves_blend",
    "modify_curve",
    "state_result",
    "root_output",
    "save_cached_pose",
    "use_cached_pose",
    "linked_input_pose",
    "linked_anim_graph",
    "linked_anim_layer",
    "control_rig",
    "pose_snapshot",
    "inertialization",
    "pose_lifecycle_call",
    "blendspace_evaluator",
    "blendspace_player",
    "property_access",
    "boolean_operator",
    "comparison_operator",
    "gameplay_tag_equal",
    "boolean_to_float",
    "state_machine",
    "multi_way_blend",
    "set_hips_direction",
    "enum_pose_blend",
    "layered_bone_blend",
    "gameplay_tag_blend",
    "slot",
    "reroute",
}

SCALAR_RENDERERS = {
    "property_access",
    "boolean_operator",
    "comparison_operator",
    "gameplay_tag_equal",
    "boolean_to_float",
}


def quoted(value):
    return json.dumps(value, ensure_ascii=False)


def node_id(node):
    value = node.get("node_guid")
    try:
        return str(uuid.UUID(value))
    except (AttributeError, TypeError, ValueError):
        path = node.get("object_path")
        if not isinstance(path, str) or not path:
            raise PoseRenderError("source node identity is invalid")
        return str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/pose/" + path))


def node_index(graph_contract):
    values = graph_contract.get("source_graph", {}).get("nodes")
    if not isinstance(values, list):
        raise PoseRenderError("source graph nodes are invalid")
    return {node.get("object_path"): node for node in values}


def semantic_index(graph_ir):
    values = graph_ir.get("nodes")
    if not isinstance(values, list):
        raise PoseRenderError("source IR graph nodes are invalid")
    return {node.get("source_node"): node for node in values}


def asset_index(animation_assets):
    values = {}
    for asset in animation_assets:
        path = asset.get("source_node")
        if not isinstance(path, str) or path in values:
            continue
        values[path] = asset
    return values


def source_title(node):
    value = node.get("title")
    if not isinstance(value, str) or not value:
        raise PoseRenderError("source node title is invalid")
    return value


def position(node):
    x = node.get("position_x")
    y = node.get("position_y")
    if not isinstance(x, (int, float)) or not isinstance(y, (int, float)):
        raise PoseRenderError("source node position is invalid")
    return float(x), float(y)


def input_names(semantic):
    values = semantic.get("input_pins")
    if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
        raise PoseRenderError("source renderer input pins are invalid")
    return values


def field(name, value):
    return f"{name}: {value}"


def bool_ron(value):
    return "true" if value else "false"


def mapping_ron(mapping):
    required = (
        "source_semantics",
        "map_range",
        "input_min",
        "input_max",
        "output_min",
        "output_max",
        "scale",
        "bias",
        "clamp_result",
        "clamp_min",
        "clamp_max",
        "interpolate_result",
        "interpolate_speed_increasing",
        "interpolate_speed_decreasing",
    )
    if not isinstance(mapping, dict) or any(name not in mapping for name in required):
        raise PoseRenderError("source alpha mapping is invalid")
    values = [
        field("source_semantics", bool_ron(mapping["source_semantics"])),
        field("map_range", bool_ron(mapping["map_range"])),
        field("input_min", repr(mapping["input_min"])),
        field("input_max", repr(mapping["input_max"])),
        field("output_min", repr(mapping["output_min"])),
        field("output_max", repr(mapping["output_max"])),
        field("scale", repr(mapping["scale"])),
        field("bias", repr(mapping["bias"])),
        field("clamp_result", bool_ron(mapping["clamp_result"])),
        field("clamp_min", repr(mapping["clamp_min"])),
        field("clamp_max", repr(mapping["clamp_max"])),
        field("interpolate_result", bool_ron(mapping["interpolate_result"])),
        field("interpolate_speed_increasing", repr(mapping["interpolate_speed_increasing"])),
        field("interpolate_speed_decreasing", repr(mapping["interpolate_speed_decreasing"])),
    ]
    return "(" + ", ".join(values) + ")"


def asset_for_node(asset_by_node, source_node, category):
    asset = asset_by_node.get(source_node)
    if not isinstance(asset, dict) or asset.get("category") != category:
        raise PoseRenderError("source animation asset binding is unavailable")
    target = asset.get("target_asset")
    if not isinstance(target, str) or not target:
        raise PoseRenderError("target animation asset binding is invalid")
    return target


def cache_key(graph_contract, cache_name):
    asset = graph_contract.get("source_asset")
    if not isinstance(asset, str) or not isinstance(cache_name, str) or not cache_name:
        raise PoseRenderError("source cache identity is invalid")
    return "ue-cache:" + str(uuid.uuid5(uuid.NAMESPACE_URL, asset + ":" + cache_name))


def sync_marker_ron(markers):
    if not isinstance(markers, list) or len(markers) == 1:
        raise PoseRenderError("source sync marker contract is invalid")
    rows = []
    for marker in markers:
        if (
            not isinstance(marker, dict)
            or not isinstance(marker.get("name"), str)
            or not marker["name"]
            or not isinstance(marker.get("time"), (int, float))
        ):
            raise PoseRenderError("source blendspace marker is invalid")
        rows.append("(name: " + quoted(marker["name"]) + ", time: " + repr(marker["time"]) + ")")
    return "[" + ", ".join(rows) + "]"


def marker_ron(markers):
    if not isinstance(markers, list) or len(markers) < 2:
        raise PoseRenderError("source blendspace marker contract is invalid")
    return sync_marker_ron(markers)


def pose_curves_ron(curves):
    if not isinstance(curves, list):
        raise PoseRenderError("source animation curves are unavailable")
    entries = []
    for curve in curves:
        if curve.get("flags", 0) & 64:
            continue
        count = len(curve.get("times", []))
        if not count or len(curve.get("values", [])) != count:
            raise PoseRenderError("source animation curve keys are invalid")
        if any(right <= left for left, right in zip(curve["times"], curve["times"][1:])):
            raise PoseRenderError("source animation curve times are invalid")
        fields = []
        for name in ("times", "values", "tangent_modes", "weight_modes", "arrive_tangents", "leave_tangents", "arrive_weights", "leave_weights"):
            values = curve.get(name, [0] * count)
            if len(values) != count or any(not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value) for value in values):
                raise PoseRenderError("source animation curve values are invalid")
            if curve["name"] == "RotationYawSpeed" and name in {"values", "arrive_tangents", "leave_tangents"}:
                values = [source_heading_yaw_degrees_to_runtime(value) for value in values]
            fields.append(name + ": [" + ", ".join(repr(value) for value in values) + "]")
        modes = {"LINEAR": "Linear", "CONSTANT": "Constant", "CUBIC": "Cubic"}
        interps = curve.get("interps", [])
        if len(interps) != count or any(value not in modes for value in interps):
            raise PoseRenderError("source animation curve interpolation is unsupported")
        fields.append("interps: [" + ", ".join(modes[value] for value in interps) + "]")
        modes = {"RCCE_Constant": "Constant", "RCCE_Linear": "Linear", "RCCE_Cycle": "Cycle", "RCCE_CycleWithOffset": "CycleWithOffset", "RCCE_Oscillate": "Oscillate", "RCCE_None": "Constant", 5: "Constant"}
        for name in ("pre_infinity", "post_infinity"):
            value = curve.get(name)
            if value not in modes:
                raise PoseRenderError("source animation curve extrapolation is unsupported")
            fields.append(name + ": " + modes[value])
        entries.append(quoted(curve["name"]) + ": (" + ", ".join(fields) + ")")
    return "({" + ", ".join(entries) + "})"


def sync_clock_fields(value):
    key = value.get("sync_phase_key")
    role = value.get("sync_group_role")
    delta = value.get("delta_seconds_key")
    duration = value.get("sync_phase_duration")
    looped = value.get("sync_phase_looped")
    if (
        not isinstance(key, str) or not key
        or role not in {"CanBeLeader", "AlwaysFollower", "AlwaysLeader", "TransitionLeader", "TransitionFollower", "ExclusiveAlwaysLeader"}
        or not isinstance(delta, str) or not delta
        or not isinstance(duration, (int, float)) or isinstance(duration, bool) or duration <= 0.0
        or not isinstance(looped, bool) or looped != value.get("loop_animation", value.get("loop"))
    ):
        raise PoseRenderError("source sync group clock contract is unavailable")
    sync_marker_ron(value.get("sync_phase_markers"))
    return [field("key", quoted(key)), field("role", role), field("delta_seconds_key", quoted(delta))]


def blendspace_inner(value, player=False):
    if not isinstance(value, dict):
        raise PoseRenderError("source blendspace contract is unavailable")
    blendspace = value.get("blendspace")
    if not isinstance(blendspace, dict):
        raise PoseRenderError("source blendspace contract is unavailable")
    if player:
        if abs(value.get("configured_play_rate", 0.0) - 1.0) > 1.0e-6:
            raise PoseRenderError("source blendspace player play rate is unsupported")
        phase_duration = value.get("sync_phase_duration")
        if not isinstance(phase_duration, (int, float)) or phase_duration <= 0.0:
            raise PoseRenderError("source blendspace player phase duration is invalid")
        phase_markers = marker_ron(value.get("sync_phase_markers"))
    else:
        if not value.get("teleport_to_normalized_time"):
            raise PoseRenderError("source blendspace evaluator time mode is unsupported")
        if abs(value.get("configured_play_rate", 0.0) - 1.0) > 1.0e-6:
            raise PoseRenderError("source blendspace evaluator play rate is unsupported")
        phase_duration = 0.0
        phase_markers = "[]"
    points = []
    for index, sample in enumerate(blendspace.get("samples", [])):
        coordinates = sample.get("value") if isinstance(sample, dict) else None
        target = sample.get("target_clip") if isinstance(sample, dict) else None
        rate = sample.get("rate_scale") if isinstance(sample, dict) else None
        if (
            not isinstance(coordinates, list)
            or len(coordinates) < 2
            or not all(isinstance(component, (int, float)) for component in coordinates)
            or any(abs(component) > 1.0e-8 for component in coordinates[2:])
            or not isinstance(target, str)
            or not target
            or not isinstance(rate, (int, float))
            or abs(rate - 1.0) > 1.0e-6
        ):
            raise PoseRenderError("source blendspace sample is unsupported")
        duration = 0.0
        markers = "[]"
        if player:
            timing = sample.get("source_timing") if isinstance(sample, dict) else None
            if (
                not isinstance(timing, dict)
                or not isinstance(timing.get("duration"), (int, float))
                or timing["duration"] <= 0.0
            ):
                raise PoseRenderError("source blendspace player sample timing is invalid")
            duration = timing["duration"]
            markers = marker_ron(timing.get("markers"))
        points.append(
            "(id: "
            + quoted(f"sample_{index}")
            + f", point: ({float(coordinates[0])!r}, {float(coordinates[1])!r}), duration: {duration!r}, markers: {markers}"
            + ", curves: " + pose_curves_ron(sample.get("source_curves", [])) + ")"
        )
    if not points:
        raise PoseRenderError("source blendspace samples are missing")
    playback = "None"
    triangulation = "None"
    if player:
        binding_keys = value.get("input_binding_keys")
        topology = value.get("triangulation")
        if not isinstance(binding_keys, list) or len(binding_keys) != 3 or not isinstance(topology, dict):
            raise PoseRenderError("source blendspace clock contract is unavailable")
        playback = "Some((" + ", ".join([*sync_clock_fields(value), field("start_position", repr(value["start_position"])), field("play_rate_key", quoted(binding_keys[2]))]) + "))"
        triangulation = "Some((axis_min: (" + ", ".join(repr(float(x)) for x in topology["axis_min"]) + "), axis_max: (" + ", ".join(repr(float(x)) for x in topology["axis_max"]) + "), triangles: [" + ", ".join("(" + ", ".join(str(index) for index in triangle) + ")" for triangle in topology["triangles"]) + "]))"
    return (
        "als_ext::NormalizedBlendSpaceNode",
        "(looped: "
        + bool_ron(value["loop"])
        + ", phase_duration: "
        + repr(phase_duration)
        + ", phase_markers: "
        + phase_markers
        + ", points: ["
        + ", ".join(points)
        + "], playback: " + playback + ", source_triangulation: " + triangulation + ")",
    )


def rendered_inner(graph_contract, source, semantic, asset_by_node, fsm_assets):
    renderer = semantic.get("renderer")
    value = semantic.get("semantic")
    if (
        renderer not in {"root_output", "property_access", "state_machine", *SCALAR_RENDERERS}
        and not isinstance(value, dict)
    ):
        raise PoseRenderError("source renderer semantic is unavailable")
    if renderer == "sequence_player":
        pins = input_names(value)
        play_rate_key = value.get("play_rate_key")
        loop_animation_key = value.get("loop_animation_key")
        play_rate_pin = value.get("play_rate_pin")
        start_position_pin = value.get("start_position_pin")
        loop_animation_pin = value.get("loop_animation_pin")
        if (
            not isinstance(play_rate_key, str)
            or not isinstance(loop_animation_key, str)
            or not all(
                isinstance(item, str) and item
                for item in (play_rate_pin, start_position_pin, loop_animation_pin)
            )
        ):
            raise PoseRenderError("source sequence property binding is unavailable")
        fields = [
            field("clip", quoted(asset_for_node(asset_by_node, semantic["source_node"], "sequence"))),
            field("curves", pose_curves_ron(asset_by_node[semantic["source_node"]].get("source_curves", []))),
            field("additive_reference", "None"),
            field("additive_reference_position", "0.0"),
            field("play_rate", repr(value["play_rate"])),
            field("play_rate_key", quoted(play_rate_key)),
            field("play_rate_pin", quoted(play_rate_pin)),
            field("expose_play_rate_pin", bool_ron(play_rate_pin in pins)),
            field("play_rate_basis", repr(value["play_rate_basis"])),
            field("play_rate_mapping", mapping_ron(value["play_rate_mapping"])),
            field("start_position", repr(value["start_position"])),
            field("start_position_pin", quoted(start_position_pin)),
            field("expose_start_position_pin", bool_ron(start_position_pin in pins)),
            field("loop_animation", bool_ron(value["loop_animation"])),
            field("loop_animation_key", quoted(loop_animation_key)),
            field("loop_animation_pin", quoted(loop_animation_pin)),
            field("expose_loop_animation_pin", bool_ron(loop_animation_pin in pins)),
            field("sync_group", quoted(value["sync_group"])),
        ]
        if value["sync_group"] != "None":
            synchronization = sync_clock_fields(value)
            timing = asset_by_node[semantic["source_node"]]["source_timing"]
            synchronization.extend([field("phase_duration", repr(value["sync_phase_duration"])), field("phase_markers", sync_marker_ron(value["sync_phase_markers"])), field("markers", sync_marker_ron(timing["markers"]))])
            fields.append(field("synchronization", "Some((" + ", ".join(synchronization) + "))"))
        return "als_ext::SourceSequencePlayerNode", "(" + ", ".join(fields) + ")"
    if renderer == "sequence_evaluator":
        pins = input_names(value)
        explicit_frame_pin = value.get("explicit_frame_pin")
        if not isinstance(explicit_frame_pin, str) or not explicit_frame_pin:
            raise PoseRenderError("source sequence evaluator pin is unavailable")
        fields = [
            field("clip", quoted(asset_for_node(asset_by_node, semantic["source_node"], "sequence"))),
            field("curves", pose_curves_ron(asset_by_node[semantic["source_node"]].get("source_curves", []))),
            field("explicit_frame", "0.0"),
            field("explicit_frame_key", quoted("")),
            field("explicit_frame_pin", quoted(explicit_frame_pin)),
            field("expose_explicit_frame_pin", bool_ron(explicit_frame_pin in pins)),
            field("frames_per_second", "30.0"),
            field("should_loop", bool_ron(value["loop"])),
            field("teleport_to_explicit_time", bool_ron(value["teleport_to_explicit_time"])),
            field("reinitialization_behavior", quoted(value["reinitialization_behavior"])),
        ]
        return "als_ext::SourceSequenceEvaluatorNode", "(" + ", ".join(fields) + ")"
    if renderer == "blendspace_evaluator":
        return blendspace_inner(value)
    if renderer == "blendspace_player":
        return blendspace_inner(value, player=True)
    if renderer == "slot":
        keys = [value.get(name) for name in ("pose_key", "weight_key", "layer_weight_key", "mesh_space_additive_key")]
        if not all(isinstance(key, str) and key for key in keys):
            raise PoseRenderError("source slot provider contract is unavailable")
        return (
            "als_ext::SourceSlotNode",
            "(slot_name: "
            + quoted(value["slot_name"])
            + ", group_name: "
            + quoted(value["group_name"])
            + ", pose_key: "
            + quoted(keys[0])
            + ", weight_key: "
            + quoted(keys[1])
            + ", layer_weight_key: "
            + quoted(keys[2])
            + ", mesh_space_additive_key: "
            + quoted(keys[3])
            + ")",
        )
    if renderer == "gameplay_tag_blend":
        active_key = value.get("active_key")
        blend_type = value.get("blend_type")
        mode = {"Linear": "Linear", "Cubic": "HermiteCubic"}.get(blend_type)
        fields = (
            "active_key",
            "tags",
            "blend_times",
            "source_blend_times",
            "pose_pin_names",
            "blend_time_pin_names",
            "active_tag_pin",
            "output_pin",
        )
        if (
            not isinstance(active_key, str)
            or not active_key
            or mode is None
            or any(not isinstance(value.get(field), list) for field in ("tags", "blend_times", "source_blend_times", "pose_pin_names", "blend_time_pin_names"))
            or not all(isinstance(value.get(field), str) and value[field] for field in ("active_tag_pin", "output_pin"))
            or len(value["tags"]) + 1 != len(value["blend_times"])
            or len(value["blend_times"]) != len(value["source_blend_times"])
            or len(value["blend_times"]) != len(value["pose_pin_names"])
            or len(value["blend_times"]) != len(value["blend_time_pin_names"])
        ):
            raise PoseRenderError("source gameplay tag blend contract is invalid")
        return (
            "als_ext::SourceGameplayTagBlendNode",
            "(active_key: "
            + quoted(active_key)
            + ", active_tag_pin: "
            + quoted(value["active_tag_pin"])
            + ", tags: ["
            + ", ".join(quoted(tag) for tag in value["tags"])
            + "], blend_times: ["
            + ", ".join(repr(blend_time) for blend_time in value["blend_times"])
            + "], blend_mode: "
            + mode
            + ", source_blend_times: ["
            + ", ".join(repr(blend_time) for blend_time in value["source_blend_times"])
            + "], pose_pin_names: ["
            + ", ".join(quoted(pin) for pin in value["pose_pin_names"])
            + "], blend_time_pin_names: ["
            + ", ".join(quoted(pin) for pin in value["blend_time_pin_names"])
            + "], output_pin: "
            + quoted(value["output_pin"])
            + ")",
        )
    if renderer == "property_access":
        try:
            tag_input = scalar_renderer.gameplay_tag_input_config(graph_contract, source)
        except ValueError as error:
            raise PoseRenderError("source GameplayTag property access is unsupported") from error
        if tag_input is not None:
            return (
                "als_ext::SourceGameplayTagInputNode",
                "(key: "
                + quoted(tag_input["key"])
                + ", output: "
                + quoted(tag_input["output"])
                + ", expected_tag: \"\", source_property_path: "
                + quoted(tag_input["source_property_path"])
                + ", source_properties: ["
                + ", ".join(quoted(item) for item in scalar_renderer.source_properties(source))
                + "])"
            )
    if renderer in SCALAR_RENDERERS:
        try:
            return (
                "als_ext::SourceScalarExpressionNode",
                scalar_renderer.node_inner(graph_contract, source),
            )
        except ValueError as error:
            raise PoseRenderError("source scalar contract is unsupported") from error
    if renderer == "pose_snapshot":
        key = value.get("snapshot_key") if isinstance(value, dict) else None
        if not isinstance(key, str) or not key:
            raise PoseRenderError("source pose snapshot provider is unavailable")
        return "als_ext::GlobalPoseInputNode", "(key: " + quoted(key) + ', output: "Pose")'
    if renderer == "state_machine":
        fsm_source = value.get("fsm_source_graph") if isinstance(value, dict) else None
        config = fsm_assets.get(fsm_source)
        if not isinstance(config, dict):
            raise PoseRenderError("source state machine materialization is unavailable")
        aliases = config.get("state_aliases", [])
        conditions = config.get("source_transition_conditions", [])
        entry_conditions = config.get("source_entry_conditions", [])
        max_transitions_per_frame = config.get("max_transitions_per_frame", 3)
        max_transition_requests = config.get("max_transition_requests", 32)
        skip_first_update_transition = config.get("skip_first_update_transition", True)
        reinitialize_on_becoming_relevant = config.get(
            "reinitialize_on_becoming_relevant", True
        )
        if (
            not isinstance(aliases, list)
            or not isinstance(conditions, list)
            or not isinstance(entry_conditions, list)
            or not isinstance(max_transitions_per_frame, int)
            or max_transitions_per_frame < 0
            or not isinstance(max_transition_requests, int)
            or max_transition_requests < 0
            or not isinstance(skip_first_update_transition, bool)
            or not isinstance(reinitialize_on_becoming_relevant, bool)
            or not all(
                isinstance(condition, dict)
                and all(
                    isinstance(condition.get(field), str) and condition[field]
                    for field in (
                        "transition_id",
                        "source_state_id",
                        "condition",
                        "output",
                        "context_state_id",
                    )
                )
                and (
                    condition.get("automatic_trigger_time") is None
                    or isinstance(condition.get("automatic_trigger_time"), (int, float))
                )
                and isinstance(condition.get("transition_duration"), (int, float))
                and isinstance(condition.get("always_reset_target_on_entry"), bool)
                and isinstance(condition.get("gates", []), list)
                and all(
                    isinstance(gate, dict)
                    and all(
                        isinstance(gate.get(field), str) and gate[field]
                        for field in ("condition", "output", "context_state_id")
                    )
                    for gate in condition.get("gates", [])
                )
                and isinstance(condition.get("notifications", []), list)
                and all(
                    isinstance(notification, dict)
                    and all(
                        isinstance(notification.get(field), str) and notification[field]
                        for field in ("phase", "name")
                    )
                    for notification in condition.get("notifications", [])
                )
                for condition in conditions
            )
            or not all(
                isinstance(condition, dict)
                and isinstance(condition.get("target_state_id"), str)
                and condition["target_state_id"]
                and isinstance(condition.get("gates", []), list)
                and all(
                    isinstance(gate, dict)
                    and all(
                        isinstance(gate.get(field), str) and gate[field]
                        for field in ("condition", "output", "context_state_id")
                    )
                    for gate in condition.get("gates", [])
                )
                for condition in entry_conditions
            )
        ):
            raise PoseRenderError("source state machine materialization is invalid")
        alias_ron = ", ".join(
            "(state_id: "
            + quoted(alias["state_id"])
            + ", member_state_ids: ["
            + ", ".join(quoted(member) for member in alias["member_state_ids"])
            + "])"
            for alias in aliases
        )
        conditions_ron = ", ".join(
            "(transition_id: "
            + quoted(condition["transition_id"])
            + ", source_state_id: "
            + quoted(condition["source_state_id"])
            + ", condition: "
            + quoted(condition["condition"])
            + ", output: "
            + quoted(condition["output"])
            + ", context_state: ("
            + quoted(condition["context_state_id"])
            + ")"
            + ", automatic_trigger_time: "
            + (
                "None"
                if condition["automatic_trigger_time"] is None
                else "Some(" + repr(condition["automatic_trigger_time"]) + ")"
            )
            + ", transition_duration: "
            + repr(condition["transition_duration"])
            + ", always_reset_target_on_entry: Some("
            + str(condition["always_reset_target_on_entry"]).lower()
            + ")"
            + ", gates: ["
            + ", ".join(
                "(condition: "
                + quoted(gate["condition"])
                + ", output: "
                + quoted(gate["output"])
                + ", context_state: ("
                + quoted(gate["context_state_id"])
                + ")"
                + ")"
                for gate in condition.get("gates", [])
            )
            + "]"
            + ", notifications: ["
            + ", ".join(
                "(phase: "
                + quoted(notification["phase"])
                + ", name: "
                + quoted(notification["name"])
                + ")"
                for notification in condition.get("notifications", [])
            )
            + "]"
            + ")"
            for condition in conditions
        )
        entry_conditions_ron = ", ".join(
            "(target_state_id: "
            + quoted(condition["target_state_id"])
            + ", gates: ["
            + ", ".join(
                "(condition: "
                + quoted(gate["condition"])
                + ", output: "
                + quoted(gate["output"])
                + ", context_state: ("
                + quoted(gate["context_state_id"])
                + ")"
                + ")"
                for gate in condition.get("gates", [])
            )
            + "])"
            for condition in entry_conditions
        )
        return (
            "als_ext::InterruptibleFsmNode",
            "(fsm: "
            + quoted(config["path"])
            + ", state_pose_output: \"Result\", outer_pose_output: \"Pose\", state_aliases: ["
            + alias_ron
            + "], source_transition_conditions: ["
            + conditions_ron
            + "], source_entry_conditions: ["
            + entry_conditions_ron
            + "], internal_time_key: \"animation.delta_seconds\", max_transitions_per_frame: "
            + repr(max_transitions_per_frame)
            + ", max_transition_requests: "
            + repr(max_transition_requests)
            + ", skip_first_update_transition: "
            + bool_ron(skip_first_update_transition)
            + ", reinitialize_on_becoming_relevant: "
            + bool_ron(reinitialize_on_becoming_relevant)
            + ", expose_telemetry: false)",
        )
    if renderer == "multi_way_blend":
        keys = value.get("desired_alpha_keys")
        if not isinstance(keys, list) or len(keys) != value["pose_count"] or not all(
            isinstance(key, str) for key in keys
        ):
            raise PoseRenderError("source multi-way blend property binding is unavailable")
        return (
            "als_ext::SourceMultiWayBlendNode",
            "(pose_count: "
            + repr(value["pose_count"])
            + ", desired_alpha_keys: ["
            + ", ".join(quoted(key) for key in keys)
            + "], desired_alpha_defaults: ["
            + ", ".join(repr(default) for default in value["desired_alpha_defaults"])
            + "], normalize_alpha: "
            + bool_ron(value["normalize_alpha"])
            + ", alpha_scale: "
            + repr(value["alpha_scale"])
            + ", pose_pin_prefix: \"Poses_\", alpha_pin_prefix: \"DesiredAlphas_\")",
        )
    if renderer == "set_hips_direction":
        return (
            "als_ext::SourceSetHipsDirectionNode",
            "(new_hips_direction: "
            + quoted(value["new_hips_direction"])
            + ", on_become_relevant: "
            + bool_ron(value["on_become_relevant"])
            + ", call_site: "
            + quoted(value["call_site"])
            + ")",
        )
    if renderer == "enum_pose_blend":
        active_key = value.get("active_key")
        selected_enum_value = value.get("selected_enum_value")
        if not isinstance(active_key, str) or not isinstance(selected_enum_value, (int, float)):
            raise PoseRenderError("source enum pose blend property binding is unavailable")
        fields = [
            field("active_key", quoted(active_key)),
            field("selected_enum_value", repr(selected_enum_value)),
            field("active_enum_default", repr(value["active_enum_default"])),
            field("default_blend_time", repr(value["default_blend_time"])),
            field("selected_blend_time", repr(value["selected_blend_time"])),
            field("default_pose_pin", quoted(value["default_pose_pin"])),
            field("selected_pose_pin", quoted(value["selected_pose_pin"])),
            field("default_blend_time_pin", quoted(value["default_blend_time_pin"])),
            field("selected_blend_time_pin", quoted(value["selected_blend_time_pin"])),
            field("active_enum_pin", quoted(value["active_enum_pin"])),
            field("output_pin", quoted(value["output_pin"])),
            field("source_visible_enum_entry", quoted(value["selected_enum_entry"])),
        ]
        return "als_ext::SourceEnumPoseBlendNode", "(" + ", ".join(fields) + ")"
    if renderer == "layered_bone_blend":
        filters = value.get("branch_filter_layers")
        weights = value.get("blend_weights")
        pins = value.get("blend_pose_pins")
        if (
            not isinstance(filters, list)
            or not isinstance(weights, list)
            or not isinstance(pins, list)
            or len(filters) != len(weights)
            or len(filters) != len(pins)
            or not all(
                isinstance(filter, dict)
                and set(filter) == {"bone", "blend_depth"}
                and isinstance(filter["bone"], str)
                and isinstance(filter["blend_depth"], int)
                and not isinstance(filter["blend_depth"], bool)
                for layer in filters
                for filter in layer
            )
            or not all(isinstance(weight, (int, float)) for weight in weights)
            or not all(isinstance(pin, str) for pin in pins)
        ):
            raise PoseRenderError("source layered bone blend contract is invalid")
        filters_ron = "[" + ", ".join(
            "["
            + ", ".join(
                "(bone: " + quoted(filter["bone"]) + ", blend_depth: " + repr(filter["blend_depth"]) + ")"
                for filter in layer
            )
            + "]"
            for layer in filters
        ) + "]"
        fields = [
            field("branch_filter_layers", filters_ron),
            field("blend_weights", "[" + ", ".join(repr(weight) for weight in weights) + "]"),
            field("blend_weight_pin_prefix", quoted("BlendWeights_")),
            field("expose_blend_weight_pins", "true"),
            field("mesh_space_rotation_blend", bool_ron(value["mesh_space_rotation_blend"])),
            field("blend_root_motion_based_on_root_bone", bool_ron(value["blend_root_motion_based_on_root_bone"])),
            field("base_pose_pin", quoted(value["base_pose_pin"])),
            field("blend_pose_pins", "[" + ", ".join(quoted(pin) for pin in pins) + "]"),
            field("output_pin", quoted(value["output_pin"])),
        ]
        return "als_ext::SourceLayeredBoneBlendNode", "(" + ", ".join(fields) + ")"
    if renderer == "two_way_blend":
        pins = input_names(value)
        alpha_key = value.get("alpha_key")
        reset_child_on_activation = value.get("reset_child_on_activation", False)
        if not isinstance(alpha_key, str) or not isinstance(reset_child_on_activation, bool):
            raise PoseRenderError("source blend property binding is unavailable")
        fields = [
            field("alpha_key", quoted(alpha_key)),
            field("alpha_pin", quoted("Alpha")),
            field("expose_alpha_pin", bool_ron("Alpha" in pins)),
            field("pin_default", repr(value["alpha_default"])),
            field("mapping_max", "0.0"),
            field("mapping", mapping_ron(value["alpha_mapping"])),
            field("alpha_scale", repr(value["alpha_scale_bias"]["scale"])),
            field("alpha_bias", repr(value["alpha_scale_bias"]["bias"])),
            field("increasing_speed", "0.0"),
            field("decreasing_speed", "0.0"),
            field("reset_child_on_activation", bool_ron(reset_child_on_activation)),
        ]
        return "als_ext::SourceTwoWayBlendNode", "(" + ", ".join(fields) + ")"
    if renderer in {"apply_additive", "apply_mesh_space_additive"}:
        pins = input_names(value)
        alpha_key = value.get("alpha_key")
        if not isinstance(alpha_key, str):
            raise PoseRenderError("source additive property binding is unavailable")
        fields = [
            field("alpha_key", quoted(alpha_key)),
            field("alpha_pin", quoted("Alpha")),
            field("expose_alpha_pin", bool_ron("Alpha" in pins)),
            field("use_constant_alpha", bool_ron("Alpha" not in pins)),
            field("constant_alpha", repr(value["alpha_default"])),
            field("mapping", mapping_ron(value["alpha_mapping"])),
            field("alpha_scale", repr(value["alpha_scale_bias"]["scale"])),
            field("alpha_bias", repr(value["alpha_scale_bias"]["bias"])),
            field("minimum_alpha", "0.0"),
            field("maximum_alpha", "1.0"),
            field("increasing_speed", "0.0"),
            field("decreasing_speed", "0.0"),
            field("expose_source_alpha_controls", "false"),
            field("alpha_curve_name", quoted("None")),
            field("alpha_bool_enabled", "true"),
        ]
        node_type = (
            "als_ext::SourceApplyAdditiveNode"
            if renderer == "apply_additive"
            else "als_ext::SourceMeshSpaceAdditiveNode"
        )
        return node_type, "(" + ", ".join(fields) + ")"
    if renderer == "make_dynamic_additive":
        mesh_space = value.get("mesh_space_additive")
        if not isinstance(mesh_space, bool):
            raise PoseRenderError("source dynamic additive space is unavailable")
        return (
            "als_ext::SourceMakeDynamicAdditiveNode",
            "(mesh_space_additive: " + bool_ron(mesh_space) + ")",
        )
    if renderer == "curves_blend":
        blend_amount = value.get("blend_amount")
        blend_mode = value.get("blend_mode")
        if (
            not isinstance(blend_amount, (int, float))
            or isinstance(blend_amount, bool)
            or not 0.0 <= blend_amount <= 1.0
            or blend_mode not in {"Accumulate", "Override"}
        ):
            raise PoseRenderError("source curves blend contract is unavailable")
        return (
            "als_ext::SourceCurvesBlendNode",
            "(blend_amount: " + repr(blend_amount) + ", blend_mode: " + blend_mode + ")",
        )
    if renderer == "modify_curve":
        pins = input_names(value)
        source_pose_pin = value.get("source_pose_pin")
        value_pins = [pin for pin in pins if pin not in {"SourcePose", "Alpha"}]
        value_keys = value.get("curve_value_keys")
        value_paths = value.get("curve_value_paths")
        if (
            not isinstance(value_keys, list)
            or not isinstance(value_paths, list)
            or not isinstance(source_pose_pin, str)
            or not source_pose_pin
            or len(value_keys) != len(value["curve_names"])
            or len(value_paths) != len(value["curve_names"])
            or not all(isinstance(item, str) for item in [*value_keys, *value_paths])
        ):
            raise PoseRenderError("source curve property binding is unavailable")
        fields = [
            field("source_pose_pin", quoted(source_pose_pin)),
            field("curve_name", quoted("")),
            field("curve_names", "[" + ", ".join(quoted(name) for name in value["curve_names"]) + "]"),
            field("value", "0.0"),
            field("pin_default", "0.0"),
            field("pin_defaults", "[" + ", ".join(repr(item) for item in value["curve_values"]) + "]"),
            field("value_pin_names", "[" + ", ".join(quoted(name) for name in value_pins) + "]"),
            field("value_key", quoted("")),
            field("value_keys", "[" + ", ".join(quoted(key) for key in value_keys) + "]"),
            field("source_value_path", quoted("")),
            field("source_value_paths", "[" + ", ".join(quoted(path) for path in value_paths) + "]"),
            field("alpha", repr(value["alpha"])),
            field("alpha_key", quoted(value.get("alpha_key", ""))),
            field("alpha_pin", quoted("Alpha")),
            field("expose_alpha_pin", bool_ron(value["alpha_visible"])),
            field("apply_mode", quoted(value["apply_mode"])),
        ]
        return "als_ext::SourceCurveModifyNode", "(" + ", ".join(fields) + ")"
    if renderer == "state_result":
        lifecycle = value.get("lifecycle")
        if not isinstance(lifecycle, list) or len(lifecycle) > 1:
            raise PoseRenderError("source state lifecycle form is unsupported")
        source_function = ""
        call_site = ""
        if lifecycle:
            source_function = lifecycle[0].get("source_function", "")
            call_site = lifecycle[0].get("call_site", "")
        return (
            "als_ext::SourcePosePassthroughNode",
            "(input: \"Source\", output: \"Pose\", input_time: \"source_time\", output_time: false, source_function: "
            + quoted(source_function)
            + ", call_site: "
            + quoted(call_site)
            + ", synchronize_groups: " + bool_ron(value.get("synchronize_groups", False))
            + ")",
        )
    if renderer == "root_output":
        return (
            "als_ext::SourcePosePassthroughNode",
            "(input: \"Source\", output: \"Pose\", input_time: \"source_time\", output_time: false, source_function: \"\", call_site: \"\", synchronize_groups: " + bool_ron((value or {}).get("synchronize_groups", False)) + ")",
        )
    if renderer == "save_cached_pose":
        return (
            "als_ext::SourceCachedPoseSaveNode",
            "(cache_key: " + quoted(cache_key(graph_contract, value["cache_name"])) + ")",
        )
    if renderer == "use_cached_pose":
        name = value["cache_name"]
        return (
            "als_ext::SourceScopedCachedPoseUseNode",
            "(cache_node: "
            + quoted("Save cached pose '" + name + "'")
            + ", cache_key: "
            + quoted(cache_key(graph_contract, name))
            + ", context_defaults: [])",
        )
    if renderer == "linked_input_pose":
        return (
            "als_ext::SourceLinkedInputPoseNode",
            "(input: "
            + quoted(value["input_name"])
            + ", output: \"Pose\", context_curve_prefix: \"\", context_curve_names: [])",
        )
    if renderer == "linked_anim_graph":
        target = value.get("target_graph")
        if not isinstance(target, str) or not target:
            raise PoseRenderError("source linked animation graph is unavailable")
        return (
            "als_ext::LayeredGraphNode",
            "(graph: "
            + quoted(target)
            + ", input_aliases: [], output_aliases: [(child_output: \"Result\", outer_output: \"Pose\")], capture_context: true, expose_context_curves: false, exposed_output_data: [], internalize_time: true)",
        )
    if renderer == "linked_anim_layer":
        target = value.get("target_graph")
        if not isinstance(target, str) or not target:
            raise PoseRenderError("source linked animation layer graph is unavailable")
        if value.get("external_instance"):
            return (
                "als_ext::LayeredGraphNode",
                "(graph: "
                + quoted(target)
                + ", input_aliases: [], output_aliases: [(child_output: \"Result\", outer_output: \"Pose\")], capture_context: true, expose_context_curves: false, exposed_output_data: [], internalize_time: true)",
            )
        return (
            "als_ext::LinkedAnimLayerNode",
            "(interface: "
            + quoted(target)
            + ", implementations: ["
            + quoted(target)
            + "], selector: \"layer_index\")",
        )
    if renderer == "control_rig":
        return (
            "als_ext::ControlRigBoundaryNode",
            "(rig_class: "
            + quoted(value["rig_class"])
            + ", alpha_enabled: "
            + bool_ron(value["alpha_enabled"])
            + ", transfer_input_pose: "
            + bool_ron(value["transfer_input_pose"])
            + ", transfer_input_curves: "
            + bool_ron(value["transfer_input_curves"])
            + ", transfer_pose_in_global_space: "
            + bool_ron(value["transfer_pose_in_global_space"])
            + ", pose_input: \"Source\", curve_names: [], capture_context: "
            + bool_ron(value["transfer_input_curves"])
            + ", expose_context_curves: false, exposed_output_data: [], missing_curve_value: None, internalize_time: true)",
        )
    if renderer == "inertialization":
        return (
            "als_ext::InertializationPassthroughNode",
            "(reset_on_becoming_relevant: "
            + bool_ron(value["reset_on_becoming_relevant"])
            + ", forward_requests_through_skipped_cached_poses: "
            + bool_ron(value["forward_requests_through_skipped_cached_poses"])
            + ", filtered_curves: ["
            + ", ".join(quoted(curve) for curve in value["filtered_curves"])
            + "], filtered_bones: [], curve_names: [], capture_context: false, expose_context_curves: false, exposed_output_data: [], missing_curve_value: None, internalize_time: true, output: \"Pose\", request_epoch_key: \"\", request_duration_key: \"\")",
        )
    if renderer == "pose_lifecycle_call":
        return (
            "als_ext::SourcePosePassthroughNode",
            "(input: \"Source\", output: \"Pose\", input_time: \"source_time\", output_time: false, source_function: "
            + quoted(value["source_function"])
            + ", call_site: "
            + quoted(value["call_site"])
            + ")",
        )
    raise PoseRenderError("source pose renderer is unsupported")


def scalar_pin_name(node, source_pin):
    pin = next((pin for pin in node.get("pins", []) if pin.get("name") == source_pin), None)
    if pin is None:
        raise PoseRenderError("source scalar pin is unavailable")
    return scalar_renderer.source_pin_label(pin)


def target_pin(renderer, source_pin, node=None):
    if renderer in SCALAR_RENDERERS:
        if node is None:
            raise PoseRenderError("source scalar node is unavailable")
        return scalar_pin_name(node, source_pin)
    if renderer in {
        "sequence_player",
        "sequence_evaluator",
        "modify_curve",
        "multi_way_blend",
        "gameplay_tag_blend",
        "enum_pose_blend",
        "layered_bone_blend",
    }:
        return source_pin
    if renderer == "state_result":
        return {"Result": "Source"}.get(source_pin, source_pin)
    if renderer == "root_output":
        return {"Result": "Source"}.get(source_pin, source_pin)
    if renderer == "linked_anim_layer":
        return {"Pose": "Pose"}.get(source_pin, source_pin)
    if renderer == "control_rig":
        return {"Source": "Source", "Pose": "Pose"}.get(source_pin, source_pin)
    if renderer in {"blendspace_evaluator", "blendspace_player"}:
        return {"Pose": "pose"}.get(source_pin, source_pin)
    if renderer == "state_machine":
        return {"Pose": "Pose"}.get(source_pin, source_pin)
    return source_pin


def blendspace_helpers(graph_contract, raw, semantic):
    renderer = semantic.get("renderer")
    player = renderer == "blendspace_player"
    value = semantic.get("semantic")
    if not isinstance(value, dict):
        raise PoseRenderError("source blendspace renderer semantic is invalid")
    blendspace = value.get("blendspace")
    inputs = value.get("input_pins")
    if not isinstance(blendspace, dict) or not isinstance(inputs, list):
        raise PoseRenderError("source blendspace input contract is invalid")
    binding_keys = value.get("input_binding_keys")
    if not isinstance(binding_keys, list) or len(binding_keys) != len(inputs) or not all(
        isinstance(key, str) for key in binding_keys
    ):
        raise PoseRenderError("source blendspace property binding is unavailable")
    if player:
        axis_entries = list(enumerate(inputs[:2]))
        phase_entry = None
        if [item.get("name") for item in inputs] != ["X", "Y", "PlayRate"]:
            raise PoseRenderError("source blendspace input schema is unsupported")
    else:
        input_names = [item.get("name") for item in inputs]
        axis_entries = [
            (index, item)
            for index, item in enumerate(inputs)
            if isinstance(item.get("axis"), str)
        ]
        phase_entries = [
            (index, item)
            for index, item in enumerate(inputs)
            if item.get("name") == "NormalizedTime" and item.get("axis") is None
        ]
        if (
            [item.get("name") for _, item in axis_entries] not in (["X"], ["X", "Y"])
            or len(phase_entries) > 1
            or len(axis_entries) + len(phase_entries) != len(inputs)
        ):
            raise PoseRenderError("source blendspace evaluator input schema is unsupported")
        phase_entry = phase_entries[0] if phase_entries else None
    primary_id = node_id(raw)
    source_path = raw["object_path"]
    x, y = position(raw)
    axis_input_x = x - (288.0 if player else 192.0)
    linked_inputs = {
        (link.get("target", {}).get("node_object_path"), link.get("target", {}).get("pin_name"))
        for link in graph_contract.get("links", [])
    }
    vector_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/vector/" + source_path))
    phase_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/phase/" + source_path))
    nodes = [
        (
            vector_id,
            "Blendspace position",
            "()",
            "als_ext::F32ToVec2Node",
            (x - (192.0 if player else 96.0), y),
        ),
    ]
    edges = []
    if player:
        phase_key = value.get("sync_phase_key")
        delta_seconds_key = value.get("delta_seconds_key")
        axis_filter = value.get("axis_filter")
        if (
            not isinstance(phase_key, str)
            or not phase_key
            or not isinstance(delta_seconds_key, str)
            or not delta_seconds_key
            or not isinstance(axis_filter, dict)
            or not all(
                isinstance(axis_filter.get(key), (int, float)) and axis_filter[key] >= 0.0
                for key in ("x_window_seconds", "y_window_seconds")
            )
        ):
            raise PoseRenderError("source blendspace player phase contract is unavailable")
        filter_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/filter/" + source_path))
        delta_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/delta/" + source_path))
        nodes.extend(
            [
                (
                    filter_id,
                    "Blendspace cubic axis filter",
                    "(x_window_seconds: "
                    + repr(axis_filter["x_window_seconds"])
                    + ", y_window_seconds: "
                    + repr(axis_filter["y_window_seconds"])
                    + ")",
                    "als_ext::CubicVec2AxisFilterNode",
                    (x - 96.0, y),
                ),
                (
                    delta_id,
                    "Animation delta seconds",
                    "(key: " + quoted(delta_seconds_key) + ")",
                    "als_ext::GlobalF32InputNode",
                    (x - 96.0, y + 144.0),
                ),
            ]
        )
        edges.extend(
            [
                f"NodeData({quoted(filter_id)}, \"position\"): NodeData({quoted(vector_id)}, \"value\")",
                f"NodeData({quoted(filter_id)}, \"delta_seconds\"): NodeData({quoted(delta_id)}, \"{delta_seconds_key}\")",
                f"NodeData({quoted(primary_id)}, \"position\"): NodeData({quoted(filter_id)}, \"value\")",
            ]
        )
    else:
        phase_key = ""
        phase_default = value["normalized_time"]
        if phase_entry is not None:
            phase_index, phase_input = phase_entry
            if (source_path, phase_input["name"]) in linked_inputs:
                raise PoseRenderError("source blendspace evaluator linked normalized time is unsupported")
            phase_key = binding_keys[phase_index]
            phase_default = phase_input["default_value"]
        nodes.append(
            (
                phase_id,
                "Blendspace normalized time",
                (
                    "(key: " + quoted(phase_key) + ")"
                    if phase_key
                    else "(value: " + repr(phase_default) + ")"
                ),
                "als_ext::GlobalF32InputNode" if phase_key else "als_ext::ConstantF32Node",
                (x - 96.0, y + 96.0),
            )
        )
        phase_output = phase_key or "value"
        edges.extend(
            [
                f"NodeData({quoted(primary_id)}, \"position\"): NodeData({quoted(vector_id)}, \"value\")",
                f"NodeData({quoted(primary_id)}, \"phase\"): NodeData({quoted(phase_id)}, {quoted(phase_output)})",
            ]
        )
    targets = {}
    for axis_index, axis in enumerate(("x", "y")):
        if axis_index >= len(axis_entries):
            default_id = str(
                uuid.uuid5(
                    uuid.NAMESPACE_URL,
                    "bevy-als/blendspace/default/" + source_path + "/" + axis,
                )
            )
            nodes.append(
                (
                    default_id,
                    "Blendspace " + axis.upper() + " default",
                    "(value: 0.0)",
                    "als_ext::ConstantF32Node",
                    (axis_input_x, y + (0.0 if axis == "x" else 48.0)),
                )
            )
            edges.append(
                f"NodeData({quoted(vector_id)}, {quoted(axis)}): NodeData({quoted(default_id)}, \"value\")"
            )
            continue
        input_index, input_value = axis_entries[axis_index]
        binding_key = binding_keys[input_index]
        pin_name = input_value["name"]
        targets[(source_path, pin_name)] = (vector_id, axis)
        if (source_path, pin_name) not in linked_inputs:
            default_id = str(
                uuid.uuid5(
                    uuid.NAMESPACE_URL,
                    "bevy-als/blendspace/default/" + source_path + "/" + pin_name,
                )
            )
            if binding_key:
                nodes.append(
                    (
                        default_id,
                        "Blendspace " + pin_name + " binding",
                        "(key: " + quoted(binding_key) + ")",
                        "als_ext::GlobalF32InputNode",
                        (axis_input_x, y + (0.0 if axis == "x" else 48.0)),
                    )
                )
                source_pin = binding_key
            else:
                nodes.append(
                    (
                        default_id,
                        "Blendspace " + pin_name + " default",
                        "(value: " + repr(input_value["default_value"]) + ")",
                        "als_ext::ConstantF32Node",
                        (axis_input_x, y + (0.0 if axis == "x" else 48.0)),
                    )
                )
                source_pin = "value"
            edges.append(
                f"NodeData({quoted(vector_id)}, {quoted(axis)}): NodeData({quoted(default_id)}, {quoted(source_pin)})"
            )
    for index, sample in enumerate(blendspace.get("samples", [])):
        target = sample.get("target_clip") if isinstance(sample, dict) else None
        if not isinstance(target, str) or not target:
            raise PoseRenderError("source blendspace target clip is invalid")
        clip_id = str(
            uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/sample/" + source_path + f"/{index}")
        )
        nodes.append(
            (
                clip_id,
                "Blendspace sample " + str(index),
                "(clip: " + quoted(target) + ", override_duration: None, override_interpolation: None)",
                "bevy_animation_graph::builtin_nodes::ClipNode",
                (x - 192.0, y + 160.0 + index * 64.0),
            )
        )
        point = f"sample_{index}"
        edges.extend(
            [
                f"NodeData({quoted(primary_id)}, {quoted('pose_' + point)}): NodeData({quoted(clip_id)}, \"pose\")",
                f"NodeTime({quoted(primary_id)}, {quoted('time_' + point)}): NodeTime({quoted(clip_id)})",
            ]
        )
    return nodes, edges, targets


def layered_bone_weight_helpers(graph_contract, raw, semantic):
    value = semantic.get("semantic")
    if not isinstance(value, dict):
        raise PoseRenderError("source layered bone blend contract is unavailable")
    defaults = value.get("blend_weights")
    keys = value.get("blend_weight_keys")
    pins = value.get("blend_weight_pins")
    if (
        not isinstance(defaults, list)
        or not isinstance(keys, list)
        or not isinstance(pins, list)
        or len(defaults) != len(keys)
        or len(defaults) != len(pins)
        or not all(isinstance(default, (int, float)) and not isinstance(default, bool) for default in defaults)
        or not all(isinstance(key, str) for key in keys)
        or not all(isinstance(pin, str) and pin for pin in pins)
    ):
        raise PoseRenderError("source layered bone blend weight contract is unavailable")
    source_path = raw["object_path"]
    primary_id = node_id(raw)
    x, y = position(raw)
    linked_inputs = {
        (link.get("target", {}).get("node_object_path"), link.get("target", {}).get("pin_name"))
        for link in graph_contract.get("links", [])
    }
    nodes = []
    edges = []
    for index, (default, key, pin) in enumerate(zip(defaults, keys, pins)):
        if (source_path, pin) in linked_inputs:
            continue
        identifier = str(
            uuid.uuid5(
                uuid.NAMESPACE_URL,
                "bevy-als/layered-bone-blend/weight/" + source_path + "/" + pin,
            )
        )
        if key:
            inner = "(key: " + quoted(key) + ")"
            node_type = "als_ext::GlobalF32InputNode"
            source_pin = key
            name = "Layered blend " + pin + " binding"
        else:
            inner = "(value: " + repr(default) + ")"
            node_type = "als_ext::ConstantF32Node"
            source_pin = "value"
            name = "Layered blend " + pin + " default"
        nodes.append((identifier, name, inner, node_type, (x - 96.0, y + 96.0 + index * 48.0)))
        edges.append(
            f"NodeData({quoted(primary_id)}, {quoted(pin)}): NodeData({quoted(identifier)}, {quoted(source_pin)})"
        )
    return nodes, edges


def collapsed_links(graph_contract, ir_nodes, links=None):
    links = graph_contract.get("links", []) if links is None else links
    reroutes = {
        path
        for path, semantic in ir_nodes.items()
        if semantic.get("renderer") == "reroute"
    }
    incoming = {}
    for link in links:
        target = link.get("target", {})
        target_path = target.get("node_object_path")
        if target_path in reroutes:
            incoming.setdefault(target_path, []).append(link)

    def resolve(source, seen):
        path = source.get("node_object_path")
        if path not in reroutes:
            return source
        if path in seen:
            raise PoseRenderError("source reroute cycle is unsupported")
        links = incoming.get(path, [])
        if len(links) != 1:
            raise PoseRenderError("source reroute input is unsupported")
        return resolve(links[0].get("source", {}), seen | {path})

    result = []
    for link in links:
        target = link.get("target", {})
        if target.get("node_object_path") in reroutes:
            continue
        result.append({"source": resolve(link.get("source", {}), set()), "target": target})
    return result


def validate_source_two_way_child_reset(graph_contract, ir_nodes):
    reset_nodes = {
        source_path
        for source_path, semantic in ir_nodes.items()
        if semantic.get("renderer") == "two_way_blend"
        and isinstance(semantic.get("semantic"), dict)
        and semantic["semantic"].get("reset_child_on_activation", False)
    }
    reset_inputs = {}
    for link in collapsed_links(graph_contract, ir_nodes, graph_contract.get("links")):
        source = link.get("source", {})
        target = link.get("target", {})
        target_path = target.get("node_object_path")
        target_semantic = ir_nodes.get(target_path)
        if target_path not in reset_nodes:
            continue
        pin = target.get("pin_name")
        if pin not in {"A", "B"}:
            continue
        source_semantic = ir_nodes.get(source.get("node_object_path"))
        if (
            source_semantic is None
            or source_semantic.get("renderer") != "sequence_player"
            or source.get("pin_name") != "Pose"
            or pin in reset_inputs.get(target_path, {})
        ):
            raise PoseRenderError("source child reset requires one direct Sequence Player per A/B input")
        reset_inputs.setdefault(target_path, {})[pin] = source_semantic["source_node"]
    for target_path in sorted(reset_nodes):
        inputs = reset_inputs.get(target_path, {})
        if set(inputs) != {"A", "B"} or inputs["A"] == inputs["B"]:
            raise PoseRenderError("source child reset requires distinct direct Sequence Players for A and B")



def render(
    graph_contract,
    graph_ir,
    animation_assets,
    fsm_assets=None,
):
    raw_nodes = node_index(graph_contract)
    ir_nodes = semantic_index(graph_ir)
    if set(raw_nodes) < set(ir_nodes):
        raise PoseRenderError("source IR node is missing from source graph")
    for semantic in ir_nodes.values():
        if semantic.get("renderer") not in SUPPORTED_RENDERERS and semantic.get("renderer") != "comment_group":
            raise PoseRenderError("source pose graph contains an unsupported renderer")
    raw_by_id = {node_id(node): node for node in raw_nodes.values()}
    if len(raw_by_id) != len(raw_nodes):
        raise PoseRenderError("source node identifiers are duplicated")
    asset_by_node = asset_index(animation_assets)
    fsm_assets = fsm_assets or {}
    result_nodes = []
    linked_layer_selectors = {}
    blendspace_edges = []
    layered_bone_edges = []
    blendspace_targets = {}
    for source_path, semantic in ir_nodes.items():
        if semantic.get("renderer") in {"comment_group", "reroute"}:
            continue
        raw = raw_nodes[source_path]
        identifier = node_id(raw)
        node_type, inner = rendered_inner(
            graph_contract,
            raw,
            semantic,
            asset_by_node,
            fsm_assets,
        )
        result_nodes.append((identifier, source_title(raw), inner, node_type, position(raw)))
        if semantic.get("renderer") in {"blendspace_evaluator", "blendspace_player"}:
            nodes, edges, targets = blendspace_helpers(graph_contract, raw, semantic)
            result_nodes.extend(nodes)
            blendspace_edges.extend(edges)
            blendspace_targets.update(targets)
        if semantic.get("renderer") == "layered_bone_blend":
            nodes, edges = layered_bone_weight_helpers(graph_contract, raw, semantic)
            result_nodes.extend(nodes)
            layered_bone_edges.extend(edges)
        if (
            semantic.get("renderer") == "linked_anim_layer"
            and not semantic.get("semantic", {}).get("external_instance")
        ):
            selector_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/linked-layer/" + source_path))
            x, y = position(raw)
            result_nodes.append(
                (
                    selector_id,
                    "Linked layer selection",
                    "(value: 0.0)",
                    "als_ext::ConstantF32Node",
                    (x - 96.0, y + 96.0),
                )
            )
            linked_layer_selectors[source_path] = selector_id
    output_nodes = [
        node
        for node in ir_nodes.values()
        if node.get("renderer") in {"state_result", "root_output"}
    ]
    if len(output_nodes) != 1:
        raise PoseRenderError("source pose graph must have exactly one output node")
    state_result = output_nodes[0]
    state_result_raw = raw_nodes[state_result["source_node"]]
    output_name = "Result"
    edges = []
    for link in collapsed_links(graph_contract, ir_nodes, graph_ir.get("links")):
        source = link.get("source", {})
        target = link.get("target", {})
        source_semantic = ir_nodes.get(source.get("node_object_path"))
        target_semantic = ir_nodes.get(target.get("node_object_path"))
        if source_semantic is None or target_semantic is None:
            raise PoseRenderError(
                "source pose edge endpoint is unsupported: "
                + str(source.get("node_object_path"))
                + " -> "
                + str(target.get("node_object_path"))
            )
        source_identifier = node_id(raw_nodes[source_semantic["source_node"]])
        target_raw = raw_nodes[target_semantic["source_node"]]
        target_identifier, target_pin_name = blendspace_targets.get(
            (target_semantic["source_node"], target.get("pin_name")),
            (
                node_id(target_raw),
                target_pin(
                    target_semantic["renderer"],
                    target.get("pin_name"),
                    target_raw,
                ),
            ),
        )
        source_pin = (
            target_pin(
                source_semantic["renderer"],
                source.get("pin_name"),
                raw_nodes[source_semantic["source_node"]],
            )
        )
        edges.append(
            f"NodeData({quoted(target_identifier)}, {quoted(target_pin_name)}): NodeData({quoted(source_identifier)}, {quoted(source_pin)})"
        )
    validate_source_two_way_child_reset(graph_contract, ir_nodes)
    for source_path, selector_id in linked_layer_selectors.items():
        identifier = node_id(raw_nodes[source_path])
        edges.append(
            f"NodeData({quoted(identifier)}, \"layer_index\"): NodeData({quoted(selector_id)}, \"value\")"
        )
    edges.extend(blendspace_edges)
    edges.extend(layered_bone_edges)
    result_identifier = node_id(state_result_raw)
    edges.append(f"OutputData({quoted(output_name)}): NodeData({quoted(result_identifier)}, \"Pose\")")
    graph_inputs = []
    for semantic in ir_nodes.values():
        if semantic.get("renderer") != "linked_input_pose":
            continue
        value = semantic.get("semantic")
        input_name = value.get("input_name") if isinstance(value, dict) else None
        if not isinstance(input_name, str) or not input_name or input_name in graph_inputs:
            raise PoseRenderError("source linked input pose graph input is invalid")
        graph_inputs.append(input_name)
    input_data = "{ " + ", ".join(
        f"Passthrough({quoted(name)}): Pose" for name in graph_inputs
    ) + " }"
    input_order = "{ " + ", ".join(
        f"Data(Passthrough({quoted(name)})): {index}" for index, name in enumerate(graph_inputs)
    ) + " }"
    positions = ", ".join(
        f"{quoted(identifier)}: ({x:.3f}, {y:.3f})" for identifier, _, _, _, (x, y) in result_nodes
    )
    return "(\n" + "\n".join(
        (
            "    nodes: [",
            *[
                f"        (id: {quoted(identifier)}, name: {quoted(name)}, inner: {{ {quoted(node_type)}: {inner} }}, should_debug: false),"
                for identifier, name, inner, node_type, _ in result_nodes
            ],
            "    ],",
            "    edges_inverted: {",
            *[f"        {edge}," for edge in edges],
            "    },",
            "    io_spec: (input_times: [], input_data: " + input_data + ", output_time: false, output_data: { \"Result\": Pose }, input_order: " + input_order + ", output_order: { Data(\"Result\"): 0 }),",
            "    default_data: {},",
            "    editor_metadata: (node_positions: { " + positions + " }, input_position: (0.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),",
            ")",
            "",
        )
    )


def fixture_graph():
    sequence_path = "/Asset:State.Player"
    blend_path = "/Asset:State.Blend"
    result_path = "/Asset:State.Result"
    graph_contract = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": sequence_path,
                    "node_guid": "10000000-0000-4000-8000-000000000001",
                    "title": "Sequence Player",
                    "position_x": -256,
                    "position_y": 0,
                },
                {
                    "object_path": blend_path,
                    "node_guid": "10000000-0000-4000-8000-000000000002",
                    "title": "Two Way Blend",
                    "position_x": -128,
                    "position_y": 0,
                },
                {
                    "object_path": result_path,
                    "node_guid": "10000000-0000-4000-8000-000000000003",
                    "title": "Output Animation Pose",
                    "position_x": 0,
                    "position_y": 0,
                },
            ]
        },
        "links": [
            {"source": {"node_object_path": sequence_path, "pin_name": "Pose"}, "target": {"node_object_path": blend_path, "pin_name": "A"}},
            {"source": {"node_object_path": sequence_path, "pin_name": "Pose"}, "target": {"node_object_path": blend_path, "pin_name": "B"}},
            {"source": {"node_object_path": blend_path, "pin_name": "Pose"}, "target": {"node_object_path": result_path, "pin_name": "Result"}},
        ],
    }
    mapping = {
        "source_semantics": True,
        "map_range": False,
        "input_min": 0.0,
        "input_max": 1.0,
        "output_min": 0.0,
        "output_max": 1.0,
        "scale": 1.0,
        "bias": 0.0,
        "clamp_result": False,
        "clamp_min": 0.0,
        "clamp_max": 1.0,
        "interpolate_result": False,
        "interpolate_speed_increasing": 10.0,
        "interpolate_speed_decreasing": 10.0,
    }
    graph_ir = {
        "nodes": [
            {
                "source_node": sequence_path,
                "renderer": "sequence_player",
                "semantic": {
                    "input_pins": [],
                    "play_rate": 1.0,
                    "play_rate_key": "",
                    "play_rate_pin": "PlayRate",
                    "play_rate_basis": 1.0,
                    "play_rate_mapping": mapping,
                    "start_position": 0.0,
                    "start_position_pin": "StartPosition",
                    "loop_animation": True,
                    "loop_animation_key": "",
                    "loop_animation_pin": "bLoopAnimation",
                    "sync_group": "None",
                },
            },
            {
                "source_node": blend_path,
                "renderer": "two_way_blend",
                "semantic": {
                    "input_pins": ["A", "B", "Alpha"],
                    "alpha_default": 0.25,
                    "alpha_key": "",
                    "alpha_mapping": mapping,
                    "alpha_scale_bias": {"scale": 1.0, "bias": 0.0},
                },
            },
            {"source_node": result_path, "renderer": "state_result", "semantic": {"lifecycle": []}},
        ]
    }
    assets = [
        {
            "source_node": sequence_path,
            "category": "sequence",
            "target_asset": "animations/A.anim.ron",
            "source_timing": {"duration": 1.0, "markers": []},
        }
    ]
    return graph_contract, graph_ir, assets


def run_self_test():
    curves = json.loads((Path(__file__).parent / "fixtures" / "ue_standing_walk_forward_curves.json").read_text())["curves"]
    raw = copy.deepcopy(curves)
    rendered_curves = pose_curves_ron(curves)
    assert '"FootPlanted"' in rendered_curves and '"PoseGait"' in rendered_curves
    assert "interps: [Linear" in rendered_curves and curves == raw
    yaw = copy.deepcopy(curves[0])
    yaw["name"] = "RotationYawSpeed"
    yaw["values"][0] = 90.0
    assert "values: [-90.0," in pose_curves_ron([yaw])
    yaw["flags"] |= 64
    assert pose_curves_ron([yaw]) == "({})"
    invalid = copy.deepcopy(curves)
    invalid[0]["values"].pop()
    try:
        pose_curves_ron(invalid)
    except PoseRenderError:
        pass
    else:
        raise AssertionError("invalid source curve key arrays were accepted")
    graph_contract, graph_ir, assets = fixture_graph()
    order_contract, order_ir, order_assets = fixture_graph()
    order_contract["source_graph"]["nodes"][0]["node_guid"] = "ffffffff-0000-4000-8000-000000000001"
    order_text = render(order_contract, order_ir, order_assets).split("edges_inverted:", 1)[0]
    assert order_text.index("ffffffff-0000-4000-8000-000000000001") < order_text.index("10000000-0000-4000-8000-000000000002"), "source node order must not be replaced with GUID order"
    try:
        blendspace_inner(None)
    except PoseRenderError:
        pass
    else:
        raise AssertionError("missing blendspace contract was accepted")
    comment_path = "/Asset:State.Comment"
    graph_contract["source_graph"]["nodes"].append(
        {
            "object_path": comment_path,
            "node_guid": "10000000-0000-4000-8000-000000000004",
            "title": "Comment",
            "position_x": 0,
            "position_y": 64,
        }
    )
    graph_ir["nodes"].append({"source_node": comment_path, "renderer": "comment_group"})
    text = render(graph_contract, graph_ir, assets)
    assert '"als_ext::SourceSequencePlayerNode"' in text
    assert '"als_ext::SourceTwoWayBlendNode"' in text
    assert 'NodeData("10000000-0000-4000-8000-000000000002", "A")' in text
    assert 'NodeTime("10000000-0000-4000-8000-000000000002", "A")' not in text
    assert 'OutputData("Result")' in text
    graph_ir["nodes"][1]["semantic"]["reset_child_on_activation"] = True
    try:
        render(graph_contract, graph_ir, assets)
    except PoseRenderError:
        pass
    else:
        raise AssertionError("child reset accepted one Sequence Player for both inputs")
    reset_player_path = "/Asset:State.ResetPlayer"
    graph_contract["source_graph"]["nodes"].append(
        {
            "object_path": reset_player_path,
            "node_guid": "10000000-0000-4000-8000-000000000006",
            "title": "Sequence Player B",
            "position_x": -256,
            "position_y": 96,
        }
    )
    graph_ir["nodes"].append(
        {
            "source_node": reset_player_path,
            "renderer": "sequence_player",
            "semantic": dict(graph_ir["nodes"][0]["semantic"]),
        }
    )
    assets.append(
        {
            "source_node": reset_player_path,
            "category": "sequence",
            "target_asset": "animations/B.anim.ron",
            "source_timing": {"duration": 1.0, "markers": []},
        }
    )
    graph_contract["links"][1]["source"]["node_object_path"] = reset_player_path
    text = render(graph_contract, graph_ir, assets)
    assert "reset_child_on_activation: true" in text
    assert 'NodeTime("10000000-0000-4000-8000-000000000002", "A")' not in text
    assert 'NodeTime("10000000-0000-4000-8000-000000000002", "B")' not in text
    graph_ir["links"] = list(graph_contract["links"])
    unused_path = "/Asset:State.Unreachable"
    graph_contract["source_graph"]["nodes"].append(
        {
            "object_path": unused_path,
            "node_guid": "10000000-0000-4000-8000-000000000090",
            "title": "Unreachable",
            "position_x": 64,
            "position_y": 64,
            "pins": [],
        }
    )
    graph_contract["links"].append(
        {
            "source": {"node_object_path": graph_ir["nodes"][0]["source_node"], "pin_name": "Pose"},
            "target": {"node_object_path": unused_path, "pin_name": "Pose"},
        }
    )
    assert 'OutputData("Result")' in render(graph_contract, graph_ir, assets)
    linked_path = "/Asset:AnimGraph.Layer"
    linked_result = "/Asset:AnimGraph.LayerResult"
    linked_contract = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": linked_path,
                    "node_guid": "10000000-0000-4000-8000-000000000031",
                    "title": "ALI_Test - Overlay\nLinked Anim Layer",
                    "position_x": -128,
                    "position_y": 0,
                    "pins": [],
                },
                {
                    "object_path": linked_result,
                    "node_guid": "10000000-0000-4000-8000-000000000032",
                    "title": "Output Pose\nAnimGraph",
                    "position_x": 0,
                    "position_y": 0,
                    "pins": [],
                },
            ]
        },
        "links": [
            {
                "source": {"node_object_path": linked_path, "pin_name": "Pose"},
                "target": {"node_object_path": linked_result, "pin_name": "Result"},
            }
        ],
    }
    linked_ir = {
        "nodes": [
            {
                "source_node": linked_path,
                "renderer": "linked_anim_layer",
                "semantic": {"target_graph": "graphs/overlay.animgraph.ron"},
            },
            {"source_node": linked_result, "renderer": "root_output", "semantic": {}},
        ]
    }
    text = render(linked_contract, linked_ir, [])
    assert '"als_ext::LinkedAnimLayerNode"' in text
    assert 'implementations: ["graphs/overlay.animgraph.ron"]' in text
    assert 'name: "Linked layer selection"' in text
    assert 'name: "Comment"' not in text
    linked_ir["nodes"][0]["semantic"]["external_instance"] = True
    text = render(linked_contract, linked_ir, [])
    assert '"als_ext::LayeredGraphNode"' in text
    assert 'name: "Linked layer selection"' not in text
    assert 'NodeData("10000000-0000-4000-8000-000000000032", "Source"): NodeData("10000000-0000-4000-8000-000000000031", "Pose")' in text
    linked_ir["nodes"][0] = {
        "source_node": linked_path,
        "renderer": "linked_anim_graph",
        "semantic": {"target_graph": "graphs/child.animgraph.ron"},
    }
    text = render(linked_contract, linked_ir, [])
    assert '"als_ext::LayeredGraphNode"' in text
    assert 'child_output: "Result", outer_output: "Pose"' in text
    input_path = "/Asset:AnimGraph.Input"
    input_contract = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": input_path,
                    "node_guid": "10000000-0000-4000-8000-000000000091",
                    "title": "Grounded Input",
                    "position_x": -128,
                    "position_y": 0,
                    "pins": [],
                },
                {
                    "object_path": linked_result,
                    "node_guid": "10000000-0000-4000-8000-000000000092",
                    "title": "Output Animation Pose",
                    "position_x": 0,
                    "position_y": 0,
                    "pins": [],
                },
            ]
        },
        "links": [
            {
                "source": {"node_object_path": input_path, "pin_name": "Pose"},
                "target": {"node_object_path": linked_result, "pin_name": "Result"},
            }
        ],
    }
    input_ir = {
        "nodes": [
            {
                "source_node": input_path,
                "renderer": "linked_input_pose",
                "semantic": {"input_name": "Grounded Input"},
            },
            {"source_node": linked_result, "renderer": "root_output", "semantic": {}},
        ]
    }
    text = render(input_contract, input_ir, [])
    assert 'input_data: { Passthrough("Grounded Input"): Pose }' in text
    assert 'Data(Passthrough("Grounded Input")): 0' in text
    reroute_path = "/Asset:State.Reroute"
    graph_contract["source_graph"]["nodes"].append(
        {
            "object_path": reroute_path,
            "node_guid": "10000000-0000-4000-8000-000000000005",
            "title": "Reroute Node",
            "position_x": -192,
            "position_y": 0,
        }
    )
    graph_contract["links"] = [
        link
        for link in graph_contract["links"]
        if not (
            link["source"]["node_object_path"] == "/Asset:State.Player"
            and link["target"]["node_object_path"] == "/Asset:State.Blend"
            and link["target"]["pin_name"] == "A"
        )
    ]
    graph_contract["links"].extend(
        [
            {
                "source": {"node_object_path": "/Asset:State.Player", "pin_name": "Pose"},
                "target": {"node_object_path": reroute_path, "pin_name": "InputPin"},
            },
            {
                "source": {"node_object_path": reroute_path, "pin_name": "OutputPin"},
                "target": {"node_object_path": "/Asset:State.Blend", "pin_name": "A"},
            },
        ]
    )
    graph_ir["nodes"].append({"source_node": reroute_path, "renderer": "reroute", "semantic": {}})
    text = render(graph_contract, graph_ir, assets)
    assert 'name: "Reroute Node"' not in text
    assert 'NodeData("10000000-0000-4000-8000-000000000002", "A"): NodeData("10000000-0000-4000-8000-000000000001", "Pose")' in text
    multi_type, multi_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "multi_way_blend",
            "semantic": {
                "pose_count": 2,
                "desired_alpha_defaults": [0.25, 0.75],
                "desired_alpha_keys": ["grounded_state.velocity_blend.forward_amount", ""],
                "normalize_alpha": True,
                "alpha_scale": 1.0,
            },
        },
        {},
        {},
    )
    assert multi_type == "als_ext::SourceMultiWayBlendNode"
    assert "desired_alpha_defaults: [0.25, 0.75]" in multi_inner
    assert 'desired_alpha_keys: ["grounded_state.velocity_blend.forward_amount", ""]' in multi_inner
    assert 'pose_pin_prefix: "Poses_"' in multi_inner
    assert target_pin("multi_way_blend", "Poses_1") == "Poses_1"
    assert target_pin("multi_way_blend", "DesiredAlphas_1") == "DesiredAlphas_1"
    tag_type, tag_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "gameplay_tag_blend",
            "semantic": {
                "active_key": "gait",
                "tags": ["Als.Gait.Sprinting"],
                "blend_times": [0.3, 0.2],
                "source_blend_times": [0.1, 0.1],
                "blend_type": "Cubic",
                "pose_pin_names": ["BlendPose_0", "BlendPose_1"],
                "blend_time_pin_names": ["BlendTime_0", "BlendTime_1"],
                "active_tag_pin": "ActiveTag",
                "output_pin": "Pose",
            },
        },
        {},
        {},
    )
    assert tag_type == "als_ext::SourceGameplayTagBlendNode"
    assert 'active_key: "gait"' in tag_inner
    assert "blend_mode: HermiteCubic" in tag_inner
    tag_contract = scalar_renderer.fixture_gameplay_tag_contract()
    tag_property = tag_contract["source_graph"]["nodes"][1]
    tag_property_type, _ = rendered_inner(
        tag_contract,
        tag_property,
        {"renderer": "property_access", "semantic": {}},
        {},
        {},
    )
    assert tag_property_type == "als_ext::SourceGameplayTagInputNode"
    tag_equal = tag_contract["source_graph"]["nodes"][4]
    tag_equal_type, tag_equal_inner = rendered_inner(
        tag_contract,
        tag_equal,
        {"renderer": "gameplay_tag_equal", "semantic": {}},
        {},
        {},
    )
    assert tag_equal_type == "als_ext::SourceScalarExpressionNode"
    assert 'data_type: Bool' in tag_equal_inner
    snapshot_type, snapshot_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "pose_snapshot",
            "semantic": {"snapshot_key": "ragdolling_state.final_ragdoll_pose"},
        },
        {},
        {},
    )
    assert snapshot_type == "als_ext::GlobalPoseInputNode"
    assert 'output: "Pose"' in snapshot_inner
    control_rig_type, control_rig_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "control_rig",
            "semantic": {
                "rig_class": "CR_Als",
                "alpha_enabled": True,
                "transfer_input_pose": True,
                "transfer_input_curves": True,
                "transfer_pose_in_global_space": False,
            },
        },
        {},
        {},
    )
    assert control_rig_type == "als_ext::ControlRigBoundaryNode"
    assert 'rig_class: "CR_Als"' in control_rig_inner
    assert "capture_context: true" in control_rig_inner
    fsm_type, fsm_inner = rendered_inner(
        graph_contract,
        {},
        {"renderer": "state_machine", "semantic": {"fsm_source_graph": "Machine"}},
        {},
        {
            "Machine": {
                "path": "fsms/machine.fsm.ron",
                "state_aliases": [],
                "source_entry_conditions": [
                    {
                        "target_state_id": "entry",
                        "gates": [
                            {
                                "condition": "conditions/entry.animgraph.ron",
                                "output": "bCanEnterTransition",
                                "context_state_id": "10000000-0000-4000-8000-000000000001",
                            }
                        ],
                    }
                ],
                "source_transition_conditions": [
                    {
                        "transition_id": "transition",
                        "source_state_id": "source",
                        "condition": "conditions/primary.animgraph.ron",
                        "output": "bCanEnterTransition",
                        "context_state_id": "10000000-0000-4000-8000-000000000002",
                        "automatic_trigger_time": -1.0,
                        "transition_duration": 0.0,
                        "always_reset_target_on_entry": True,
                        "gates": [
                            {
                                "condition": "conditions/conduit.animgraph.ron",
                                "output": "bCanEnterTransition",
                                "context_state_id": "10000000-0000-4000-8000-000000000003",
                            }
                        ],
                        "notifications": [
                            {"phase": "TransitionStart", "name": "Notify"}
                        ],
                    }
                ],
            }
        },
    )
    assert fsm_type == "als_ext::InterruptibleFsmNode"
    assert 'internal_time_key: "animation.delta_seconds"' in fsm_inner
    assert "max_transitions_per_frame: 3" in fsm_inner
    assert "automatic_trigger_time: Some(-1.0)" in fsm_inner
    assert "always_reset_target_on_entry: Some(true)" in fsm_inner
    assert 'context_state: ("10000000-0000-4000-8000-000000000002")' in fsm_inner
    assert 'gates: [(condition: "conditions/conduit.animgraph.ron"' in fsm_inner
    assert 'notifications: [(phase: "TransitionStart", name: "Notify")]' in fsm_inner
    assert 'source_entry_conditions: [(target_state_id: "entry", gates: [(condition: "conditions/entry.animgraph.ron"' in fsm_inner
    hips_type, hips_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "set_hips_direction",
            "semantic": {
                "new_hips_direction": "LeftBackward",
                "on_become_relevant": True,
                "call_site": "OnBecomeRelevant",
            },
        },
        {},
        {},
    )
    assert hips_type == "als_ext::SourceSetHipsDirectionNode"
    assert 'new_hips_direction: "LeftBackward"' in hips_inner
    enum_type, enum_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "enum_pose_blend",
            "semantic": {
                "active_key": "grounded_state.hips_direction",
                "selected_enum_value": 3.0,
                "active_enum_default": 0.0,
                "default_blend_time": 0.1,
                "selected_blend_time": 0.1,
                "default_pose_pin": "BlendPose_0",
                "selected_pose_pin": "BlendPose_1",
                "default_blend_time_pin": "BlendTime_0",
                "selected_blend_time_pin": "BlendTime_1",
                "active_enum_pin": "ActiveEnumValue",
                "output_pin": "Pose",
                "selected_enum_entry": "LeftBackward",
            },
        },
        {},
        {},
    )
    assert enum_type == "als_ext::SourceEnumPoseBlendNode"
    assert 'selected_enum_value: 3.0' in enum_inner
    assert target_pin("enum_pose_blend", "BlendPose_1") == "BlendPose_1"
    layered_type, layered_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "layered_bone_blend",
            "semantic": {
                "branch_filter_layers": [
                    [{"bone": "thigh_l", "blend_depth": 3}],
                    [{"bone": "thigh_r", "blend_depth": -1}],
                ],
                "blend_weights": [1.0, 0.5],
                "mesh_space_rotation_blend": True,
                "blend_root_motion_based_on_root_bone": True,
                "base_pose_pin": "BasePose",
                "blend_pose_pins": ["BlendPoses_0", "BlendPoses_1"],
                "output_pin": "Pose",
            },
        },
        {},
        {},
    )
    assert layered_type == "als_ext::SourceLayeredBoneBlendNode"
    assert 'branch_bone_layers' not in layered_inner
    assert 'blend_depth: -1' in layered_inner
    assert 'base_pose_pin: "BasePose"' in layered_inner
    assert target_pin("layered_bone_blend", "BlendPoses_1") == "BlendPoses_1"
    assert target_pin("layered_bone_blend", "BlendWeights_1") == "BlendWeights_1"
    layered_path = "/Asset:State.Layered"
    layered_raw = {
        "object_path": layered_path,
        "node_guid": "10000000-0000-4000-8000-000000000023",
        "position_x": 0,
        "position_y": 0,
    }
    layered_nodes, layered_edges = layered_bone_weight_helpers(
        {"links": []},
        layered_raw,
        {
            "semantic": {
                "blend_weights": [1.0, 0.5],
                "blend_weight_keys": ["layering_state.left_amount", ""],
                "blend_weight_pins": ["BlendWeights_0", "BlendWeights_1"],
            }
        },
    )
    assert len(layered_nodes) == 2
    assert any(node[3] == "als_ext::GlobalF32InputNode" for node in layered_nodes)
    assert any(node[3] == "als_ext::ConstantF32Node" for node in layered_nodes)
    assert any('"BlendWeights_0"' in edge for edge in layered_edges)
    curve_type, curve_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "modify_curve",
            "semantic": {
                "input_pins": ["SourcePose", "CurveValues_0", "Alpha"],
                "source_pose_pin": "SourcePose",
                "curve_names": ["RotationYawOffset"],
                "curve_values": [0.0],
                "curve_value_paths": ["GroundedState.RotationYawOffsets.ForwardAngle"],
                "curve_value_keys": ["grounded_state.rotation_yaw_offsets.forward_angle"],
                "alpha": 1.0,
                "alpha_visible": True,
                "apply_mode": "Blend",
            },
        },
        {},
        {},
    )
    assert curve_type == "als_ext::SourceCurveModifyNode"
    assert 'source_pose_pin: "SourcePose"' in curve_inner
    assert 'value_keys: ["grounded_state.rotation_yaw_offsets.forward_angle"]' in curve_inner
    slot_type, slot_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "slot",
            "semantic": {
                "slot_name": "TurnInPlaceStanding",
                "group_name": "Grounded",
                "pose_key": "turn_in_place_standing.pose",
                "weight_key": "turn_in_place_standing.weight",
                "layer_weight_key": "turn_in_place_standing.layer_weight",
                "mesh_space_additive_key": "turn_in_place_standing.mesh_space_additive",
            },
        },
        {},
        {},
    )
    assert slot_type == "als_ext::SourceSlotNode"
    assert 'slot_name: "TurnInPlaceStanding"' in slot_inner
    assert 'pose_key: "turn_in_place_standing.pose"' in slot_inner
    assert 'mesh_space_additive_key: "turn_in_place_standing.mesh_space_additive"' in slot_inner
    for mesh_space in (False, True):
        node_type, inner = rendered_inner(
            graph_contract,
            {},
            {
                "renderer": "make_dynamic_additive",
                "semantic": {"mesh_space_additive": mesh_space},
            },
            {},
            {},
        )
        assert node_type == "als_ext::SourceMakeDynamicAdditiveNode"
        assert inner == "(mesh_space_additive: " + bool_ron(mesh_space) + ")"
    for mode in ("Accumulate", "Override"):
        node_type, inner = rendered_inner(
            graph_contract,
            {},
            {
                "renderer": "curves_blend",
                "semantic": {"blend_amount": 1.0, "blend_mode": mode},
            },
            {},
            {},
        )
        assert node_type == "als_ext::SourceCurvesBlendNode"
        assert inner == "(blend_amount: 1.0, blend_mode: " + mode + ")"
    try:
        rendered_inner(
            graph_contract,
            {},
            {"renderer": "layered_bone_blend"},
            {},
            {},
        )
    except PoseRenderError:
        pass
    else:
        raise AssertionError("missing source renderer semantic was accepted")
    graph_ir["nodes"][0]["semantic"]["sync_group"] = "Movement"
    try:
        render(graph_contract, graph_ir, assets)
    except PoseRenderError:
        pass
    else:
        raise AssertionError("source sync group was rendered without a group renderer")
    graph_ir["nodes"][0]["semantic"].update(
        {
            "sync_phase_key": "sync_groups.asset.movement.phase",
            "sync_phase_duration": 1.0,
            "sync_phase_markers": [],
            "sync_phase_looped": True,
            "sync_group_role": "CanBeLeader",
            "delta_seconds_key": "animation.delta_seconds",
        }
    )
    text = render(graph_contract, graph_ir, assets)
    assert "synchronization: Some((" in text
    assert '"als_ext::GlobalSyncGroupSampleNode"' not in text
    blend_path = "/Asset:State.Blendspace"
    blend_result = "/Asset:State.BlendspaceResult"
    blend_contract = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": blend_path,
                    "node_guid": "10000000-0000-4000-8000-000000000101",
                    "title": "Blendspace Evaluator",
                    "position_x": -64,
                    "position_y": 0,
                    "pins": [],
                },
                {
                    "object_path": blend_result,
                    "node_guid": "10000000-0000-4000-8000-000000000102",
                    "title": "Output Animation Pose",
                    "position_x": 0,
                    "position_y": 0,
                    "pins": [],
                },
            ]
        },
        "links": [
            {
                "source": {"node_object_path": blend_path, "pin_name": "Pose"},
                "target": {"node_object_path": blend_result, "pin_name": "Result"},
            }
        ],
    }
    blend_ir = {
        "nodes": [
            {
                "source_node": blend_path,
                "renderer": "blendspace_evaluator",
                "semantic": {
                    "input_pins": [
                        {"name": "X", "axis": "X", "default_value": 0.25},
                        {"name": "Y", "axis": "Y", "default_value": -0.5},
                    ],
                    "input_binding_keys": ["", ""],
                    "normalized_time": 0.0,
                    "teleport_to_normalized_time": True,
                    "loop": True,
                    "configured_play_rate": 1.0,
                    "blendspace": {
                        "samples": [
                            {
                                "value": [0.0, 0.0, 0.0],
                                "rate_scale": 1.0,
                                "target_clip": "animations/Center.anim.ron",
                            }
                        ]
                    },
                },
            },
            {"source_node": blend_result, "renderer": "state_result", "semantic": {"lifecycle": []}},
        ]
    }
    text = render(blend_contract, blend_ir, [])
    assert '"als_ext::NormalizedBlendSpaceNode"' in text
    assert '"als_ext::F32ToVec2Node"' in text
    assert '"bevy_animation_graph::builtin_nodes::ClipNode"' in text
    assert "value: 0.25" in text and "value: -0.5" in text
    blend_ir["nodes"][0]["semantic"]["input_binding_keys"] = [
        "lean_state.right_amount",
        "",
    ]
    text = render(blend_contract, blend_ir, [])
    assert '"als_ext::GlobalF32InputNode"' in text
    assert 'key: "lean_state.right_amount"' in text
    one_dimensional_ir = {
        "nodes": [
            {
                "source_node": blend_path,
                "renderer": "blendspace_evaluator",
                "semantic": {
                    "input_pins": [
                        {"name": "X", "axis": "Pitch Angle", "default_value": 0.0},
                        {"name": "NormalizedTime", "axis": None, "default_value": 0.0},
                    ],
                    "input_binding_keys": ["head_state.pitch_angle", "head_state.yaw_amount"],
                    "normalized_time": 0.0,
                    "teleport_to_normalized_time": True,
                    "loop": True,
                    "configured_play_rate": 1.0,
                    "blendspace": {
                        "samples": [
                            {
                                "value": [0.0, 0.0, 0.0],
                                "rate_scale": 1.0,
                                "target_clip": "animations/Center.anim.ron",
                            }
                        ]
                    },
                },
            },
            {"source_node": blend_result, "renderer": "state_result", "semantic": {"lifecycle": []}},
        ]
    }
    text = render(blend_contract, one_dimensional_ir, [])
    assert 'key: "head_state.yaw_amount"' in text
    assert 'Blendspace Y default' in text
    player_ir = {
        "nodes": [
            {
                "source_node": blend_path,
                "renderer": "blendspace_player",
                "semantic": {
                    "input_pins": [
                        {"name": "X", "default_value": 0.25},
                        {"name": "Y", "default_value": 0.5},
                        {"name": "PlayRate", "default_value": 1.0},
                    ],
                    "input_binding_keys": [
                        "standing_state.stride_blend_amount",
                        "standing_state.walk_run_blend_amount",
                        "standing_state.play_rate",
                    ],
                    "loop": True,
                    "configured_play_rate": 1.0,
                    "sync_phase_key": "sync_groups.ab_als_standing.movement.phase",
                    "sync_group_role": "CanBeLeader",
                    "sync_phase_looped": True,
                    "start_position": 0.2,
                    "sync_phase_duration": 1.0,
                    "sync_phase_markers": [
                        {"name": "Left", "time": 0.0},
                        {"name": "Right", "time": 0.5},
                    ],
                    "delta_seconds_key": "animation.delta_seconds",
                    "axis_filter": {"x_window_seconds": 0.2, "y_window_seconds": 0.4},
                    "triangulation": {"axis_min": [0.0, 0.0], "axis_max": [1.0, 1.0], "triangles": [[0, 0, 0]]},
                    "blendspace": {
                        "samples": [
                            {
                                "value": [0.0, 0.0, 0.0],
                                "rate_scale": 1.0,
                                "target_clip": "animations/Center.anim.ron",
                                "source_timing": {
                                    "duration": 1.0,
                                    "markers": [
                                        {"name": "Left", "time": 0.0},
                                        {"name": "Right", "time": 0.5},
                                    ],
                                },
                            }
                        ]
                    },
                },
            },
            {"source_node": blend_result, "renderer": "state_result", "semantic": {"lifecycle": []}},
        ]
    }
    text = render(blend_contract, player_ir, [])
    assert '"als_ext::CubicVec2AxisFilterNode"' in text
    assert 'key: "sync_groups.ab_als_standing.movement.phase"' in text
    assert 'key: "animation.delta_seconds"' in text
    assert "duration: 1.0" in text and 'markers: [(name: "Left", time: 0.0)' in text
    player_nodes, _, _ = blendspace_helpers(
        blend_contract,
        blend_contract["source_graph"]["nodes"][0],
        player_ir["nodes"][0],
    )
    player_positions = {node[1]: node[4] for node in player_nodes}
    assert player_positions["Blendspace position"] == (-256.0, 0.0)
    assert player_positions["Blendspace cubic axis filter"] == (-160.0, 0.0)
    assert player_positions["Blendspace X binding"] == (-352.0, 0.0)
    return {
        "pose_leaf_graph_rendered": "PASS",
        "source_node_identity_and_pin_names_preserved": "PASS",
        "source_node_order_preserved": "PASS",
        "sync_group_requires_a_clock_contract": "PASS",
        "sync_group_clock_preserves_source_player": "PASS",
        "nonfunctional_source_comments_are_ignored": "PASS",
        "source_reroutes_are_folded": "PASS",
        "unreachable_source_edges_are_ignored": "PASS",
        "source_blendspace_evaluator_is_rendered": "PASS",
        "one_dimensional_dynamic_blendspace_evaluator_is_rendered": "PASS",
        "source_blendspace_property_binding_is_rendered": "PASS",
        "source_blendspace_player_marker_phase_is_rendered": "PASS",
        "source_multi_way_blend_is_rendered": "PASS",
        "source_gameplay_tag_blend_is_rendered": "PASS",
        "source_gameplay_tag_scalar_contract_is_rendered": "PASS",
        "source_pose_snapshot_is_rendered": "PASS",
        "source_control_rig_context_transfer_is_rendered": "PASS",
        "source_state_machine_uses_internal_delta_time": "PASS",
        "source_set_hips_direction_is_rendered": "PASS",
        "source_enum_pose_blend_is_rendered": "PASS",
        "source_layered_bone_blend_is_rendered": "PASS",
        "source_modify_curve_binding_is_rendered": "PASS",
        "source_dynamic_additive_space_is_rendered": "PASS",
        "source_curves_blend_is_rendered": "PASS",
        "missing_renderer_semantic_is_reported": "PASS",
        "source_slot_is_rendered": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
