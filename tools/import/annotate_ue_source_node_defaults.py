import argparse
import json
import re
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from animgraph_context_contract import parse_nodes
from audit_ue_visible_animgraph import source_binding_paths, visible_pins


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def source_text(node):
    values = list(node.get("properties", []))
    binding = node.get("binding")
    if isinstance(binding, dict):
        values.extend(binding.get("properties", []))
    return "\n".join(
        value.get("exported_text", "")
        for value in values
        if isinstance(value, dict)
    )


def mapped_pin(node_map, source_pin):
    for pin in node_map.get("input_pins", []):
        if pin.get("source") == source_pin:
            return pin
    raise ValueError(f"mapped input pin is missing: {source_pin}")


def source_pin(node, name):
    for pin in visible_pins(node, "Input"):
        if pin.get("name") == name:
            return pin
    raise ValueError(f"source input pin is missing: {name}")


def add_assertion(node_map, source_contains, target_contains, target_runtime_path=None):
    assertions = node_map.setdefault("property_assertions", [])
    if target_runtime_path is not None:
        assertions[:] = [
            assertion
            for assertion in assertions
            if not (
                isinstance(assertion, dict)
                and assertion.get("source_contains") == source_contains
                and assertion.get("target_contains") == target_contains
                and "target_runtime_path" not in assertion
            )
        ]
    value = {"source_contains": source_contains, "target_contains": target_contains}
    if target_runtime_path is not None:
        value["target_runtime_path"] = target_runtime_path
    if value not in assertions:
        assertions.append(value)


def add_exception(node_map, source_contains, target_exception, target_runtime_path):
    assertions = node_map.setdefault("property_assertions", [])
    value = {
        "source_contains": source_contains,
        "target_exception": target_exception,
        "target_runtime_path": target_runtime_path,
    }
    if value not in assertions:
        assertions.append(value)


def remove_empty_target_assertions(node_map):
    assertions = node_map.get("property_assertions")
    if not isinstance(assertions, list):
        return
    node_map["property_assertions"] = [
        assertion
        for assertion in assertions
        if not (
            isinstance(assertion, dict)
            and assertion.get("target_contains") == ""
            and "target_exception" not in assertion
        )
    ]


def scalar_literal(entry, field):
    match = re.search(rf'\b{re.escape(field)}\s*:\s*(-?[0-9]+(?:\.[0-9]+)?)', entry)
    if match is None:
        raise ValueError(f"target scalar is missing: {field}")
    return match.group(0)


def string_literal(entry, field):
    match = re.search(rf'\b{re.escape(field)}\s*:\s*"([^"]*)"', entry)
    if match is None:
        raise ValueError(f"target string is missing: {field}")
    return match.group(0), match.group(1)


def bool_literal(entry, field):
    match = re.search(rf'\b{re.escape(field)}\s*:\s*(true|false)', entry)
    if match is None:
        raise ValueError(f"target bool is missing: {field}")
    return match.group(0)


def array_literal(entry, field):
    match = re.search(rf'\b{re.escape(field)}\s*:\s*\[([^\]]*)\]', entry)
    if match is None:
        raise ValueError(f"target array is missing: {field}")
    return match.group(0), match.group(1)


def bracket_literal(entry, field):
    match = re.search(rf'\b{re.escape(field)}\s*:\s*\[', entry)
    if match is None:
        raise ValueError(f"target array is missing: {field}")
    start = entry.find("[", match.start())
    depth = 0
    for index in range(start, len(entry)):
        if entry[index] == "[":
            depth += 1
        elif entry[index] == "]":
            depth -= 1
            if depth == 0:
                return entry[match.start() : index + 1], entry[start : index + 1]
    raise ValueError(f"target array is malformed: {field}")


def nested_string_arrays(entry, field):
    target, values = bracket_literal(entry, field)
    layers = [
        re.findall(r'"([^"]*)"', layer)
        for layer in re.findall(r'\[([^\[\]]*)\]', values)
    ]
    return target, layers


def global_input_fields(entry, pin):
    match = re.search(
        rf'\(pin:\s*"{re.escape(pin)}",\s*key:\s*"([^"]+)"([^\)]*)\)',
        entry,
    )
    if match is None:
        raise ValueError(f"target global input is missing: {pin}")
    default = re.search(
        r'\bdefault_value\s*:\s*(-?[0-9]+(?:\.[0-9]+)?)',
        match.group(2),
    )
    if default is None:
        raise ValueError(f"target global input default is missing: {pin}")
    key = match.group(1)
    return f'key: "{key}"', key, default.group(0)


