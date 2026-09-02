import argparse
import json
import re
import tempfile
from pathlib import Path


FORMAT = "als_source_movement_state_graphs_v1"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def read_text(path):
    return path.read_text(encoding="utf-8")


def write_text(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value, encoding="utf-8")


def camel_to_snake(value):
    return re.sub(r"(?<!^)([A-Z])", r"_\1", value).lower()


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


def pin_label(pin):
    friendly = pin.get("friendly_name")
    return friendly if isinstance(friendly, str) and friendly else pin["name"]


def source_pin(node, name, direction):
    for pin in node.get("pins", []):
        if pin.get("name") == name and pin.get("direction") == direction:
            return pin
    raise ValueError(f"source pin is missing: {name}")


def source_property_path(node, pin_name):
    match = re.search(
        rf'PropertyName="{re.escape(pin_name)}".*?PathAsText="([^"]+)"',
        source_text(node),
    )
    if match is None:
        raise ValueError(f"source property path is missing: {pin_name}")
    return match.group(1)


def cache_name(node):
    text = source_text(node)
    match = re.search(r"NameOfCache=([^,\)]+)", text)
    if match is not None:
        return match.group(1)
    title = node.get("title", "")
    match = re.search(r"Use cached pose '([^']+)'", title)
    if match is None:
        raise ValueError("source cache name is missing")
    return f"Save cached pose '{match.group(1)}'"


def curve_config(node, global_prefix):
    text = source_text(node)
    names = re.search(r"CurveNames=\(([^\)]*)\)", text)
    if names is None:
        raise ValueError("source curve names are missing")
    values = re.findall(r'"([^"]+)"', names.group(1))
    if len(values) != 1:
        raise ValueError("movement curve count is invalid")
    curve = values[0]
    default = source_pin(node, "CurveValues_0", "Input").get("default_value")
    if not isinstance(default, str) or not default:
        raise ValueError("movement curve default is missing")
    path = source_property_path(node, "CurveValues_0").removeprefix("GetParent.")
    return curve, default, f"{global_prefix}_curve_{camel_to_snake(curve)}", path


def multi_way_config(node, global_prefix):
    pins = [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input"
        and re.fullmatch(r"DesiredAlphas_\d+", pin.get("name", ""))
    ]
    pins.sort(key=lambda pin: int(pin["name"].rsplit("_", 1)[1]))
    if not pins:
        raise ValueError("movement blend alpha pins are missing")
    keys = []
    defaults = []
    for pin in pins:
        default = pin.get("default_value")
        if not isinstance(default, str) or not default:
            raise ValueError(f"movement blend alpha default is missing: {pin['name']}")
        path = source_property_path(node, pin["name"])
        match = re.search(r"VelocityBlend\.([A-Za-z0-9_]+)", path)
        if match is None:
            raise ValueError(f"movement blend alpha path is invalid: {path}")
        keys.append(f"{global_prefix}_velocity_{camel_to_snake(match.group(1))}")
        defaults.append(float(default))
    return keys, defaults


def hips_config(node):
    inputs = [pin for pin in node.get("pins", []) if pin.get("direction") == "Input"]
    unlinked = [pin for pin in inputs if not pin.get("linked_to")]
    if len(unlinked) != 1:
        raise ValueError("movement hips direction default is invalid")
    value = unlinked[0].get("default_value")
    if not isinstance(value, str) or not value:
        raise ValueError("movement hips direction default is missing")
    return value


def node_identifier(prefix, index):
    return f"{prefix}-0000-4000-8000-{index:012d}"


def existing_prefix(path):
    match = re.search(r'\bid\s*:\s*"([0-9a-fA-F]{8})-', read_text(path))
    if match is None:
        raise ValueError(f"target state id prefix is missing: {path.as_posix()}")
    return match.group(1).lower()


def target_filename(source_graph):
    label = source_graph.rsplit(".", 1)[-1]
    names = {
        "Move Forward": "state_movement_forward.animgraph.ron",
        "Move Backward": "state_movement_backward.animgraph.ron",
        "Move Right Forward": "state_movement_right_forward.animgraph.ron",
        "Move Right Backward": "state_movement_right_backward.animgraph.ron",
        "Move Left Forward": "state_movement_left_forward.animgraph.ron",
        "Move Left Backward": "state_movement_left_backward.animgraph.ron",
    }
    try:
        return names[label]
    except KeyError as error:
        raise ValueError(f"unsupported movement state: {label}") from error


