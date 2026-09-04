import argparse
import json

from ue_exported_text import (
    ExportedTextError,
    field,
    parse_bool,
    parse_list,
    parse_number,
    parse_quoted,
    parse_scale_bias,
    parse_scale_bias_clamp,
    parse_struct,
    reject_unknown_fields,
)


POSE_RENDERERS = {
    "sequence_player",
    "sequence_evaluator",
    "two_way_blend",
    "apply_additive",
    "apply_mesh_space_additive",
    "modify_curve",
    "save_cached_pose",
    "use_cached_pose",
    "state_result",
    "linked_input_pose",
    "inertialization",
    "pose_lifecycle_call",
    "multi_way_blend",
    "set_hips_direction",
    "enum_pose_blend",
    "layered_bone_blend",
}
SYNC_GROUP_ROLES = {
    "CanBeLeader",
    "AlwaysFollower",
    "AlwaysLeader",
    "TransitionLeader",
    "TransitionFollower",
    "ExclusiveAlwaysLeader",
}


class PoseContractError(ValueError):
    pass


def source_node_id(node):
    value = node.get("source_node", node.get("object_path"))
    if not isinstance(value, str) or not value:
        raise PoseContractError("source node identity is invalid")
    return value


def node_property(node, name):
    values = [
        value.get("exported_text")
        for value in node.get("properties", [])
        if value.get("name") == name and isinstance(value.get("exported_text"), str)
    ]
    if len(values) != 1:
        raise PoseContractError(f"source node property is missing or ambiguous: {name}")
    return values[0]


def visible_input_names(node):
    return [
        pin.get("name")
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden")
    ]


def visible_output_names(node):
    return [
        pin.get("name")
        for pin in node.get("pins", [])
        if pin.get("direction") == "Output" and not pin.get("hidden")
    ]


def visible_input_default(node, name):
    values = [
        pin.get("default_value")
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden") and pin.get("name") == name
    ]
    if len(values) != 1:
        raise PoseContractError(f"source input pin is missing or ambiguous: {name}")
    return parse_number(values[0])


def property_binding_paths(node):
    binding = node.get("binding")
    if binding is None:
        return {}
    if not isinstance(binding, dict):
        raise PoseContractError("source property binding is invalid")
    values = [
        value.get("exported_text")
        for value in binding.get("properties", [])
        if value.get("name") == "PropertyBindings" and isinstance(value.get("exported_text"), str)
    ]
    if len(values) != 1:
        raise PoseContractError("source property bindings are missing or ambiguous")
    result = {}
    for item in parse_list(values[0]):
        pair = parse_list(item)
        if len(pair) != 2:
            raise PoseContractError("source property binding entry is invalid")
        pin = parse_quoted(pair[0])
        fields = parse_struct(pair[1])
        if parse_quoted(field(fields, "PropertyName")) != pin:
            raise PoseContractError("source property binding pin name is invalid")
        if not parse_bool(field(fields, "bIsBound")):
            continue
        path = parse_quoted(field(fields, "PathAsText"))
        if path.startswith("GetParent."):
            path = path[len("GetParent.") :]
        if not path or pin in result:
            raise PoseContractError("source property binding path is invalid")
        result[pin] = path
    return result


def input_is_linked(node, name):
    pins = [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden") and pin.get("name") == name
    ]
    if len(pins) != 1:
        raise PoseContractError("source input pin is missing or ambiguous")
    return any(item.get("node_object_path") != "None" for item in pins[0].get("linked_to", []))


def issue(node, renderer, kind, detail):
    return {
        "kind": kind,
        "source_node": source_node_id(node),
        "source_class": node.get("source_class", node.get("class_path")),
        "renderer": renderer,
        "detail": detail,
    }


def parse_name(value):
    if value.startswith('"'):
        return parse_quoted(value)
    if not value:
        raise ExportedTextError("name value is invalid")
    return value


def validate_inputs(node, renderer, required, optional=()):
    names = visible_input_names(node)
    missing = [name for name in required if name not in names]
    extra = [name for name in names if name not in set(required) | set(optional)]
    issues = []
    if missing:
        issues.append(issue(node, renderer, "source_pin_missing", ",".join(missing)))
    if extra:
        issues.append(issue(node, renderer, "source_pin_unsupported", ",".join(extra)))
    return names, issues


def lifecycle_issues(node, renderer, fields, names):
    issues = []
    for name in names:
        if field(fields, name, "()") != "()":
            issues.append(issue(node, renderer, "source_lifecycle_unsupported", name))
    return issues