def scalar_or_global_default(entry, field, pin):
    try:
        return scalar_literal(entry, field)
    except ValueError:
        return global_input_fields(entry, pin)[2]


def runtime_text(root, entry):
    try:
        _, path = string_literal(entry, "graph")
    except ValueError:
        return entry, None
    runtime_path = root / path
    relative_path = path
    if not runtime_path.is_file():
        runtime_path = root / "assets" / path
        relative_path = (Path("assets") / path).as_posix()
    if not runtime_path.is_file():
        raise ValueError(f"target runtime graph is missing: {path}")
    return runtime_path.read_text(encoding="utf-8"), relative_path


def annotate_sequence_player(source, node_map, target_entry):
    bindings = source_binding_paths(source)
    for source_name, target_field, target_key_field in (
        ("PlayRate", "play_rate", "play_rate_key"),
        ("StartPosition", "start_position", None),
    ):
        try:
            source_default = source_pin(source, source_name).get("default_value")
        except ValueError:
            continue
        if not isinstance(source_default, str) or not source_default:
            raise ValueError(f"source sequence default is missing: {source_name}")
        mapping = mapped_pin(node_map, source_name)
        mapping["source_default"] = source_default
        mapping["target_default_contains"] = scalar_or_global_default(
            target_entry,
            target_field,
            mapping["target"],
        )
        source_binding = bindings.get(source_name)
        if source_binding is not None and target_key_field is not None:
            try:
                target_key, key = string_literal(target_entry, target_key_field)
            except ValueError:
                target_key, key, _ = global_input_fields(target_entry, mapping["target"])
            if key:
                mapping["target_binding"] = key
                add_assertion(node_map, source_binding, target_key)
    try:
        loop_default = source_pin(source, "bLoopAnimation").get("default_value")
    except ValueError:
        loop_default = None
    if isinstance(loop_default, str) and loop_default:
        mapping = mapped_pin(node_map, "bLoopAnimation")
        mapping["source_default"] = loop_default
        mapping["target_default_contains"] = bool_literal(target_entry, "loop_animation")
        source_binding = bindings.get("bLoopAnimation")
        if source_binding is not None:
            target_key, key = string_literal(target_entry, "loop_animation_key")
            if key:
                mapping["target_binding"] = key
                add_assertion(node_map, source_binding, target_key)
    source = source_text(source)
    try:
        target_group, group = string_literal(target_entry, "sync_group")
    except ValueError:
        target_group, group = "", ""
    if group:
        add_assertion(node_map, f'GroupName="{group}"', target_group)
    try:
        target_clip, clip = string_literal(target_entry, "clip")
    except ValueError:
        target_clip, clip = "", ""
    asset = re.search(r"(A_Als_[A-Za-z0-9_]+)", clip)
    if asset is not None and target_clip:
        add_assertion(node_map, asset.group(1), target_clip)
    if "Method=SyncGroup" in source and target_group:
        add_assertion(node_map, "Method=SyncGroup", target_group)
    reference = re.search(
        r'\badditive_reference\s*:\s*Some\("([^"]+)"\)', target_entry
    )
    if reference is not None and asset is not None and target_clip:
        add_assertion(node_map, asset.group(1), reference.group(0))


def annotate_apply_additive(source, node_map, target_entry):
    source = source_text(source)
    target_alpha = scalar_literal(target_entry, "constant_alpha")
    target_enabled = re.search(r"\balpha_bool_enabled\s*:\s*(true|false)", target_entry)
    if target_enabled is None:
        raise ValueError("target additive alpha bool is missing")
    if "Alpha=1.000000" in source:
        add_assertion(node_map, "Alpha=1.000000", target_alpha)
    if "bAlphaBoolEnabled=True" in source:
        add_assertion(
            node_map,
            "bAlphaBoolEnabled=True",
            target_enabled.group(0),
        )
    alpha_curve = re.search(r"AlphaCurveName=([^,\)]+)", source)
    if alpha_curve is not None:
        target_curve, _ = string_literal(target_entry, "alpha_curve_name")
        add_assertion(node_map, alpha_curve.group(0), target_curve)