def state_nodes(contract):
    values = []
    for node in contract["source_graph"]["nodes"]:
        class_path = node.get("class_path", "")
        if class_path.endswith("AnimGraphNode_StateResult"):
            continue
        if class_path.endswith("EdGraphNode_Comment"):
            continue
        values.append(node)
    return values


def target_inner(node, global_prefix):
    class_path = node.get("class_path", "")
    if class_path.endswith("AnimGraphNode_UseCachedPose"):
        return f'{{ "als_ext::SourceScopedCachedPoseUseNode": (cache_node: {json.dumps(cache_name(node))}) }}'
    if class_path.endswith("AnimGraphNode_ModifyCurve"):
        curve, default, key, path = curve_config(node, global_prefix)
        return (
            '{ "als_ext::SourceCurveModifyNode": '
            f'(curve_name: {json.dumps(curve)}, value: 0.0, pin_default: {float(default):.1f}, '
            f'value_key: {json.dumps(key)}, source_value_path: {json.dumps(path)}, alpha: 1.0, apply_mode: "Blend") }}'
        )
    if class_path.endswith("AnimGraphNode_MultiWayBlend"):
        keys, defaults = multi_way_config(node, global_prefix)
        return (
            '{ "als_ext::SourceMultiWayBlendNode": '
            f'(pose_count: {len(keys)}, desired_alpha_keys: {json.dumps(keys)}, '
            f'desired_alpha_defaults: {json.dumps(defaults)}, normalize_alpha: true, alpha_scale: 1.0) }}'
        )
    if class_path.endswith("AnimGraphNode_CallFunction"):
        return (
            '{ "als_ext::SourceSetHipsDirectionNode": '
            f'(new_hips_direction: {json.dumps(hips_config(node))}, on_become_relevant: true, call_site: "OnBecomeRelevant") }}'
        )
    raise ValueError(f"unsupported movement state node: {class_path}")


def target_pin(contract, endpoint):
    path = endpoint["node_object_path"]
    for node in contract["source_graph"]["nodes"]:
        if node.get("object_path") != path:
            continue
        return pin_label(source_pin(node, endpoint["pin_name"], endpoint["direction"]))
    return endpoint["pin_name"]


def build(contract, prefix, global_prefix):
    root = next(
        node
        for node in contract["source_graph"]["nodes"]
        if node.get("class_path", "").endswith("AnimGraphNode_StateResult")
    )
    nodes = state_nodes(contract)
    identifiers = {
        node["object_path"]: node_identifier(prefix, index + 1)
        for index, node in enumerate(nodes)
    }
    rows = []
    for node in nodes:
        identifier = identifiers[node["object_path"]]
        rows.append(
            f'        (id: {json.dumps(identifier)}, name: {json.dumps(node["title"].replace(chr(13), ""))}, inner: {target_inner(node, global_prefix)}, should_debug: false),'
        )
    edges = []
    for link in contract.get("links", []):
        source = link["source"]
        target = link["target"]
        if source["node_object_path"] not in identifiers:
            continue
        source_id = identifiers[source["node_object_path"]]
        source_pin = target_pin(contract, source)
        source_value = f'NodeData({json.dumps(source_id)}, {json.dumps(source_pin)})'
        if target["node_object_path"] == root["object_path"]:
            if target["pin_name"] != "Result":
                raise ValueError("movement state root pin is invalid")
            target_value = 'OutputData("Result")'
        else:
            target_id = identifiers.get(target["node_object_path"])
            if target_id is None:
                continue
            target_pin_name = target_pin(contract, target)
            target_value = f'NodeData({json.dumps(target_id)}, {json.dumps(target_pin_name)})'
        edges.append((target_value, source_value))
    if len(edges) != len(contract.get("links", [])):
        raise ValueError("movement state links do not fully map")
    edges.sort(key=lambda value: value[0])
    edge_rows = [f"        {target}: {source}," for target, source in edges]
    position_rows = [
        f'            {json.dumps(identifiers[node["object_path"]])}: ({float(node["position_x"]):.1f}, {float(node["position_y"]):.1f}),'
        for node in nodes
    ]
    return "\n".join(
        [
            "(",
            "    nodes: [",
            *rows,
            "    ],",
            "    edges_inverted: {",
            *edge_rows,
            "    },",
            "    io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { \"Result\": Pose }, input_order: {}, output_order: { Data(\"Result\"): 0 }),",
            "    default_data: {},",
            "    editor_metadata: (",
            "        node_positions: {",
            *position_rows,
            "        },",
            "        input_position: (0.0, 0.0),",
            "        output_position: (0.0, 0.0),",
            "        input_param_order: {},",
            "        input_time_order: {},",
            "        output_data_order: {},",
            "        output_pose_order: {},",
            "    ),",
            ")",
            "",
        ]
    )