def alpha_contract(node, renderer, fields, base_inputs):
    names, issues = validate_inputs(node, renderer, base_inputs, ("Alpha",))
    input_type = parse_name(field(fields, "AlphaInputType", "Float"))
    if input_type != "Float":
        issues.append(issue(node, renderer, "source_alpha_input_type_unsupported", input_type))
    alpha_bool_blend = field(fields, "AlphaBoolBlend", "()")
    if input_type != "Float" and alpha_bool_blend != "()":
        issues.append(issue(node, renderer, "source_alpha_bool_blend_unsupported", alpha_bool_blend))
    alpha_path = property_binding_paths(node).get("Alpha", "")
    if alpha_path and "Alpha" in names and input_is_linked(node, "Alpha"):
        issues.append(issue(node, renderer, "source_property_binding_link_unsupported", "Alpha"))
    return {
        "input_pins": names,
        "alpha_visible": "Alpha" in names,
        "alpha_default": parse_number(field(fields, "Alpha", "0.0")),
        "alpha_scale_bias": parse_scale_bias(field(fields, "AlphaScaleBias", "()")),
        "alpha_mapping": parse_scale_bias_clamp(field(fields, "AlphaScaleBiasClamp", "()")),
        "alpha_path": alpha_path,
    }, issues


def sequence_player_contract(node):
    renderer = "sequence_player"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "GroupName",
            "GroupRole",
            "Method",
            "Sequence",
            "PlayRateBasis",
            "PlayRate",
            "PlayRateScaleBiasClampConstants",
            "PlayRateScaleBiasClampState",
            "StartPosition",
            "bLoopAnimation",
            "bStartFromMatchingPose",
            "bIgnoreForRelevancyTest",
            "bOverridePositionWhenJoiningSyncGroupAsLeader",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    names, issues = validate_inputs(node, renderer, (), ("PlayRate", "StartPosition", "LoopAnimation"))
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    group = parse_name(field(fields, "GroupName", "None"))
    group_role = parse_name(field(fields, "GroupRole", "CanBeLeader"))
    method = parse_name(field(fields, "Method", "DoNotSync"))
    if group_role not in SYNC_GROUP_ROLES:
        issues.append(issue(node, renderer, "source_sync_group_role_unsupported", group_role))
    if group != "None" or method != "DoNotSync":
        issues.append(issue(node, renderer, "source_sync_group_unsupported", f"{group}:{method}"))
    if parse_bool(field(fields, "bStartFromMatchingPose", "False")):
        issues.append(issue(node, renderer, "source_pose_matching_unsupported", "bStartFromMatchingPose"))
    if field(fields, "PlayRateScaleBiasClampState", "()") != "()":
        issues.append(issue(node, renderer, "source_player_runtime_state_unsupported", "PlayRateScaleBiasClampState"))
    bindings = property_binding_paths(node)
    play_rate_path = bindings.get("PlayRate", "")
    loop_animation_path = bindings.get("bLoopAnimation", "")
    if play_rate_path and input_is_linked(node, "PlayRate"):
        issues.append(issue(node, renderer, "source_property_binding_link_unsupported", "PlayRate"))
    if loop_animation_path and "LoopAnimation" in names and input_is_linked(node, "LoopAnimation"):
        issues.append(issue(node, renderer, "source_property_binding_link_unsupported", "LoopAnimation"))
    return {
        "renderer": renderer,
        "input_pins": names,
        "sequence": parse_quoted(field(fields, "Sequence")),
        "play_rate_basis": parse_number(field(fields, "PlayRateBasis", "1.0")),
        "play_rate": parse_number(field(fields, "PlayRate", "1.0")),
        "play_rate_path": play_rate_path,
        "play_rate_mapping": parse_scale_bias_clamp(
            field(fields, "PlayRateScaleBiasClampConstants", "()")
        ),
        "start_position": parse_number(field(fields, "StartPosition", "0.0")),
        "loop_animation": parse_bool(field(fields, "bLoopAnimation", "True")),
        "loop_animation_path": loop_animation_path,
        "sync_group": group,
        "sync_group_role": group_role,
        "sync_method": method,
        "override_position_when_joining_sync_group_as_leader": parse_bool(
            field(fields, "bOverridePositionWhenJoiningSyncGroupAsLeader", "False")
        ),
    }, issues


def sequence_evaluator_contract(node):
    renderer = "sequence_evaluator"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "Sequence",
            "ExplicitTime",
            "ExplicitFrame",
            "bUseExplicitFrame",
            "bShouldLoop",
            "bTeleportToExplicitTime",
            "ReinitializationBehavior",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    names, issues = validate_inputs(node, renderer, (), ("ExplicitTime", "ExplicitFrame"))
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    use_explicit_frame = parse_bool(field(fields, "bUseExplicitFrame", "False"))
    if not use_explicit_frame:
        issues.append(issue(node, renderer, "source_evaluator_time_mode_unsupported", "ExplicitTime"))
    if "ExplicitTime" in names:
        issues.append(issue(node, renderer, "source_evaluator_time_pin_unsupported", "ExplicitTime"))
    return {
        "renderer": renderer,
        "input_pins": names,
        "sequence": parse_quoted(field(fields, "Sequence")),
        "use_explicit_frame": use_explicit_frame,
        "loop": parse_bool(field(fields, "bShouldLoop", "True")),
        "teleport_to_explicit_time": parse_bool(
            field(fields, "bTeleportToExplicitTime", "True")
        ),
        "reinitialization_behavior": parse_name(
            field(fields, "ReinitializationBehavior", "ExplicitTime")
        ),
    }, issues