def annotate_curve_modify(source, node_map, target_entry):
    source_values = source_text(source)
    curve_names = re.search(r"CurveNames=\(([^\)]*)\)", source_values)
    try:
        target_defaults, _ = array_literal(target_entry, "pin_defaults")
    except ValueError:
        target_defaults = scalar_literal(target_entry, "pin_default")
    try:
        target_curves, target_curve_values = array_literal(target_entry, "curve_names")
    except ValueError:
        target_curves, curve = string_literal(target_entry, "curve_name")
        target_curve_values = json.dumps(curve)
    source_curve_names = (
        re.findall(r'"([^\"]+)"', curve_names.group(1))
        if curve_names is not None
        else []
    )
    target_curve_names = re.findall(r'"([^\"]+)"', target_curve_values)
    if source_curve_names and source_curve_names != target_curve_names:
        raise ValueError("target curve names differ from source")
    if curve_names is not None:
        add_assertion(node_map, curve_names.group(0), target_curves)
    bindings = source_binding_paths(source)
    for index, curve_name in enumerate(source_curve_names):
        source_name = f"CurveValues_{index}"
        source_default = source_pin(source, source_name).get("default_value")
        if not isinstance(source_default, str) or not source_default:
            raise ValueError("source curve default is missing")
        mapping = mapped_pin(node_map, source_name)
        mapping["source_default"] = source_default
        mapping["target_default_contains"] = target_defaults
        source_binding = bindings.get(source_name)
        if source_binding is not None:
            try:
                target_keys, target_key_values = array_literal(target_entry, "value_keys")
            except ValueError:
                target_keys, key = string_literal(target_entry, "value_key")
                target_key_values = json.dumps(key)
            try:
                target_source_paths, _ = array_literal(target_entry, "source_value_paths")
            except ValueError:
                target_source_paths, _ = string_literal(target_entry, "source_value_path")
            target_value_keys = re.findall(r'"([^\"]*)"', target_key_values)
            if index >= len(target_value_keys) or not target_value_keys[index]:
                raise ValueError("target curve value key is missing")
            mapping["target_binding"] = target_value_keys[index]
            add_assertion(node_map, source_binding, target_keys)
            add_assertion(node_map, source_binding, target_source_paths)
    for source_contains, target_field in (
        ("Alpha=1.000000", "alpha"),
        ("ApplyMode=Scale", "apply_mode"),
        ("ApplyMode=Blend", "apply_mode"),
    ):
        if source_contains in source_values:
            if target_field == "alpha":
                target = scalar_literal(target_entry, target_field)
            else:
                target, _ = string_literal(target_entry, target_field)
            add_assertion(node_map, source_contains, target)


def annotate_sequence_evaluator(root, source, node_map, target_entry):
    source_default = source_pin(source, "ExplicitFrame").get("default_value")
    if not isinstance(source_default, str) or not source_default:
        raise ValueError("source explicit frame default is missing")
    mapping = mapped_pin(node_map, "ExplicitFrame")
    mapping["source_default"] = source_default
    mapping["target_default_contains"] = scalar_or_global_default(
        target_entry,
        "explicit_frame",
        mapping["target"],
    )
    mapping["type_exception"] = "UE frame count is represented as target F32."
    source_values = source_text(source)
    target_text, runtime_path = runtime_text(root, target_entry)
    target_clip, clip = string_literal(target_text, "clip")
    asset = re.search(r"(A_Als_[A-Za-z0-9_]+)", clip)
    if asset is not None:
        add_assertion(node_map, asset.group(1), target_clip, runtime_path)
    for source_contains, target_field in (
        ("bUseExplicitFrame=True", "explicit_frame_pin"),
        ("bShouldLoop=True", "should_loop"),
        ("bTeleportToExplicitTime=True", "teleport_to_explicit_time"),
        ("ReinitializationBehavior=ExplicitTime", "reinitialization_behavior"),
    ):
        if source_contains in source_values:
            if target_field in {"should_loop", "teleport_to_explicit_time"}:
                target = bool_literal(target_text, target_field)
            else:
                target, _ = string_literal(target_text, target_field)
            add_assertion(node_map, source_contains, target, runtime_path)


def annotate_slot(source, node_map, target_entry):
    source_values = source_text(source)
    slot_name = re.search(r'SlotName="([^\"]+)"', source_values)
    if slot_name is not None:
        target_slot, _ = string_literal(target_entry, "slot_name")
        add_assertion(node_map, slot_name.group(0), target_slot)


def annotate_call_function(source, node_map, target_entry):
    source_values = source_text(source)
    call_site = re.search(r"CallSite=([A-Za-z]+)", source_values)
    if call_site is not None:
        target_site, _ = string_literal(target_entry, "call_site")
        add_assertion(node_map, call_site.group(0), target_site)
    function = re.search(r"MemberName=([A-Za-z][A-Za-z0-9_]*)", source_values)
    if function is not None:
        target_function, _ = string_literal(target_entry, "source_function")
        add_assertion(node_map, function.group(0), target_function)
    try:
        hips_default = source_pin(source, "NewHipsDirection").get("default_value")
    except ValueError:
        hips_default = None
    if isinstance(hips_default, str) and hips_default:
        mapping = mapped_pin(node_map, "NewHipsDirection")
        mapping["source_default"] = hips_default
        mapping["target_default_contains"] = string_literal(
            target_entry,
            "new_hips_direction",
        )[0]
        mapping["type_exception"] = "UE hips direction enum is represented as target F32 scalar."