def plans(root, source_index, output_dir, global_prefix):
    index = read_json(source_index)
    if index.get("format") != "als_ue_visible_graph_contract_batch_v1":
        raise ValueError("source contract index is invalid")
    result = []
    for entry in index.get("entries", []):
        source_graph = entry.get("source_graph", "")
        if ".Movement States.AnimStateNode_" not in source_graph:
            continue
        contract_path = source_index.parent / entry["contract"]
        contract = read_json(contract_path)
        output = output_dir / target_filename(source_graph)
        result.append(
            {
                "source_graph": source_graph,
                "output": output,
                "content": build(contract, existing_prefix(output), global_prefix),
            }
        )
    if len(result) != 6:
        raise ValueError("movement state closure must contain six graphs")
    return result


def run_self_test():
    root_node = {
        "object_path": "root",
        "class_path": "AnimGraphNode_StateResult",
        "title": "Output Animation Pose",
        "position_x": 0,
        "position_y": 0,
        "pins": [{"name": "Result", "direction": "Input", "friendly_name": "", "linked_to": []}],
    }
    cache_node = {
        "object_path": "cache",
        "class_path": "AnimGraphNode_UseCachedPose",
        "title": "Use cached pose 'Move Forward'",
        "position_x": -64,
        "position_y": 0,
        "properties": [{"exported_text": "NameOfCache=Save cached pose 'Move Forward'"}],
        "pins": [{"name": "Pose", "direction": "Output", "friendly_name": "", "linked_to": []}],
    }
    contract = {
        "source_graph": {"nodes": [root_node, cache_node]},
        "links": [
            {
                "source": {"node_object_path": "cache", "pin_name": "Pose", "direction": "Output"},
                "target": {"node_object_path": "root", "pin_name": "Result", "direction": "Input"},
            }
        ],
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        target = root / "state_movement_forward.animgraph.ron"
        write_text(target, '(nodes: [(id: "15651000-0000-4000-8000-000000000001", name: "old", inner: { "x": () }, should_debug: false)], edges_inverted: {}, io_spec: (), default_data: {}, editor_metadata: ())\n')
        value = build(contract, existing_prefix(target), "als_fixture")
        assert 'cache_node: "Save cached pose \'Move Forward\'"' in value
        assert 'OutputData("Result")' in value
        assert '"15651000-0000-4000-8000-000000000001"' in value
    return {
        "source_cache_state_graph_generated": "PASS",
        "source_state_root_output_generated": "PASS",
        "existing_target_id_prefix_preserved": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source-index", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--global-prefix")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source_index is None
        or args.output_dir is None
        or not args.global_prefix
        or args.write == args.check
    ):
        raise ValueError("source-index, output-dir, global-prefix, and exactly one mode are required")
    root = args.root.resolve()
    source_index = args.source_index if args.source_index.is_absolute() else root / args.source_index
    output_dir = args.output_dir if args.output_dir.is_absolute() else root / args.output_dir
    values = plans(root, source_index, output_dir, args.global_prefix)
    differences = []
    for value in values:
        if args.write:
            write_text(value["output"], value["content"])
        elif not value["output"].is_file() or read_text(value["output"]) != value["content"]:
            differences.append(value["output"].relative_to(root).as_posix())
    print(json.dumps({"graphs": len(values), "differences": differences}, ensure_ascii=False, indent=2))
    return 0 if not differences else 1


if __name__ == "__main__":
    raise SystemExit(main())