def blend_contract(node, renderer, property_name, base_inputs, extra_fields=()):
    fields = parse_struct(node_property(node, property_name))
    reject_unknown_fields(
        fields,
        {
            *base_inputs,
            "Alpha",
            "AlphaInputType",
            "AlphaScaleBias",
            "AlphaScaleBiasClamp",
            "AlphaBoolBlend",
            "AlphaCurveName",
            "bAlphaBoolEnabled",
            "bResetChildOnActivation",
            "bAlwaysUpdateChildren",
            "LODThreshold",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
            *extra_fields,
        },
    )
    contract, issues = alpha_contract(node, renderer, fields, base_inputs)
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    if parse_bool(field(fields, "bResetChildOnActivation", "False")):
        issues.append(issue(node, renderer, "source_child_reset_unsupported", "bResetChildOnActivation"))
    if parse_bool(field(fields, "bAlwaysUpdateChildren", "False")):
        issues.append(issue(node, renderer, "source_child_update_unsupported", "bAlwaysUpdateChildren"))
    if parse_number(field(fields, "LODThreshold", "-1.0")) != -1.0:
        issues.append(issue(node, renderer, "source_lod_threshold_unsupported", "LODThreshold"))
    contract["renderer"] = renderer
    return contract, issues


def two_way_blend_contract(node):
    return blend_contract(node, "two_way_blend", "BlendNode", ("A", "B"))


def apply_additive_contract(node, mesh_space):
    renderer = "apply_mesh_space_additive" if mesh_space else "apply_additive"
    contract, issues = blend_contract(
        node,
        renderer,
        "Node",
        ("Base", "Additive"),
        ("bRootSpaceAdditive",) if mesh_space else (),
    )
    if mesh_space:
        fields = parse_struct(node_property(node, "Node"))
        if parse_bool(field(fields, "bRootSpaceAdditive", "False")):
            issues.append(issue(node, renderer, "source_root_space_additive_unsupported", "bRootSpaceAdditive"))
    return contract, issues


def modify_curve_contract(node):
    renderer = "modify_curve"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "SourcePose",
            "CurveValues",
            "CurveNames",
            "CurveMap",
            "Alpha",
            "ApplyMode",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    values = [parse_number(value) for value in parse_list(field(fields, "CurveValues", "()"))]
    names = [parse_quoted(value) for value in parse_list(field(fields, "CurveNames", "()"))]
    input_names, issues = validate_inputs(
        node,
        renderer,
        ("SourcePose",),
        tuple(f"CurveValues_{index}" for index in range(len(names))) + ("Alpha",),
    )
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    if len(values) != len(names):
        issues.append(issue(node, renderer, "source_curve_value_count_mismatch", "CurveValues"))
    if field(fields, "CurveMap", "()") != "()":
        issues.append(issue(node, renderer, "source_curve_map_unsupported", "CurveMap"))
    alpha = parse_number(field(fields, "Alpha", "1.0"))
    alpha_visible = "Alpha" in input_names
    if alpha_visible:
        pin_alpha = visible_input_default(node, "Alpha")
        if abs(pin_alpha - alpha) > 1.0e-6:
            issues.append(issue(node, renderer, "source_curve_alpha_default_mismatch", "Alpha"))
    bindings = property_binding_paths(node)
    curve_pins = {f"CurveValues_{index}" for index in range(len(names))}
    unknown_bindings = sorted(set(bindings) - curve_pins - {"Alpha"})
    if unknown_bindings:
        issues.append(
            issue(node, renderer, "source_property_binding_pin_unsupported", ",".join(unknown_bindings))
        )
    curve_value_paths = []
    for index in range(len(names)):
        pin = f"CurveValues_{index}"
        path = bindings.get(pin, "")
        if path and input_is_linked(node, pin):
            issues.append(issue(node, renderer, "source_property_binding_link_unsupported", pin))
        curve_value_paths.append(path)
    alpha_path = bindings.get("Alpha", "")
    if alpha_path and input_is_linked(node, "Alpha"):
        issues.append(issue(node, renderer, "source_property_binding_link_unsupported", "Alpha"))
    return {
        "renderer": renderer,
        "input_pins": input_names,
        "curve_names": names,
        "curve_values": values,
        "curve_value_paths": curve_value_paths,
        "alpha_path": alpha_path,
        "alpha": alpha,
        "alpha_visible": alpha_visible,
        "apply_mode": parse_name(field(fields, "ApplyMode", "Blend")),
    }, issues