def annotate_cached_pose(source, node_map, target_entry):
    source_values = source_text(source)
    match = re.search(r"NameOfCache=([^,\)]+)", source_values)
    cache_name = match.group(1) if match is not None else None
    if cache_name is None:
        for property_value in source.get("properties", []):
            if property_value.get("name") == "NameOfCache":
                value = property_value.get("exported_text")
                if isinstance(value, str) and value:
                    cache_name = value
                break
    if cache_name is None:
        raise ValueError("source cache name is missing")
    target, cache = string_literal(target_entry, "cache_node")
    if cache != cache_name and f"'{cache_name}'" not in cache:
        raise ValueError("target cache name differs from source")
    node_map["property_assertions"] = [
        value
        for value in node_map.get("property_assertions", [])
        if not (
            value.get("source_contains") == cache_name
            and str(value.get("target_contains", "")).startswith("cache_node:")
        )
    ]
    add_assertion(node_map, cache_name, target)


def annotate_multi_way_blend(source, node_map, target_entry):
    source_defaults, _ = array_literal(target_entry, "desired_alpha_defaults")
    _, keys_text = array_literal(target_entry, "desired_alpha_keys")
    keys = re.findall(r'"([^"]*)"', keys_text)
    bindings = source_binding_paths(source)
    for pin in visible_pins(source, "Input"):
        name = pin.get("name", "")
        match = re.fullmatch(r"DesiredAlphas_(\d+)", name)
        if match is None:
            continue
        index = int(match.group(1))
        if index >= len(keys) or not keys[index]:
            raise ValueError(f"target desired alpha key is missing: {name}")
        mapping = mapped_pin(node_map, name)
        default = pin.get("default_value")
        if not isinstance(default, str) or not default:
            raise ValueError(f"source desired alpha default is missing: {name}")
        mapping["source_default"] = default
        mapping["target_default_contains"] = source_defaults
        mapping["target_binding"] = keys[index]
        source_binding = bindings.get(name)
        if source_binding is not None:
            add_assertion(node_map, source_binding, keys[index])


def annotate_two_way_blend(source, node_map, target_entry):
    try:
        source_default = source_pin(source, "Alpha").get("default_value")
    except ValueError:
        return
    if not isinstance(source_default, str) or not source_default:
        raise ValueError("source two way blend alpha default is missing")
    mapping = mapped_pin(node_map, "Alpha")
    mapping["source_default"] = source_default
    mapping["target_default_contains"] = scalar_literal(target_entry, "pin_default")
    source_binding = source_binding_paths(source).get("Alpha")
    if source_binding is not None:
        target_key, key = string_literal(target_entry, "alpha_key")
        if key:
            mapping["target_binding"] = key
            add_assertion(node_map, source_binding, target_key)


def annotate_blendspace_evaluator(source, node_map, target_entry):
    bindings = source_binding_paths(source)
    for pin in visible_pins(source, "Input"):
        default = pin.get("default_value")
        if not isinstance(default, str) or not default:
            continue
        mapping = mapped_pin(node_map, pin["name"])
        target_key, key, target_default = global_input_fields(target_entry, mapping["target"])
        mapping["source_default"] = default
        mapping["target_default_contains"] = target_default
        source_binding = bindings.get(pin["name"])
        if source_binding is not None:
            mapping["target_binding"] = key
            add_assertion(node_map, source_binding, target_key)


def annotate_enum_pose_blend(source, node_map, target_entry):
    source_values = source_text(source)
    for source_name, target_field in (
        ("BlendTime_0", "default_blend_time"),
        ("BlendTime_1", "selected_blend_time"),
        ("ActiveEnumValue", "active_enum_default"),
    ):
        source_default = source_pin(source, source_name).get("default_value")
        if not isinstance(source_default, str) or not source_default:
            raise ValueError(f"source enum blend default is missing: {source_name}")
        mapping = mapped_pin(node_map, source_name)
        mapping["source_default"] = source_default
        mapping["target_default_contains"] = scalar_literal(target_entry, target_field)
        if source_name == "ActiveEnumValue":
            mapping["type_exception"] = "UE byte enum is represented as target F32 scalar."
            source_binding = source_binding_paths(source).get(source_name)
            target_key, key = string_literal(target_entry, "active_key")
            if source_binding is not None:
                if not key:
                    raise ValueError("target enum active key is missing")
                mapping["target_binding"] = key
                add_assertion(node_map, source_binding, target_key)
    blend_times = re.search(r"BlendTime=\(([^\)]*)\)", source_values)
    if blend_times is not None:
        add_assertion(
            node_map,
            blend_times.group(0),
            scalar_literal(target_entry, "default_blend_time"),
        )
        add_assertion(
            node_map,
            blend_times.group(0),
            scalar_literal(target_entry, "selected_blend_time"),
        )


