import json
import sys
import uuid
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "import"))

import generate_ue_transition_expression_graph as scalar_renderer


class PoseRenderError(ValueError):
    pass


SUPPORTED_RENDERERS = {
    "sequence_player",
    "sequence_evaluator",
    "two_way_blend",
    "apply_additive",
    "apply_mesh_space_additive",
    "modify_curve",
    "state_result",
    "root_output",
    "save_cached_pose",
    "use_cached_pose",
    "linked_input_pose",
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


def marker_ron(markers):
    if not isinstance(markers, list) or len(markers) < 2:
        raise PoseRenderError("source blendspace marker contract is invalid")
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


def blendspace_inner(value, player=False):
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
        if abs(value.get("normalized_time", 0.0)) > 1.0e-6:
            raise PoseRenderError("source blendspace evaluator normalized time is unsupported")
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
            + f", point: ({float(coordinates[0])!r}, {float(coordinates[1])!r}), duration: {duration!r}, markers: {markers})"
        )
    if not points:
        raise PoseRenderError("source blendspace samples are missing")
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
        + "])",
    )


def rendered_inner(graph_contract, source, semantic, asset_by_node, fsm_assets):
    renderer = semantic.get("renderer")
    value = semantic.get("semantic")
    if renderer == "sequence_player":
        pins = input_names(value)
        play_rate_key = value.get("play_rate_key")
        loop_animation_key = value.get("loop_animation_key")
        if not isinstance(play_rate_key, str) or not isinstance(loop_animation_key, str):
            raise PoseRenderError("source sequence property binding is unavailable")
        fields = [
            field("clip", quoted(asset_for_node(asset_by_node, semantic["source_node"], "sequence"))),
            field("additive_reference", "None"),
            field("additive_reference_position", "0.0"),
            field("play_rate", repr(value["play_rate"])),
            field("play_rate_key", quoted(play_rate_key)),
            field("expose_play_rate_pin", bool_ron("PlayRate" in pins)),
            field("play_rate_basis", repr(value["play_rate_basis"])),
            field("play_rate_mapping", mapping_ron(value["play_rate_mapping"])),
            field("start_position", repr(value["start_position"])),
            field("expose_start_position_pin", bool_ron("StartPosition" in pins)),
            field("loop_animation", bool_ron(value["loop_animation"])),
            field("loop_animation_key", quoted(loop_animation_key)),
            field("expose_loop_animation_pin", bool_ron("LoopAnimation" in pins)),
            field("sync_group", quoted(value["sync_group"])),
        ]
        return "als_ext::SourceSequencePlayerNode", "(" + ", ".join(fields) + ")"
    if renderer == "sequence_evaluator":
        pins = input_names(value)
        fields = [
            field("clip", quoted(asset_for_node(asset_by_node, semantic["source_node"], "sequence"))),
            field("explicit_frame", "0.0"),
            field("explicit_frame_key", quoted("")),
            field("explicit_frame_pin", quoted("Explicit Frame")),
            field("expose_explicit_frame_pin", bool_ron("ExplicitFrame" in pins)),
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
    if renderer in SCALAR_RENDERERS:
        return "als_ext::SourceScalarExpressionNode", scalar_renderer.node_inner(source)
    if renderer == "state_machine":
        fsm_source = value.get("fsm_source_graph") if isinstance(value, dict) else None
        config = fsm_assets.get(fsm_source)
        if not isinstance(config, dict):
            raise PoseRenderError("source state machine materialization is unavailable")
        aliases = config.get("state_aliases", [])
        labels = config.get("state_labels", [])
        if not isinstance(aliases, list) or not isinstance(labels, list):
            raise PoseRenderError("source state machine materialization is invalid")
        alias_ron = ", ".join(
            "(state_id: "
            + quoted(alias["state_id"])
            + ", member_state_ids: ["
            + ", ".join(quoted(member) for member in alias["member_state_ids"])
            + "])"
            for alias in aliases
        )
        return (
            "als_ext::InterruptibleFsmNode",
            "(fsm: "
            + quoted(config["path"])
            + ", state_pose_output: \"Result\", outer_pose_output: \"Pose\", state_aliases: ["
            + alias_ron
            + "], state_label_key: "
            + quoted(config["state_label_key"])
            + ", state_labels: ["
            + ", ".join(quoted(label) for label in labels)
            + "], expose_telemetry: false)",
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
            + ", pose_pin_prefix: \"Poses \", alpha_pin_prefix: \"Desired Alphas \")",
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
        layers = value.get("branch_bone_layers")
        weights = value.get("blend_weights")
        pins = value.get("blend_pose_pins")
        if (
            not isinstance(layers, list)
            or not isinstance(weights, list)
            or not isinstance(pins, list)
            or len(layers) != len(weights)
            or len(layers) != len(pins)
            or not all(isinstance(bone, str) for layer in layers for bone in layer)
            or not all(isinstance(weight, (int, float)) for weight in weights)
            or not all(isinstance(pin, str) for pin in pins)
        ):
            raise PoseRenderError("source layered bone blend contract is invalid")
        layers_ron = "[" + ", ".join(
            "[" + ", ".join(quoted(bone) for bone in layer) + "]" for layer in layers
        ) + "]"
        fields = [
            field("branch_bones", "[]"),
            field("branch_bone_layers", layers_ron),
            field("blend_weight", "1.0"),
            field("blend_weights", "[" + ", ".join(repr(weight) for weight in weights) + "]"),
            field("mesh_space_rotation_blend", bool_ron(value["mesh_space_rotation_blend"])),
            field("blend_root_motion_based_on_root_bone", bool_ron(value["blend_root_motion_based_on_root_bone"])),
            field("base_pose_pin", quoted(value["base_pose_pin"])),
            field("blend_pose_pin", quoted(pins[0])),
            field("blend_pose_pins", "[" + ", ".join(quoted(pin) for pin in pins) + "]"),
            field("output_pin", quoted(value["output_pin"])),
        ]
        return "als_ext::SourceLayeredBoneBlendNode", "(" + ", ".join(fields) + ")"
    if renderer == "two_way_blend":
        pins = input_names(value)
        alpha_key = value.get("alpha_key")
        if not isinstance(alpha_key, str):
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
        ]
        return "als_ext::SourceTwoWayBlendNode", "(" + ", ".join(fields) + ")"
    if renderer in {"apply_additive", "apply_mesh_space_additive"}:
        pins = input_names(value)
        fields = [
            field("alpha_key", quoted("")),
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
    if renderer == "modify_curve":
        pins = input_names(value)
        value_pins = [pin for pin in pins if pin not in {"SourcePose", "Alpha"}]
        value_keys = value.get("curve_value_keys")
        value_paths = value.get("curve_value_paths")
        if (
            not isinstance(value_keys, list)
            or not isinstance(value_paths, list)
            or len(value_keys) != len(value["curve_names"])
            or len(value_paths) != len(value["curve_names"])
            or not all(isinstance(item, str) for item in [*value_keys, *value_paths])
        ):
            raise PoseRenderError("source curve property binding is unavailable")
        fields = [
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
            + ")",
        )
    if renderer == "root_output":
        return (
            "als_ext::SourcePosePassthroughNode",
            "(input: \"Source\", output: \"Pose\", input_time: \"source_time\", output_time: false, source_function: \"\", call_site: \"\")",
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
    if renderer == "sequence_player":
        return {"PlayRate": "Play Rate", "StartPosition": "Start Position", "LoopAnimation": "Loop Animation"}.get(source_pin, source_pin)
    if renderer == "sequence_evaluator":
        return {"ExplicitFrame": "Explicit Frame"}.get(source_pin, source_pin)
    if renderer == "modify_curve":
        return {"SourcePose": "Source Pose"}.get(source_pin, source_pin)
    if renderer == "state_result":
        return {"Result": "Source"}.get(source_pin, source_pin)
    if renderer == "root_output":
        return {"Result": "Source"}.get(source_pin, source_pin)
    if renderer in {"blendspace_evaluator", "blendspace_player"}:
        return {"Pose": "pose"}.get(source_pin, source_pin)
    if renderer == "state_machine":
        return {"Pose": "Pose"}.get(source_pin, source_pin)
    if renderer == "multi_way_blend":
        if source_pin.startswith("Poses_"):
            return "Poses " + source_pin.rsplit("_", 1)[1]
        if source_pin.startswith("DesiredAlphas_"):
            return "Desired Alphas " + source_pin.rsplit("_", 1)[1]
    if renderer == "enum_pose_blend":
        if source_pin == "Pose":
            return "Pose"
        if node is None:
            raise PoseRenderError("source enum pose blend pin is unavailable")
        pin = next((pin for pin in node.get("pins", []) if pin.get("name") == source_pin), None)
        label = pin.get("friendly_name") if isinstance(pin, dict) else None
        if not isinstance(label, str) or not label:
            raise PoseRenderError("source enum pose blend pin is unavailable")
        return label
    if renderer == "layered_bone_blend":
        if source_pin == "BasePose":
            return "Base Pose"
        if source_pin.startswith("BlendPoses_"):
            return "Blend Poses " + source_pin.rsplit("_", 1)[1]
    return source_pin


def blendspace_helpers(graph_contract, raw, semantic):
    renderer = semantic.get("renderer")
    player = renderer == "blendspace_player"
    value = semantic.get("semantic")
    if not isinstance(value, dict):
        raise PoseRenderError("source blendspace renderer semantic is invalid")
    blendspace = value.get("blendspace")
    inputs = value.get("input_pins")
    expected_inputs = ["X", "Y", "PlayRate"] if player else ["X", "Y"]
    if not isinstance(blendspace, dict) or not isinstance(inputs, list) or len(inputs) != len(expected_inputs):
        raise PoseRenderError("source blendspace input contract is invalid")
    binding_keys = value.get("input_binding_keys")
    if not isinstance(binding_keys, list) or len(binding_keys) != len(expected_inputs) or not all(
        isinstance(key, str) for key in binding_keys
    ):
        raise PoseRenderError("source blendspace property binding is unavailable")
    if [item.get("name") for item in inputs] != expected_inputs:
        raise PoseRenderError("source blendspace input schema is unsupported")
    primary_id = node_id(raw)
    source_path = raw["object_path"]
    x, y = position(raw)
    linked_inputs = {
        (link.get("target", {}).get("node_object_path"), link.get("target", {}).get("pin_name"))
        for link in graph_contract.get("links", [])
    }
    vector_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/vector/" + source_path))
    phase_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/blendspace/phase/" + source_path))
    nodes = [
        (vector_id, "Blendspace position", "()", "als_ext::F32ToVec2Node", (x - 96.0, y)),
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
                    phase_id,
                    "Blendspace sync phase",
                    "(key: " + quoted(phase_key) + ")",
                    "als_ext::GlobalF32InputNode",
                    (x - 96.0, y + 96.0),
                ),
                (
                    filter_id,
                    "Blendspace cubic axis filter",
                    "(x_window_seconds: "
                    + repr(axis_filter["x_window_seconds"])
                    + ", y_window_seconds: "
                    + repr(axis_filter["y_window_seconds"])
                    + ")",
                    "als_ext::CubicVec2AxisFilterNode",
                    (x, y),
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
                f"NodeData({quoted(primary_id)}, \"phase\"): NodeData({quoted(phase_id)}, \"{phase_key}\")",
            ]
        )
    else:
        nodes.append(
            (
                phase_id,
                "Blendspace normalized time",
                "(value: " + repr(value["normalized_time"]) + ")",
                "als_ext::ConstantF32Node",
                (x - 96.0, y + 96.0),
            )
        )
        edges.extend(
            [
                f"NodeData({quoted(primary_id)}, \"position\"): NodeData({quoted(vector_id)}, \"value\")",
                f"NodeData({quoted(primary_id)}, \"phase\"): NodeData({quoted(phase_id)}, \"value\")",
            ]
        )
    targets = {}
    for axis, input_value, binding_key in zip(("x", "y"), inputs[:2], binding_keys[:2]):
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
                        (x - 192.0, y + (0.0 if axis == "x" else 48.0)),
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
                        (x - 192.0, y + (0.0 if axis == "x" else 48.0)),
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


def render(
    graph_contract,
    graph_ir,
    animation_assets,
    sync_group_phase_keys=None,
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
    sync_group_phase_keys = sync_group_phase_keys or {}
    fsm_assets = fsm_assets or {}
    result_nodes = []
    sync_wrappers = {}
    blendspace_edges = []
    blendspace_targets = {}
    for source_path, semantic in ir_nodes.items():
        if semantic.get("renderer") == "comment_group":
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
        if semantic.get("renderer") != "sequence_player":
            continue
        group = semantic["semantic"].get("sync_group")
        if group == "None":
            continue
        key = sync_group_phase_keys.get(group)
        if not isinstance(key, str) or not key:
            raise PoseRenderError("source sync group phase key is unavailable")
        wrapper_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "bevy-als/sync/" + source_path))
        x, y = position(raw)
        looped = bool_ron(semantic["semantic"]["loop_animation"])
        wrapper = (
            "(key: " + quoted(key)
            + ", leader_duration: 0.0, leader_markers: [], follower_duration: 0.0, follower_markers: [], looped: "
            + looped
            + ")"
        )
        result_nodes.append(
            (
                wrapper_id,
                "Sync group " + group,
                wrapper,
                "als_ext::GlobalSyncGroupSampleNode",
                (x, y + 96.0),
            )
        )
        sync_wrappers[source_path] = wrapper_id
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
    for link in graph_contract.get("links", []):
        source = link.get("source", {})
        target = link.get("target", {})
        source_semantic = ir_nodes.get(source.get("node_object_path"))
        target_semantic = ir_nodes.get(target.get("node_object_path"))
        if source_semantic is None or target_semantic is None:
            raise PoseRenderError("source pose edge endpoint is unsupported")
        source_identifier = sync_wrappers.get(
            source_semantic["source_node"],
            node_id(raw_nodes[source_semantic["source_node"]]),
        )
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
            "pose"
            if source_semantic["source_node"] in sync_wrappers
            else target_pin(
                source_semantic["renderer"],
                source.get("pin_name"),
                raw_nodes[source_semantic["source_node"]],
            )
        )
        edges.append(
            f"NodeData({quoted(target_identifier)}, {quoted(target_pin_name)}): NodeData({quoted(source_identifier)}, {quoted(source_pin)})"
        )
    for source_path, wrapper_id in sync_wrappers.items():
        identifier = node_id(raw_nodes[source_path])
        edges.append(
            f"NodeData({quoted(wrapper_id)}, \"pose\"): NodeData({quoted(identifier)}, \"Pose\")"
        )
        edges.append(f"NodeTime({quoted(wrapper_id)}, \"time\"): NodeTime({quoted(identifier)})")
    edges.extend(blendspace_edges)
    result_identifier = node_id(state_result_raw)
    edges.append(f"OutputData({quoted(output_name)}): NodeData({quoted(result_identifier)}, \"Pose\")")
    result_nodes.sort(key=lambda value: value[0])
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
            "    io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { \"Result\": Pose }, input_order: {}, output_order: { Data(\"Result\"): 0 }),",
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
                    "play_rate_basis": 1.0,
                    "play_rate_mapping": mapping,
                    "start_position": 0.0,
                    "loop_animation": True,
                    "loop_animation_key": "",
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
    assets = [{"source_node": sequence_path, "category": "sequence", "target_asset": "animations/A.anim.ron"}]
    return graph_contract, graph_ir, assets