def multi_way_blend_contract(node):
    renderer = "multi_way_blend"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "Poses",
            "DesiredAlphas",
            "AlphaScaleBias",
            "bAdditiveNode",
            "bNormalizeAlpha",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    poses = parse_list(field(fields, "Poses"))
    alphas = [parse_number(value) for value in parse_list(field(fields, "DesiredAlphas"))]
    if not poses or len(poses) != len(alphas):
        raise PoseContractError("multi-way blend pose and alpha count is invalid")
    required = tuple(f"Poses_{index}" for index in range(len(poses))) + tuple(
        f"DesiredAlphas_{index}" for index in range(len(alphas))
    )
    inputs, issues = validate_inputs(node, renderer, required)
    if visible_output_names(node) != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    for index, value in enumerate(alphas):
        if abs(visible_input_default(node, f"DesiredAlphas_{index}") - value) > 1.0e-6:
            issues.append(
                issue(node, renderer, "source_multi_way_alpha_default_mismatch", str(index))
            )
    scale_bias = parse_scale_bias(field(fields, "AlphaScaleBias", "()"))
    if abs(scale_bias["bias"]) > 1.0e-6:
        issues.append(issue(node, renderer, "source_multi_way_alpha_bias_unsupported", "AlphaScaleBias"))
    if parse_bool(field(fields, "bAdditiveNode", "False")):
        issues.append(issue(node, renderer, "source_multi_way_additive_unsupported", "bAdditiveNode"))
    if not parse_bool(field(fields, "bNormalizeAlpha", "True")):
        issues.append(issue(node, renderer, "source_multi_way_non_normalized_unsupported", "bNormalizeAlpha"))
    if abs(scale_bias["scale"] - 1.0) > 1.0e-6:
        issues.append(issue(node, renderer, "source_multi_way_alpha_scale_unsupported", "AlphaScaleBias"))
    bindings = property_binding_paths(node)
    allowed_binding_pins = {f"DesiredAlphas_{index}" for index in range(len(alphas))}
    unknown_bindings = sorted(set(bindings) - allowed_binding_pins)
    if unknown_bindings:
        issues.append(
            issue(node, renderer, "source_property_binding_pin_unsupported", ",".join(unknown_bindings))
        )
    desired_alpha_paths = []
    for index in range(len(alphas)):
        pin = f"DesiredAlphas_{index}"
        path = bindings.get(pin, "")
        if path and input_is_linked(node, pin):
            issues.append(
                issue(node, renderer, "source_property_binding_link_unsupported", pin)
            )
        desired_alpha_paths.append(path)
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "pose_count": len(poses),
        "desired_alpha_defaults": alphas,
        "desired_alpha_paths": desired_alpha_paths,
        "normalize_alpha": parse_bool(field(fields, "bNormalizeAlpha", "True")),
        "alpha_scale": scale_bias["scale"],
    }, issues


def set_hips_direction_contract(node):
    renderer = "set_hips_direction"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "Source",
            "Function",
            "CallSite",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    inputs, issues = validate_inputs(node, renderer, ("Source", "NewHipsDirection"))
    if visible_output_names(node) != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    directions = [
        pin.get("default_value")
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input"
        and not pin.get("hidden")
        and pin.get("name") == "NewHipsDirection"
    ]
    if len(directions) != 1 or not isinstance(directions[0], str):
        raise PoseContractError("source hips direction input is missing or ambiguous")
    direction = directions[0]
    if direction not in {
        "Forward",
        "Backward",
        "RightForward",
        "RightBackward",
        "LeftForward",
        "LeftBackward",
    }:
        issues.append(issue(node, renderer, "source_hips_direction_unsupported", direction))
    if input_is_linked(node, "NewHipsDirection"):
        issues.append(issue(node, renderer, "source_hips_direction_link_unsupported", "NewHipsDirection"))
    if field(fields, "Function") != "()":
        issues.append(issue(node, renderer, "source_hips_direction_function_unsupported", "Function"))
    call_site = parse_name(field(fields, "CallSite"))
    if call_site != "OnBecomeRelevant":
        issues.append(issue(node, renderer, "source_hips_direction_call_site_unsupported", call_site))
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "new_hips_direction": direction,
        "on_become_relevant": True,
        "call_site": call_site,
    }, issues


def enum_pose_blend_contract(node):
    renderer = "enum_pose_blend"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "BlendPose",
            "BlendTime",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    poses = parse_list(field(fields, "BlendPose"))
    times = [parse_number(value) for value in parse_list(field(fields, "BlendTime"))]
    if len(poses) != 2 or len(times) != 2:
        raise PoseContractError("source enum pose blend entry count is unsupported")
    required = (
        "BlendPose_0",
        "BlendPose_1",
        "BlendTime_0",
        "BlendTime_1",
        "ActiveEnumValue",
    )
    inputs, issues = validate_inputs(node, renderer, required)
    if visible_output_names(node) != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    pins = {
        pin.get("name"): pin
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden")
    }
    labels = []
    for name in ("BlendPose_0", "BlendPose_1", "BlendTime_0", "BlendTime_1", "ActiveEnumValue"):
        label = pins.get(name, {}).get("friendly_name")
        if not isinstance(label, str) or not label:
            issues.append(issue(node, renderer, "source_enum_pose_blend_pin_unsupported", name))
            label = name
        labels.append(label)
    title = node.get("title")
    prefix = "Blend Poses ("
    if not isinstance(title, str) or not title.startswith(prefix) or not title.endswith(")"):
        raise PoseContractError("source enum pose blend title is invalid")
    enum_name = title[len(prefix) : -1]
    selected_label = labels[1]
    if not selected_label.endswith(" Pose"):
        raise PoseContractError("source enum pose blend selected label is invalid")
    selected_entry = selected_label[: -len(" Pose")].replace(" ", "")
    if selected_entry not in {"LeftBackward", "RightBackward"}:
        issues.append(issue(node, renderer, "source_enum_entry_unsupported", selected_entry))
    bindings = property_binding_paths(node)
    unknown_bindings = sorted(set(bindings) - {"ActiveEnumValue"})
    if unknown_bindings:
        issues.append(
            issue(node, renderer, "source_property_binding_pin_unsupported", ",".join(unknown_bindings))
        )
    active_enum_path = bindings.get("ActiveEnumValue", "")
    if active_enum_path and input_is_linked(node, "ActiveEnumValue"):
        issues.append(
            issue(node, renderer, "source_property_binding_link_unsupported", "ActiveEnumValue")
        )
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "enum_name": enum_name,
        "selected_enum_entry": selected_entry,
        "active_enum_path": active_enum_path,
        "active_enum_default": visible_input_default(node, "ActiveEnumValue"),
        "default_blend_time": times[0],
        "selected_blend_time": times[1],
        "default_pose_pin": labels[0],
        "selected_pose_pin": labels[1],
        "default_blend_time_pin": labels[2],
        "selected_blend_time_pin": labels[3],
        "active_enum_pin": labels[4],
        "output_pin": "Pose",
    }, issues