def annotate_layered_bone_blend(source, node_map, target_entry):
    source_values = source_text(source)
    source_layers = [
        re.findall(r'BoneName="([^"]+)"', value)
        for value in re.findall(r"BranchFilters=\(\((.*?)\)\)", source_values)
    ]
    if source_layers:
        target_layers, target_values = nested_string_arrays(
            target_entry, "branch_bone_layers"
        )
        if source_layers != target_values:
            raise ValueError("target layered bone branches differ from source")
        add_assertion(node_map, "LayerSetup=", target_layers)
    source_weights = re.search(r"BlendWeights=\(([^\)]*)\)", source_values)
    if source_weights is not None:
        target_weights, target_values = array_literal(target_entry, "blend_weights")
        source_values_list = [float(value) for value in source_weights.group(1).split(",")]
        target_values_list = [
            float(value) for value in re.findall(r"-?[0-9]+(?:\.[0-9]+)?", target_values)
        ]
        if source_values_list != target_values_list:
            raise ValueError("target layered bone weights differ from source")
        add_assertion(node_map, source_weights.group(0), target_weights)
    for source_field, target_field in (
        ("bMeshSpaceRotationBlend", "mesh_space_rotation_blend"),
        ("bBlendRootMotionBasedOnRootBone", "blend_root_motion_based_on_root_bone"),
    ):
        source_value = re.search(rf"{source_field}=(True|False)", source_values)
        if source_value is not None:
            target_value = bool_literal(target_entry, target_field)
            expected = source_value.group(1) == "True"
            if target_value != f"{target_field}: {str(expected).lower()}":
                raise ValueError(f"target layered bone {target_field} differs from source")
            add_assertion(node_map, source_value.group(0), target_value)
    for source_field, target_exception in (
        ("LODThreshold=-1", "Target graph evaluates without LOD culling."),
        (
            "SkeletonGuid=00000000000000000000000000000000",
            "Target resolves the skeleton from evaluated poses.",
        ),
        (
            "VirtualBoneGuid=00000000000000000000000000000000",
            "Target resolves virtual-bone paths from the evaluated skeleton.",
        ),
    ):
        if source_field in source_values:
            add_exception(
                node_map,
                source_field,
                target_exception,
                "crates/als-ext/src/source_stop_nodes.rs",
            )