def run_self_test():
    graph_contract, graph_ir, assets = fixture_graph()
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
    assert 'OutputData("Result")' in text
    assert 'name: "Comment"' not in text
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
    assert target_pin("multi_way_blend", "Poses_1") == "Poses 1"
    assert target_pin("multi_way_blend", "DesiredAlphas_1") == "Desired Alphas 1"
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
                "default_pose_pin": "Default Pose",
                "selected_pose_pin": "Left Backward Pose",
                "default_blend_time_pin": "Default Blend Time",
                "selected_blend_time_pin": "Left Backward Blend Time",
                "active_enum_pin": "Active Enum Value",
                "output_pin": "Pose",
                "selected_enum_entry": "LeftBackward",
            },
        },
        {},
        {},
    )
    assert enum_type == "als_ext::SourceEnumPoseBlendNode"
    assert 'selected_enum_value: 3.0' in enum_inner
    assert target_pin(
        "enum_pose_blend",
        "BlendPose_1",
        {"pins": [{"name": "BlendPose_1", "friendly_name": "Left Backward Pose"}]},
    ) == "Left Backward Pose"
    layered_type, layered_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "layered_bone_blend",
            "semantic": {
                "branch_bone_layers": [["thigh_l"], ["thigh_r"]],
                "blend_weights": [1.0, 0.5],
                "mesh_space_rotation_blend": True,
                "blend_root_motion_based_on_root_bone": True,
                "base_pose_pin": "Base Pose",
                "blend_pose_pins": ["Blend Poses 0", "Blend Poses 1"],
                "output_pin": "Pose",
            },
        },
        {},
        {},
    )
    assert layered_type == "als_ext::SourceLayeredBoneBlendNode"
    assert 'branch_bone_layers: [["thigh_l"], ["thigh_r"]]' in layered_inner
    assert target_pin("layered_bone_blend", "BlendPoses_1") == "Blend Poses 1"
    curve_type, curve_inner = rendered_inner(
        graph_contract,
        {},
        {
            "renderer": "modify_curve",
            "semantic": {
                "input_pins": ["SourcePose", "CurveValues_0", "Alpha"],
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
    assert 'value_keys: ["grounded_state.rotation_yaw_offsets.forward_angle"]' in curve_inner
    graph_ir["nodes"][0]["semantic"]["sync_group"] = "Movement"
    try:
        render(graph_contract, graph_ir, assets)
    except PoseRenderError:
        pass
    else:
        raise AssertionError("source sync group was rendered without a group renderer")
    text = render(
        graph_contract,
        graph_ir,
        assets,
        {"Movement": "als_source_movement_phase"},
    )
    assert '"als_ext::GlobalSyncGroupSampleNode"' in text
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
                        {"name": "X", "default_value": 0.25},
                        {"name": "Y", "default_value": -0.5},
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
                    "sync_phase_key": "standing_state.movement.sync_phase",
                    "sync_phase_duration": 1.0,
                    "sync_phase_markers": [
                        {"name": "Left", "time": 0.0},
                        {"name": "Right", "time": 0.5},
                    ],
                    "delta_seconds_key": "animation.delta_seconds",
                    "axis_filter": {"x_window_seconds": 0.2, "y_window_seconds": 0.4},
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
    assert 'key: "standing_state.movement.sync_phase"' in text
    assert 'key: "animation.delta_seconds"' in text
    assert "duration: 1.0" in text and 'markers: [(name: "Left", time: 0.0)' in text
    return {
        "pose_leaf_graph_rendered": "PASS",
        "source_node_identity_and_pin_names_preserved": "PASS",
        "sync_group_requires_a_phase_source": "PASS",
        "sync_group_wrapper_preserves_source_player": "PASS",
        "nonfunctional_source_comments_are_ignored": "PASS",
        "source_blendspace_evaluator_is_rendered": "PASS",
        "source_blendspace_property_binding_is_rendered": "PASS",
        "source_blendspace_player_marker_phase_is_rendered": "PASS",
        "source_multi_way_blend_is_rendered": "PASS",
        "source_set_hips_direction_is_rendered": "PASS",
        "source_enum_pose_blend_is_rendered": "PASS",
        "source_layered_bone_blend_is_rendered": "PASS",
        "source_modify_curve_binding_is_rendered": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