def layered_bone_blend_contract(node):
    renderer = "layered_bone_blend"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "BasePose",
            "BlendPoses",
            "LayerSetup",
            "BlendWeights",
            "SkeletonGuid",
            "VirtualBoneGuid",
            "LODThreshold",
            "bMeshSpaceRotationBlend",
            "bBlendRootMotionBasedOnRootBone",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    poses = parse_list(field(fields, "BlendPoses"))
    layers = parse_list(field(fields, "LayerSetup"))
    weights = [parse_number(value) for value in parse_list(field(fields, "BlendWeights"))]
    if not poses or len(poses) != len(layers) or len(poses) != len(weights):
        raise PoseContractError("source layered bone blend layer count is invalid")
    required = ("BasePose",) + tuple(f"BlendPoses_{index}" for index in range(len(poses)))
    inputs, issues = validate_inputs(node, renderer, required)
    if visible_output_names(node) != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    branch_layers = []
    for layer in layers:
        layer_fields = parse_struct(layer)
        reject_unknown_fields(layer_fields, {"BranchFilters"})
        bones = []
        for branch in parse_list(field(layer_fields, "BranchFilters")):
            branch_fields = parse_struct(branch)
            reject_unknown_fields(branch_fields, {"BoneName", "BlendDepth"})
            if parse_number(field(branch_fields, "BlendDepth", "0.0")) != 0.0:
                raise PoseContractError("source layered bone blend depth is unsupported")
            bones.append(parse_quoted(field(branch_fields, "BoneName")))
        if not bones:
            raise PoseContractError("source layered bone blend branch filter is empty")
        branch_layers.append(bones)
    if field(fields, "SkeletonGuid", "00000000000000000000000000000000") != "00000000000000000000000000000000":
        issues.append(issue(node, renderer, "source_layered_bone_skeleton_guid_unsupported", "SkeletonGuid"))
    if field(fields, "VirtualBoneGuid", "00000000000000000000000000000000") != "00000000000000000000000000000000":
        issues.append(issue(node, renderer, "source_layered_bone_virtual_guid_unsupported", "VirtualBoneGuid"))
    if parse_number(field(fields, "LODThreshold", "-1")) != -1.0:
        issues.append(issue(node, renderer, "source_lod_threshold_unsupported", "LODThreshold"))
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "branch_bone_layers": branch_layers,
        "blend_weights": weights,
        "mesh_space_rotation_blend": parse_bool(
            field(fields, "bMeshSpaceRotationBlend", "False")
        ),
        "blend_root_motion_based_on_root_bone": parse_bool(
            field(fields, "bBlendRootMotionBasedOnRootBone", "False")
        ),
        "base_pose_pin": "Base Pose",
        "blend_pose_pins": [f"Blend Poses {index}" for index in range(len(poses))],
        "output_pin": "Pose",
    }, issues


def cached_pose_contract(node, renderer):
    property_name = "CacheName" if renderer == "save_cached_pose" else "NameOfCache"
    source_name = node_property(node, property_name)
    if not source_name or source_name == "None":
        raise PoseContractError("cached pose name is invalid")
    required = ("Pose",) if renderer == "save_cached_pose" else ()
    inputs, issues = validate_inputs(node, renderer, required)
    return {"renderer": renderer, "input_pins": inputs, "cache_name": source_name}, issues


def linked_input_pose_contract(node):
    renderer = "linked_input_pose"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "Name",
            "InputPose",
            "bIsOutputLinked",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    inputs, issues = validate_inputs(node, renderer, ())
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    outputs = visible_output_names(node)
    if outputs != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "output_pins": outputs,
        "input_name": parse_quoted(field(fields, "Name")),
        "output_linked": parse_bool(field(fields, "bIsOutputLinked", "False")),
    }, issues


