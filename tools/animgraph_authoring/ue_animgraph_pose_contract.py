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


def visible_input_default(node, name):
    values = [
        pin.get("default_value")
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden") and pin.get("name") == name
    ]
    if len(values) != 1:
        raise PoseContractError(f"source input pin is missing or ambiguous: {name}")
    return parse_number(values[0])


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
    return {
        "input_pins": names,
        "alpha_visible": "Alpha" in names,
        "alpha_default": parse_number(field(fields, "Alpha", "0.0")),
        "alpha_scale_bias": parse_scale_bias(field(fields, "AlphaScaleBias", "()")),
        "alpha_mapping": parse_scale_bias_clamp(field(fields, "AlphaScaleBiasClamp", "()")),
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
    return {
        "renderer": renderer,
        "input_pins": names,
        "sequence": parse_quoted(field(fields, "Sequence")),
        "play_rate_basis": parse_number(field(fields, "PlayRateBasis", "1.0")),
        "play_rate": parse_number(field(fields, "PlayRate", "1.0")),
        "play_rate_mapping": parse_scale_bias_clamp(
            field(fields, "PlayRateScaleBiasClampConstants", "()")
        ),
        "start_position": parse_number(field(fields, "StartPosition", "0.0")),
        "loop_animation": parse_bool(field(fields, "bLoopAnimation", "True")),
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
    return {
        "renderer": renderer,
        "input_pins": input_names,
        "curve_names": names,
        "curve_values": values,
        "alpha": alpha,
        "alpha_visible": alpha_visible,
        "apply_mode": parse_name(field(fields, "ApplyMode", "Blend")),
    }, issues


def cached_pose_contract(node, renderer):
    property_name = "CacheName" if renderer == "save_cached_pose" else "NameOfCache"
    source_name = node_property(node, property_name)
    if not source_name or source_name == "None":
        raise PoseContractError("cached pose name is invalid")
    required = ("Pose",) if renderer == "save_cached_pose" else ()
    inputs, issues = validate_inputs(node, renderer, required)
    return {"renderer": renderer, "input_pins": inputs, "cache_name": source_name}, issues


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
        if renderer in {"save_cached_pose", "use_cached_pose"}:
            return cached_pose_contract(node, renderer)
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
    curve["pins"][-1]["default_value"] = "0.500000"
    _, issues = pose_contract(curve, "modify_curve")
    assert issues[-1]["kind"] == "source_curve_alpha_default_mismatch"
    state = fixture_node(
        "state_result",
        "Node",
        "(StateIndex=-1,StateEntryFunction=(),StateFullyBlendedInFunction=(),StateExitFunction=(FunctionName=\"Exit\"),StateFullyBlendedOutFunction=(),Result=(),LayerGroup=\"DefaultSharedGroup\",InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())",
        [input_pin("Result")],
    )
    _, issues = pose_contract(state, "state_result")
    assert issues[0]["kind"] == "source_lifecycle_unsupported"
    return {
        "two_way_alpha_mapping_contract_parsed": "PASS",
        "sequence_sync_group_blocked": "PASS",
        "modify_curve_alpha_pin_contract_parsed": "PASS",
        "state_result_lifecycle_blocked": "PASS",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
