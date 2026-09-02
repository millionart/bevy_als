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


def add_assertion(node_map, source_contains, target_contains):
    assertions = node_map.setdefault("property_assertions", [])
    value = {"source_contains": source_contains, "target_contains": target_contains}
    if value not in assertions:
        assertions.append(value)


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
        mapping["target_default_contains"] = scalar_literal(target_entry, target_field)
        source_binding = bindings.get(source_name)
        if source_binding is not None and target_key_field is not None:
            target_key, key = string_literal(target_entry, target_key_field)
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
    target_group, group = string_literal(target_entry, "sync_group")
    if group:
        add_assertion(node_map, f'GroupName="{group}"', target_group)
    target_clip, clip = string_literal(target_entry, "clip")
    asset = re.search(r"(A_Als_[A-Za-z0-9_]+)", clip)
    if asset is not None:
        add_assertion(node_map, asset.group(1), target_clip)
    if "Method=SyncGroup" in source:
        add_assertion(node_map, "Method=SyncGroup", target_group)
    reference = re.search(
        r'\badditive_reference\s*:\s*Some\("([^"]+)"\)', target_entry
    )
    if reference is not None and asset is not None:
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
    try:
        target_keys, target_key_values = array_literal(target_entry, "value_keys")
    except ValueError:
        target_keys, key = string_literal(target_entry, "value_key")
        target_key_values = json.dumps(key)
    try:
        target_source_paths, _ = array_literal(target_entry, "source_value_paths")
    except ValueError:
        target_source_paths, _ = string_literal(target_entry, "source_value_path")
    source_curve_names = (
        re.findall(r'"([^\"]+)"', curve_names.group(1))
        if curve_names is not None
        else []
    )
    target_curve_names = re.findall(r'"([^\"]+)"', target_curve_values)
    target_value_keys = re.findall(r'"([^\"]*)"', target_key_values)
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


def annotate_sequence_evaluator(source, node_map, target_entry):
    source_default = source_pin(source, "ExplicitFrame").get("default_value")
    if not isinstance(source_default, str) or not source_default:
        raise ValueError("source explicit frame default is missing")
    mapping = mapped_pin(node_map, "ExplicitFrame")
    mapping["source_default"] = source_default
    mapping["target_default_contains"] = scalar_literal(target_entry, "explicit_frame")
    mapping["type_exception"] = "UE frame count is represented as target F32."
    source_values = source_text(source)
    target_clip, clip = string_literal(target_entry, "clip")
    asset = re.search(r"(A_Als_[A-Za-z0-9_]+)", clip)
    if asset is not None:
        add_assertion(node_map, asset.group(1), target_clip)
    for source_contains, target_field in (
        ("bUseExplicitFrame=True", "explicit_frame_pin"),
        ("bShouldLoop=True", "should_loop"),
        ("bTeleportToExplicitTime=True", "teleport_to_explicit_time"),
        ("ReinitializationBehavior=ExplicitTime", "reinitialization_behavior"),
    ):
        if source_contains in source_values:
            if target_field in {"should_loop", "teleport_to_explicit_time"}:
                target = bool_literal(target_entry, target_field)
            else:
                target, _ = string_literal(target_entry, target_field)
            add_assertion(node_map, source_contains, target)


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
            annotate_sequence_evaluator(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_Slot"):
            annotate_slot(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_CallFunction"):
            annotate_call_function(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_UseCachedPose"):
            annotate_cached_pose(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_MultiWayBlend"):
            annotate_multi_way_blend(source, node_map, target["entry"])
        elif class_path.endswith("AnimGraphNode_BlendListByEnum"):
            annotate_enum_pose_blend(source, node_map, target["entry"])
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
        {"properties": [{"exported_text": "(CallSite=OnUpdate)"}]},
        call_map,
        '(call_site: "OnUpdate")',
    )
    assert call_map["property_assertions"][0]["target_contains"] == 'call_site: "OnUpdate"'
    return {
        "sequence_defaults_annotated": "PASS",
        "blend_binding_annotated": "PASS",
        "stale_cache_assertion_replaced": "PASS",
        "curve_defaults_annotated": "PASS",
        "multi_curve_defaults_annotated": "PASS",
        "enum_blend_defaults_and_binding_annotated": "PASS",
        "slot_and_call_site_properties_annotated": "PASS",
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
    expected = annotate(root, current)
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