def inertialization_contract(node):
    renderer = "inertialization"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "Source",
            "FilteredCurves",
            "bResetOnBecomingRelevant",
            "bForwardRequestsThroughSkippedCachedPoseNodes",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    inputs, issues = validate_inputs(node, renderer, ("Source",))
    outputs = visible_output_names(node)
    if outputs != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"),
        )
    )
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "output_pins": outputs,
        "filtered_curves": [parse_quoted(value) for value in parse_list(field(fields, "FilteredCurves", "()"))],
        "reset_on_becoming_relevant": parse_bool(field(fields, "bResetOnBecomingRelevant", "True")),
        "forward_requests_through_skipped_cached_poses": parse_bool(
            field(fields, "bForwardRequestsThroughSkippedCachedPoseNodes", "True")
        ),
    }, issues


def pose_lifecycle_call_contract(node):
    renderer = "pose_lifecycle_call"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "Source",
            "Function",
            "CallSite",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    inputs, issues = validate_inputs(node, renderer, ("Source",))
    outputs = visible_output_names(node)
    if outputs != ["Pose"]:
        issues.append(issue(node, renderer, "source_pin_unsupported", "Output"))
    title = node.get("title")
    function = title.splitlines()[0].strip() if isinstance(title, str) else ""
    call_site = parse_name(field(fields, "CallSite"))
    if not function:
        issues.append(issue(node, renderer, "source_lifecycle_function_missing", "title"))
    if call_site not in {"OnInitialUpdate", "OnBecomeRelevant", "OnUpdate"}:
        issues.append(issue(node, renderer, "source_lifecycle_call_site_unsupported", call_site))
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "output_pins": outputs,
        "source_function": function,
        "call_site": call_site,
    }, issues


def state_result_contract(node):
    renderer = "state_result"
    fields = parse_struct(node_property(node, "Node"))
    reject_unknown_fields(
        fields,
        {
            "StateIndex",
            "StateEntryFunction",
            "StateFullyBlendedInFunction",
            "StateExitFunction",
            "StateFullyBlendedOutFunction",
            "Result",
            "LayerGroup",
            "InitialUpdateFunction",
            "BecomeRelevantFunction",
            "UpdateFunction",
        },
    )
    inputs, issues = validate_inputs(node, renderer, ("Result",))
    lifecycle = []
    for property_name, call_site in (
        ("StateEntryFunction", "OnStateEntry"),
        ("StateFullyBlendedInFunction", "OnStateFullyBlendedIn"),
        ("StateExitFunction", "OnStateExit"),
        ("StateFullyBlendedOutFunction", "OnStateFullyBlendedOut"),
        ("InitialUpdateFunction", "OnInitialUpdate"),
        ("BecomeRelevantFunction", "OnBecomeRelevant"),
        ("UpdateFunction", "OnUpdate"),
    ):
        value = field(fields, property_name, "()")
        if value == "()":
            continue
        reference = parse_struct(value)
        function = parse_quoted(field(reference, "FunctionName"))
        lifecycle.append({"call_site": call_site, "source_function": function})
    issues.extend(
        lifecycle_issues(
            node,
            renderer,
            fields,
            (
                "StateEntryFunction",
                "StateFullyBlendedInFunction",
                "StateExitFunction",
                "StateFullyBlendedOutFunction",
                "InitialUpdateFunction",
                "BecomeRelevantFunction",
                "UpdateFunction",
            ),
        )
    )
    return {
        "renderer": renderer,
        "input_pins": inputs,
        "layer_group": parse_quoted(field(fields, "LayerGroup", '"DefaultSharedGroup"')),
        "lifecycle": lifecycle,
    }, issues


def pose_contract(node, renderer):
    if renderer not in POSE_RENDERERS:
        return None, []
    try:
        if renderer == "sequence_player":
            return sequence_player_contract(node)
        if renderer == "sequence_evaluator":
            return sequence_evaluator_contract(node)
        if renderer == "two_way_blend":
            return two_way_blend_contract(node)
        if renderer == "apply_additive":
            return apply_additive_contract(node, False)
        if renderer == "apply_mesh_space_additive":
            return apply_additive_contract(node, True)
        if renderer == "modify_curve":
            return modify_curve_contract(node)
        if renderer == "multi_way_blend":
            return multi_way_blend_contract(node)
        if renderer == "set_hips_direction":
            return set_hips_direction_contract(node)
        if renderer == "enum_pose_blend":
            return enum_pose_blend_contract(node)
        if renderer == "layered_bone_blend":
            return layered_bone_blend_contract(node)
        if renderer in {"save_cached_pose", "use_cached_pose"}:
            return cached_pose_contract(node, renderer)
        if renderer == "linked_input_pose":
            return linked_input_pose_contract(node)
        if renderer == "inertialization":
            return inertialization_contract(node)
        if renderer == "pose_lifecycle_call":
            return pose_lifecycle_call_contract(node)
        if renderer == "state_result":
            return state_result_contract(node)
    except (ExportedTextError, PoseContractError) as error:
        return None, [issue(node, renderer, "source_property_unsupported", str(error))]
    raise AssertionError("pose renderer is not dispatched")