def annotate(root, mapping):
    contract_path = root / mapping["source_contract"]
    target_path = root / mapping["target_graph"]
    if not contract_path.is_file() or not target_path.is_file():
        raise ValueError("source contract or target graph is missing")
    contract = read_json(contract_path)
    source_nodes = {
        node["object_path"]: node for node in contract["source_graph"]["nodes"]
    }
    target_nodes, _ = parse_nodes(target_path.read_text(encoding="utf-8"))
    result = json.loads(json.dumps(mapping))
    for node_map in result.get("node_map", []):
        remove_empty_target_assertions(node_map)
        source = source_nodes.get(node_map.get("source_node"))
        target = target_nodes.get(node_map.get("target_node"))
        if source is None or target is None:
            raise ValueError("source or target node is missing")
        class_path = source.get("class_path", "")
        if class_path.endswith("AnimGraphNode_SequencePlayer"):
            annotate_sequence_player(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_ApplyAdditive"):
            annotate_apply_additive(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_ModifyCurve"):
            annotate_curve_modify(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_SequenceEvaluator"):
            annotate_sequence_evaluator(root, source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_Slot"):
            annotate_slot(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_CallFunction"):
            annotate_call_function(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_UseCachedPose"):
            annotate_cached_pose(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_MultiWayBlend"):
            annotate_multi_way_blend(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_TwoWayBlend"):
            annotate_two_way_blend(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_BlendSpaceEvaluator"):
            annotate_blendspace_evaluator(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_BlendListByEnum"):
            annotate_enum_pose_blend(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_LayeredBoneBlend"):
            annotate_layered_bone_blend(source, node_map, target["entry"])
    return result


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def run_self_test():
    source = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": "sequence",
                    "class_path": "AnimGraphNode_SequencePlayer",
                    "pins": [
                        {"name": "PlayRate", "direction": "Input", "hidden": False, "default_value": "1.250000"},
                        {"name": "StartPosition", "direction": "Input", "hidden": False, "default_value": "0.250000"},
                    ],
                    "properties": [{"exported_text": "(GroupName=\"Fixture\",Method=SyncGroup,Sequence=\"A_Als_Fixture\")"}],
                },
                {
                    "object_path": "blend",
                    "class_path": "AnimGraphNode_MultiWayBlend",
                    "pins": [{"name": "DesiredAlphas_0", "direction": "Input", "hidden": False, "default_value": "0.0"}],
                    "binding": {"properties": [{"name": "PropertyBindings", "exported_text": "((\"DesiredAlphas_0\", (PathAsText=\"GetParent.Value\")))"}]},
                },
            ]
        }
    }
    mapping = {
        "source_contract": "source.json",
        "target_graph": "target.ron",
        "node_map": [
            {"source_node": "sequence", "target_node": "sequence", "input_pins": [{"source": "PlayRate", "target": "Play Rate"}, {"source": "StartPosition", "target": "Start Position"}], "output_pins": []},
            {"source_node": "blend", "target_node": "blend", "input_pins": [{"source": "DesiredAlphas_0", "target": "Desired Alphas 0"}], "output_pins": []},
        ],
    }
    text = "(nodes: [(id: \"sequence\", name: \"sequence\", inner: { \"x\": (play_rate: 1.25, start_position: 0.25, sync_group: \"Fixture\", clip: \"A_Als_Fixture\") }, should_debug: false), (id: \"blend\", name: \"blend\", inner: { \"x\": (desired_alpha_keys: [\"fixture_value\"], desired_alpha_defaults: [0.0]) }, should_debug: false)], edges_inverted: {}, io_spec: (), default_data: {}, editor_metadata: ())"
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "source.json").write_text(json.dumps(source), encoding="utf-8")
        (root / "target.ron").write_text(text, encoding="utf-8")
        result = annotate(root, mapping)
        sequence = result["node_map"][0]
        blend = result["node_map"][1]
        assert sequence["input_pins"][0]["target_default_contains"] == "play_rate: 1.25"
        assert blend["input_pins"][0]["target_binding"] == "fixture_value"
    cached = {
        "property_assertions": [
            {"source_contains": "Movement", "target_contains": "cache_node: \"Movement\""}
        ]
    }
    annotate_cached_pose(
        {"properties": [{"name": "NameOfCache", "exported_text": "Movement"}]},
        cached,
        '(cache_node: "Save cached pose \'Movement\'")',
    )
    assert cached["property_assertions"] == [
        {
            "source_contains": "Movement",
            "target_contains": "cache_node: \"Save cached pose 'Movement'\"",
        }
    ]
    empty = {"property_assertions": [{"source_contains": "stale", "target_contains": ""}]}
    remove_empty_target_assertions(empty)
    assert empty["property_assertions"] == []
    curve = {
        "pins": [
            {
                "name": "CurveValues_0",
                "direction": "Input",
                "hidden": False,
                "default_value": "0.0",
            }
        ],
        "properties": [
            {
                "exported_text": "(CurveNames=(\"RotationYawSpeed\"),Alpha=1.000000,ApplyMode=Scale)"
            }
        ],
    }
    curve_map = {
        "input_pins": [{"source": "CurveValues_0", "target": "RotationYawSpeed"}]
    }
    annotate_curve_modify(
        curve,
        curve_map,
        '(curve_name: "RotationYawSpeed", value: 0.0, pin_default: 0.0, value_key: "", source_value_path: "", alpha: 1.0, apply_mode: "Scale")',
    )
    assert curve_map["input_pins"][0]["target_default_contains"] == "pin_default: 0.0"
    default_only_curve_map = {
        "input_pins": [{"source": "CurveValues_0", "target": "RotationYawSpeed"}]
    }
    annotate_curve_modify(
        curve,
        default_only_curve_map,
        '(curve_name: "RotationYawSpeed", value: 0.0, pin_default: 0.0, alpha: 1.0, apply_mode: "Scale")',
    )
    assert default_only_curve_map["input_pins"][0]["target_default_contains"] == "pin_default: 0.0"
    evaluator = {
        "pins": [
            {"name": "ExplicitFrame", "direction": "Input", "hidden": False, "default_value": "0"}
        ],
        "properties": [
            {"exported_text": "(Sequence=\"A_Als_Fixture\",bUseExplicitFrame=True,bShouldLoop=True,bTeleportToExplicitTime=True,ReinitializationBehavior=ExplicitTime)"}
        ],
    }
    evaluator_map = {
        "input_pins": [{"source": "ExplicitFrame", "target": "Explicit Frame"}]
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "runtime.ron").write_text(
            '(clip: "A_Als_Fixture", explicit_frame_pin: "Explicit Frame", should_loop: true, teleport_to_explicit_time: true, reinitialization_behavior: "ExplicitTime")',
            encoding="utf-8",
        )
        annotate_sequence_evaluator(
            root,
            evaluator,
            evaluator_map,
            '(global_input_pins: [(pin: "Explicit Frame", key: "fixture_frame", default_value: 0.0)], graph: "runtime.ron")',
        )
        assert evaluator_map["input_pins"][0]["target_default_contains"] == "default_value: 0.0"
        assert any(
            assertion.get("target_runtime_path") == "runtime.ron"
            for assertion in evaluator_map["property_assertions"]
        )
        evaluator_map["property_assertions"].append(
            {
                "source_contains": "A_Als_Fixture",
                "target_contains": 'clip: "A_Als_Fixture"',
            }
        )
        annotate_sequence_evaluator(
            root,
            evaluator,
            evaluator_map,
            '(global_input_pins: [(pin: "Explicit Frame", key: "fixture_frame", default_value: 0.0)], graph: "runtime.ron")',
        )
        assert not any(
            assertion.get("source_contains") == "A_Als_Fixture"
            and assertion.get("target_runtime_path") is None
            for assertion in evaluator_map["property_assertions"]
        )
    multi_curve = {
        "pins": [
            {"name": "CurveValues_0", "direction": "Input", "hidden": False, "default_value": "1.0"},
            {"name": "CurveValues_1", "direction": "Input", "hidden": False, "default_value": "1.0"},
        ],
        "properties": [
            {"exported_text": "(CurveNames=(\"Left\",\"Right\"),Alpha=1.000000,ApplyMode=Blend)"}
        ],
    }
    multi_curve_map = {
        "input_pins": [
            {"source": "CurveValues_0", "target": "Left"},
            {"source": "CurveValues_1", "target": "Right"},
        ]
    }
    annotate_curve_modify(
        multi_curve,
        multi_curve_map,
        '(curve_names: ["Left", "Right"], pin_defaults: [1.0, 1.0], value_keys: ["", ""], source_value_paths: ["", ""], alpha: 1.0, apply_mode: "Blend")',
    )
    assert multi_curve_map["input_pins"][1]["target_default_contains"] == "pin_defaults: [1.0, 1.0]"
    enum_source = {
        "pins": [
            {"name": "BlendTime_0", "direction": "Input", "hidden": False, "default_value": "0.000000"},
            {"name": "BlendTime_1", "direction": "Input", "hidden": False, "default_value": "0.100000"},
            {"name": "ActiveEnumValue", "direction": "Input", "hidden": False, "default_value": "0"},
        ],
        "properties": [{"exported_text": "(BlendTime=(0.100000,0.100000))"}],
        "binding": {"properties": [{"name": "PropertyBindings", "exported_text": "((\"ActiveEnumValue\", (PathAsText=\"GetParent.HipsDirection\")))"}]},
    }
    enum_map = {
        "input_pins": [
            {"source": "BlendTime_0", "target": "Default Blend Time"},
            {"source": "BlendTime_1", "target": "Selected Blend Time"},
            {"source": "ActiveEnumValue", "target": "Active Enum Value"},
        ]
    }
    annotate_enum_pose_blend(
        enum_source,
        enum_map,
        '(active_key: "fixture_hips_direction", active_enum_default: 0.0, default_blend_time: 0.1, selected_blend_time: 0.1)',
    )
    assert enum_map["input_pins"][2]["target_binding"] == "fixture_hips_direction"
    assert enum_map["input_pins"][2]["type_exception"] == "UE byte enum is represented as target F32 scalar."
    slot_map = {"input_pins": []}
    annotate_slot(
        {"properties": [{"exported_text": '(SlotName="Fixture")'}]},
        slot_map,
        '(slot_name: "Fixture")',
    )
    assert slot_map["property_assertions"][0]["target_contains"] == 'slot_name: "Fixture"'
    call_map = {"input_pins": []}
    annotate_call_function(
        {"properties": [{"exported_text": "(CallSite=OnUpdate,MemberName=Fixture)"}]},
        call_map,
        '(call_site: "OnUpdate", source_function: "Fixture")',
    )
    assert call_map["property_assertions"][0]["target_contains"] == 'call_site: "OnUpdate"'
    assert call_map["property_assertions"][1]["target_contains"] == 'source_function: "Fixture"'
    hips_map = {"input_pins": [{"source": "NewHipsDirection", "target": "NewHipsDirection"}]}
    annotate_call_function(
        {
            "pins": [
                {
                    "name": "NewHipsDirection",
                    "direction": "Input",
                    "hidden": False,
                    "default_value": "Forward",
                }
            ],
            "properties": [{"exported_text": "(CallSite=OnBecomeRelevant)"}],
        },
        hips_map,
        '(call_site: "OnBecomeRelevant", new_hips_direction: "Forward")',
    )
    assert hips_map["input_pins"][0]["target_default_contains"] == 'new_hips_direction: "Forward"'
    assert hips_map["input_pins"][0]["type_exception"] == "UE hips direction enum is represented as target F32 scalar."
    layered_map = {"input_pins": []}
    annotate_layered_bone_blend(
        {
            "properties": [
                {
                    "exported_text": "(LayerSetup=((BranchFilters=((BoneName=\"thigh_l\"),(BoneName=\"ik_foot_l\"))),(BranchFilters=((BoneName=\"thigh_r\"),(BoneName=\"ik_foot_r\")))),BlendWeights=(1.000000,0.500000),SkeletonGuid=00000000000000000000000000000000,VirtualBoneGuid=00000000000000000000000000000000,LODThreshold=-1,bMeshSpaceRotationBlend=True,bBlendRootMotionBasedOnRootBone=True)"
                }
            ]
        },
        layered_map,
        '(branch_bone_layers: [["thigh_l", "ik_foot_l"], ["thigh_r", "ik_foot_r"]], blend_weights: [1.0, 0.5], mesh_space_rotation_blend: true, blend_root_motion_based_on_root_bone: true)',
    )
    assert any(
        assertion.get("source_contains") == "LayerSetup="
        for assertion in layered_map["property_assertions"]
    )
    assert any(
        assertion.get("source_contains") == "BlendWeights=(1.000000,0.500000)"
        for assertion in layered_map["property_assertions"]
    )
    assert any(
        assertion.get("target_exception") == "Target graph evaluates without LOD culling."
        for assertion in layered_map["property_assertions"]
    )
    target_key, key, default = global_input_fields(
        '(global_input_pins: [(pin: "Play Rate", key: "fixture_rate", default_value: 1.0)])',
        "Play Rate",
    )
    assert target_key == 'key: "fixture_rate"'
    assert key == "fixture_rate"
    assert default == "default_value: 1.0"
    try:
        global_input_fields(
            '(global_input_pins: [(pin: "Play Rate", key: "fixture_rate")])',
            "Play Rate",
        )
    except ValueError as error:
        assert str(error) == "target global input default is missing: Play Rate"
    else:
        raise AssertionError("missing global input default was accepted")
    return {
        "sequence_defaults_annotated": "PASS",
        "blend_binding_annotated": "PASS",
        "stale_cache_assertion_replaced": "PASS",
        "empty_target_assertions_removed": "PASS",
        "curve_defaults_annotated": "PASS",
        "unbound_curve_defaults_do_not_require_dynamic_keys": "PASS",
        "evaluator_runtime_properties_annotated": "PASS",
        "runtime_assertions_replace_unscoped_duplicates": "PASS",
        "multi_curve_defaults_annotated": "PASS",
        "enum_blend_defaults_and_binding_annotated": "PASS",
        "slot_and_call_site_properties_annotated": "PASS",
        "call_function_defaults_annotated": "PASS",
        "layered_bone_blend_layers_weights_and_exceptions_annotated": "PASS",
        "global_input_defaults_annotated": "PASS",
        "missing_global_input_default_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.map is None or args.write == args.check:
        raise ValueError("map and exactly one mode are required")
    root = args.root.resolve()
    path = root / args.map
    current = read_json(path)
    try:
        expected = annotate(root, current)
    except ValueError as error:
        print(json.dumps({"issues": [str(error)]}, ensure_ascii=False, indent=2))
        return 1
    if args.write:
        write_json(path, expected)
        print(json.dumps({"map": args.map.as_posix(), "changed": canonical(current) != canonical(expected)}))
        return 0
    if canonical(current) != canonical(expected):
        raise ValueError("source node default annotations differ")
    print(json.dumps({"map": args.map.as_posix(), "changed": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
