import json
import uuid


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


def rendered_inner(graph_contract, source, semantic, asset_by_node):
    renderer = semantic.get("renderer")
    value = semantic.get("semantic")
    if renderer == "sequence_player":
        pins = input_names(value)
        fields = [
            field("clip", quoted(asset_for_node(asset_by_node, semantic["source_node"], "sequence"))),
            field("additive_reference", "None"),
            field("additive_reference_position", "0.0"),
            field("play_rate", repr(value["play_rate"])),
            field("play_rate_key", quoted("")),
            field("expose_play_rate_pin", bool_ron("PlayRate" in pins)),
            field("play_rate_basis", repr(value["play_rate_basis"])),
            field("play_rate_mapping", mapping_ron(value["play_rate_mapping"])),
            field("start_position", repr(value["start_position"])),
            field("expose_start_position_pin", bool_ron("StartPosition" in pins)),
            field("loop_animation", bool_ron(value["loop_animation"])),
            field("loop_animation_key", quoted("")),
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
    if renderer == "two_way_blend":
        pins = input_names(value)
        fields = [
            field("alpha_key", quoted("")),
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
        fields = [
            field("curve_name", quoted("")),
            field("curve_names", "[" + ", ".join(quoted(name) for name in value["curve_names"]) + "]"),
            field("value", "0.0"),
            field("pin_default", "0.0"),
            field("pin_defaults", "[" + ", ".join(repr(item) for item in value["curve_values"]) + "]"),
            field("value_pin_names", "[" + ", ".join(quoted(name) for name in value_pins) + "]"),
            field("value_key", quoted("")),
            field("value_keys", "[" + ", ".join(quoted("") for _ in value["curve_names"]) + "]"),
            field("source_value_path", quoted("")),
            field("source_value_paths", "[" + ", ".join(quoted("") for _ in value["curve_names"]) + "]"),
            field("alpha", repr(value["alpha"])),
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


def target_pin(renderer, source_pin):
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
    return source_pin


def render(graph_contract, graph_ir, animation_assets, sync_group_phase_keys=None):
    raw_nodes = node_index(graph_contract)
    ir_nodes = semantic_index(graph_ir)
    if set(raw_nodes) < set(ir_nodes):
        raise PoseRenderError("source IR node is missing from source graph")
    for semantic in ir_nodes.values():
        if semantic.get("renderer") not in SUPPORTED_RENDERERS:
            raise PoseRenderError("source pose graph contains an unsupported renderer")
    raw_by_id = {node_id(node): node for node in raw_nodes.values()}
    if len(raw_by_id) != len(raw_nodes):
        raise PoseRenderError("source node identifiers are duplicated")
    asset_by_node = asset_index(animation_assets)
    sync_group_phase_keys = sync_group_phase_keys or {}
    result_nodes = []
    sync_wrappers = {}
    for source_path, semantic in ir_nodes.items():
        raw = raw_nodes[source_path]
        identifier = node_id(raw)
        node_type, inner = rendered_inner(graph_contract, raw, semantic, asset_by_node)
        result_nodes.append((identifier, source_title(raw), inner, node_type, position(raw)))
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
    result_nodes.sort(key=lambda value: value[0])
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
        target_identifier = node_id(raw_nodes[target_semantic["source_node"]])
        source_pin = (
            "pose"
            if source_semantic["source_node"] in sync_wrappers
            else target_pin(source_semantic["renderer"], source.get("pin_name"))
        )
        target_pin_name = target_pin(target_semantic["renderer"], target.get("pin_name"))
        edges.append(
            f"NodeData({quoted(target_identifier)}, {quoted(target_pin_name)}): NodeData({quoted(source_identifier)}, {quoted(source_pin)})"
        )
    for source_path, wrapper_id in sync_wrappers.items():
        identifier = node_id(raw_nodes[source_path])
        edges.append(
            f"NodeData({quoted(wrapper_id)}, \"pose\"): NodeData({quoted(identifier)}, \"Pose\")"
        )
        edges.append(f"NodeTime({quoted(wrapper_id)}, \"time\"): NodeTime({quoted(identifier)})")
    result_identifier = node_id(state_result_raw)
    edges.append(f"OutputData({quoted(output_name)}): NodeData({quoted(result_identifier)}, \"Pose\")")
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
                    "play_rate_basis": 1.0,
                    "play_rate_mapping": mapping,
                    "start_position": 0.0,
                    "loop_animation": True,
                    "sync_group": "None",
                },
            },
            {
                "source_node": blend_path,
                "renderer": "two_way_blend",
                "semantic": {
                    "input_pins": ["A", "B", "Alpha"],
                    "alpha_default": 0.25,
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
    text = render(graph_contract, graph_ir, assets)
    assert '"als_ext::SourceSequencePlayerNode"' in text
    assert '"als_ext::SourceTwoWayBlendNode"' in text
    assert 'NodeData("10000000-0000-4000-8000-000000000002", "A")' in text
    assert 'OutputData("Result")' in text
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
    return {
        "pose_leaf_graph_rendered": "PASS",
        "source_node_identity_and_pin_names_preserved": "PASS",
        "sync_group_requires_a_phase_source": "PASS",
        "sync_group_wrapper_preserves_source_player": "PASS",
    }


if __name__ == "__main__":
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