def fixture_node(renderer, property_name, property_value, pins):
    return {
        "source_node": "/Asset:AnimGraph.Node",
        "source_class": "/Script/AnimGraph.Fixture",
        "renderer": renderer,
        "properties": [{"name": property_name, "exported_text": property_value}],
        "pins": pins,
    }


def input_pin(name):
    return {
        "name": name,
        "direction": "Input",
        "hidden": False,
        "default_value": "1.000000",
    }


def output_pin(name):
    return {"name": name, "direction": "Output", "hidden": False}


def run_self_test():
    two_way = fixture_node(
        "two_way_blend",
        "BlendNode",
        "(A=(),B=(),AlphaScaleBias=(Scale=1.000000),AlphaScaleBiasClamp=(bMapRange=True,bInterpResult=True,InRange=(Min=-500.000000,Max=-1000.000000),OutRange=(Max=1.000000),Scale=1.000000,ClampMax=1.000000,InterpSpeedIncreasing=5.000000,InterpSpeedDecreasing=5.000000),InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("A"), input_pin("B"), input_pin("Alpha")],
    )
    contract, issues = pose_contract(two_way, "two_way_blend")
    assert not issues
    assert contract["alpha_mapping"]["input_max"] == -1000.0
    sequence = fixture_node(
        "sequence_player",
        "Node",
        "(GroupName=\"Movement\",Method=SyncGroup,Sequence=\"/Script/Engine.AnimSequence'/ALS/Animations/A_Test.A_Test'\",PlayRateBasis=1.000000,PlayRate=1.000000,PlayRateScaleBiasClampConstants=(),bLoopAnimation=True,PlayRateScaleBiasClampState=(),InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("PlayRate")],
    )
    _, issues = pose_contract(sequence, "sequence_player")
    assert issues[0]["kind"] == "source_sync_group_unsupported"
    contract, _ = pose_contract(sequence, "sequence_player")
    assert contract["sync_group_role"] == "CanBeLeader"
    curve = fixture_node(
        "modify_curve",
        "Node",
        "(SourcePose=(),CurveValues=(0.000000,1.000000),CurveNames=(\"A\",\"B\"),Alpha=1.000000,ApplyMode=Blend,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("SourcePose"), input_pin("CurveValues_0"), input_pin("CurveValues_1"), input_pin("Alpha")],
    )
    contract, issues = pose_contract(curve, "modify_curve")
    assert not issues
    assert contract["alpha_visible"]
    curve["binding"] = {
        "properties": [
            {
                "name": "PropertyBindings",
                "exported_text": '(("CurveValues_0", (PropertyName="CurveValues_0",PathAsText="GetParent.GroundedState.RotationYawOffsets.ForwardAngle",bIsBound=True)))',
            }
        ]
    }
    contract, issues = pose_contract(curve, "modify_curve")
    assert not issues
    assert contract["curve_value_paths"] == [
        "GroundedState.RotationYawOffsets.ForwardAngle",
        "",
    ]
    curve["pins"][-1]["default_value"] = "0.500000"
    _, issues = pose_contract(curve, "modify_curve")
    assert issues[-1]["kind"] == "source_curve_alpha_default_mismatch"
    multi = fixture_node(
        "multi_way_blend",
        "Node",
        "(Poses=((),()),DesiredAlphas=(0.250000,0.750000),AlphaScaleBias=(Scale=1.000000),bNormalizeAlpha=True,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("Poses_0"), input_pin("Poses_1"), input_pin("DesiredAlphas_0"), input_pin("DesiredAlphas_1"), output_pin("Pose")],
    )
    multi["pins"][2]["default_value"] = "0.250000"
    multi["pins"][3]["default_value"] = "0.750000"
    multi["binding"] = {
        "properties": [
            {
                "name": "PropertyBindings",
                "exported_text": '(("DesiredAlphas_0", (PropertyName="DesiredAlphas_0",PathAsText="GetParent.GroundedState.VelocityBlend.ForwardAmount",bIsBound=True)),("DesiredAlphas_1", (PropertyName="DesiredAlphas_1",PathAsText="GetParent.GroundedState.VelocityBlend.BackwardAmount",bIsBound=True)))',
            }
        ]
    }
    multi_contract, issues = pose_contract(multi, "multi_way_blend")
    assert not issues
    assert multi_contract["pose_count"] == 2
    assert multi_contract["desired_alpha_paths"] == [
        "GroundedState.VelocityBlend.ForwardAmount",
        "GroundedState.VelocityBlend.BackwardAmount",
    ]
    hips = fixture_node(
        "set_hips_direction",
        "Node",
        "(Source=(LinkID=-1,SourceLinkID=-1),Function=(),CallSite=OnBecomeRelevant,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("Source"), input_pin("NewHipsDirection"), output_pin("Pose")],
    )
    hips["pins"][1]["default_value"] = "LeftBackward"
    hips_contract, issues = pose_contract(hips, "set_hips_direction")
    assert not issues
    assert hips_contract["new_hips_direction"] == "LeftBackward"
    enum_blend = fixture_node(
        "enum_pose_blend",
        "Node",
        "(BlendPose=((),()),BlendTime=(0.100000,0.100000),InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [
            input_pin("BlendPose_0"),
            input_pin("BlendPose_1"),
            input_pin("BlendTime_0"),
            input_pin("BlendTime_1"),
            input_pin("ActiveEnumValue"),
            output_pin("Pose"),
        ],
    )
    enum_blend["title"] = "Blend Poses (EAlsHipsDirection)"
    for pin, label in zip(
        enum_blend["pins"],
        (
            "Default Pose",
            "Left Backward Pose",
            "Default Blend Time",
            "Left Backward Blend Time",
            "Active Enum Value",
            "",
        ),
    ):
        pin["friendly_name"] = label
    enum_blend["pins"][4]["default_value"] = "0"
    enum_blend["binding"] = {
        "properties": [
            {
                "name": "PropertyBindings",
                "exported_text": '(("ActiveEnumValue", (PropertyName="ActiveEnumValue",PathAsText="GetParent.GroundedState.HipsDirection",bIsBound=True)))',
            }
        ]
    }
    enum_blend_contract, issues = pose_contract(enum_blend, "enum_pose_blend")
    assert not issues
    assert enum_blend_contract["selected_enum_entry"] == "LeftBackward"
    assert enum_blend_contract["active_enum_path"] == "GroundedState.HipsDirection"
    layered = fixture_node(
        "layered_bone_blend",
        "Node",
        '(BasePose=(),BlendPoses=((),()),LayerSetup=((BranchFilters=((BoneName="thigh_l"),(BoneName="ik_foot_l"))),(BranchFilters=((BoneName="thigh_r")))),BlendWeights=(1.000000,0.500000),SkeletonGuid=00000000000000000000000000000000,VirtualBoneGuid=00000000000000000000000000000000,LODThreshold=-1,bMeshSpaceRotationBlend=True,bBlendRootMotionBasedOnRootBone=True,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())',
        [input_pin("BasePose"), input_pin("BlendPoses_0"), input_pin("BlendPoses_1"), output_pin("Pose")],
    )
    layered_contract, issues = pose_contract(layered, "layered_bone_blend")
    assert not issues
    assert layered_contract["branch_bone_layers"] == [
        ["thigh_l", "ik_foot_l"],
        ["thigh_r"],
    ]
    state = fixture_node(
        "state_result",
        "Node",
        "(StateIndex=-1,StateEntryFunction=(),StateFullyBlendedInFunction=(),StateExitFunction=(FunctionName=\"Exit\"),StateFullyBlendedOutFunction=(),Result=(),LayerGroup=\"DefaultSharedGroup\",InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("Result")],
    )
    _, issues = pose_contract(state, "state_result")
    assert issues[0]["kind"] == "source_lifecycle_unsupported"
    state_contract, _ = pose_contract(state, "state_result")
    assert state_contract["lifecycle"] == [
        {"call_site": "OnStateExit", "source_function": "Exit"}
    ]
    linked = fixture_node(
        "linked_input_pose",
        "Node",
        '(Name="Grounded Input",InputPose=(),bIsOutputLinked=True,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())',
        [output_pin("Pose")],
    )
    linked_contract, issues = pose_contract(linked, "linked_input_pose")
    assert not issues
    assert linked_contract["input_name"] == "Grounded Input"
    inertial = fixture_node(
        "inertialization",
        "Node",
        '(Source=(),FilteredCurves=("RotationYawSpeed"),bResetOnBecomingRelevant=True,bForwardRequestsThroughSkippedCachedPoseNodes=True,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())',
        [input_pin("Source"), output_pin("Pose")],
    )
    inertial_contract, issues = pose_contract(inertial, "inertialization")
    assert not issues
    assert inertial_contract["filtered_curves"] == ["RotationYawSpeed"]
    lifecycle = fixture_node(
        "pose_lifecycle_call",
        "Node",
        "(Source=(),Function=(),CallSite=OnUpdate,InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("Source"), output_pin("Pose")],
    )
    lifecycle["title"] = "RefreshInAir\nOn Update"
    lifecycle_contract, issues = pose_contract(lifecycle, "pose_lifecycle_call")
    assert not issues
    assert lifecycle_contract["source_function"] == "RefreshInAir"
    return {
        "two_way_alpha_mapping_contract_parsed": "PASS",
        "sequence_sync_group_blocked": "PASS",
        "modify_curve_alpha_pin_contract_parsed": "PASS",
        "modify_curve_property_binding_paths_preserved": "PASS",
        "state_result_lifecycle_blocked": "PASS",
        "linked_input_inertial_and_pose_lifecycle_contracts_parsed": "PASS",
        "multi_way_blend_contract_parsed": "PASS",
        "source_property_binding_paths_preserved": "PASS",
        "source_set_hips_direction_contract_parsed": "PASS",
        "source_enum_pose_blend_contract_parsed": "PASS",
        "source_layered_bone_blend_contract_parsed": "PASS",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
