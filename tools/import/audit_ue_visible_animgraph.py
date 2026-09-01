import argparse
import json
import re
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from animgraph_context_contract import parse_edges, parse_nodes
from capture_animgraph_ron_topology import parse_positions


FORMAT = "als_ue_visible_animgraph_map_v2"
SOURCE_FORMAT = "als_ue_visible_graph_contract_v2"
COMMENT_POLICIES = {"require", "ignore"}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def source_nodes(contract):
    return contract["source_graph"]["nodes"]


def is_root(node):
    return node["class_path"].endswith("AnimGraphNode_Root")


def is_comment(node):
    return node["class_path"].endswith("EdGraphNode_Comment")


def source_visible_nodes(contract):
    return [node for node in source_nodes(contract) if not is_root(node) and not is_comment(node)]


def source_root(contract):
    values = [node for node in source_nodes(contract) if is_root(node)]
    if len(values) != 1:
        raise ValueError("source root node is missing or ambiguous")
    return values[0]


def visible_pins(node, direction):
    return [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == direction and not pin.get("hidden", False)
    ]


def source_title(node):
    return re.sub(r"\r\n?", "\n", node["title"])


def source_pin_label(pin):
    friendly = pin.get("friendly_name")
    return friendly if isinstance(friendly, str) and friendly else pin["name"]


def source_pin_type(pin):
    properties = {
        value.get("name"): value.get("exported_text")
        for value in pin.get("pin_type_properties", [])
        if isinstance(value, dict)
    }
    category = properties.get("PinCategory")
    subcategory = properties.get("PinSubCategory")
    object_path = properties.get("PinSubCategoryObject")
    if isinstance(object_path, str) and object_path.endswith("Engine.PoseLink'"):
        return "Pose"
    if category == "real" and subcategory == "float":
        return "F32"
    if category == "bool":
        return "Bool"
    if isinstance(object_path, str) and "GameplayTag" in object_path:
        return "GameplayTag"
    if isinstance(object_path, str) and object_path not in {"", "None"}:
        return object_path
    if isinstance(category, str) and category:
        return category
    return "Unknown"


def source_binding_paths(node):
    binding = node.get("binding")
    if not isinstance(binding, dict):
        return {}
    result = {}
    for prop in binding.get("properties", []):
        if prop.get("name") != "PropertyBindings" or not isinstance(prop.get("exported_text"), str):
            continue
        for pin, path in re.findall(r'\("([^"\\]+)",\s*\(.*?PathAsText="([^"]+)"', prop["exported_text"]):
            if pin in result:
                raise ValueError(f"source property binding is duplicated: {node['object_path']} {pin}")
            result[pin] = path
    return result


def target_output_position(text):
    match = re.search(
        r"(?<![A-Za-z0-9_])output_position\s*:\s*\(\s*(-?[0-9.]+)\s*,\s*(-?[0-9.]+)\s*\)",
        text,
    )
    if match is None:
        raise ValueError("target output position is missing")
    return [float(match.group(1)), float(match.group(2))]


def target_positions(text):
    parsed = parse_positions(text, "node_positions")
    if parsed["malformed"]:
        raise ValueError("target node positions are malformed")
    result = {}
    for entry in parsed["entries"]:
        if entry["id"] in result or entry["position"] is None:
            raise ValueError("target node position is invalid")
        result[entry["id"]] = [float(entry["position"][0]), float(entry["position"][1])]
    return result


def pin_map_shape_issues(value, label):
    issues = []
    if not isinstance(value, list):
        return [f"{label} must be an array"]
    sources = set()
    targets = set()
    allowed = {"source", "target", "type_exception", "display_name_exception", "target_binding"}
    for index, pin in enumerate(value):
        item_label = f"{label}[{index}]"
        if not isinstance(pin, dict) or not set(pin).issubset(allowed):
            issues.append(f"{item_label} is invalid")
            continue
        for key in ("source", "target"):
            if not isinstance(pin.get(key), str) or not pin[key]:
                issues.append(f"{item_label} {key} is invalid")
        source = pin.get("source")
        target = pin.get("target")
        if isinstance(source, str) and source:
            if source in sources:
                issues.append(f"duplicate {label} source pin {source}")
            sources.add(source)
        if isinstance(target, str) and target:
            if target in targets:
                issues.append(f"duplicate {label} target pin {target}")
            targets.add(target)
        for key in ("type_exception", "display_name_exception", "target_binding"):
            if key in pin and (not isinstance(pin[key], str) or not pin[key]):
                issues.append(f"{item_label} {key} is invalid")
    return issues


def map_shape_issues(mapping):
    issues = []
    if mapping.get("format") != FORMAT:
        issues.append("visible graph map format is invalid")
    for key in ("source_contract", "target_graph", "target_spec"):
        if not isinstance(mapping.get(key), str) or not mapping[key]:
            issues.append(f"{key} is invalid")
    nodes = mapping.get("node_map")
    if not isinstance(nodes, list):
        issues.append("node_map must be an array")
    else:
        sources = set()
        targets = set()
        allowed = {"source_node", "target_node", "input_pins", "output_pins"}
        for index, value in enumerate(nodes):
            label = f"node_map[{index}]"
            if not isinstance(value, dict) or set(value) != allowed:
                issues.append(f"{label} is invalid")
                continue
            source = value["source_node"]
            target = value["target_node"]
            if not isinstance(source, str) or not source:
                issues.append(f"{label} source_node is invalid")
            elif source in sources:
                issues.append(f"duplicate source_node {source}")
            else:
                sources.add(source)
            if not isinstance(target, str) or not target:
                issues.append(f"{label} target_node is invalid")
            elif target in targets:
                issues.append(f"duplicate target_node {target}")
            else:
                targets.add(target)
            issues.extend(pin_map_shape_issues(value["input_pins"], f"{label}.input_pins"))
            issues.extend(pin_map_shape_issues(value["output_pins"], f"{label}.output_pins"))
    issues.extend(pin_map_shape_issues(mapping.get("root_output_pins"), "root_output_pins"))
    if not isinstance(mapping.get("comment_groups"), list):
        issues.append("comment_groups must be an array")
    policy = mapping.get("comment_policy", "require")
    if policy not in COMMENT_POLICIES:
        issues.append("comment_policy is invalid")
    if policy == "ignore" and (
        not isinstance(mapping.get("comment_exception"), str) or not mapping["comment_exception"]
    ):
        issues.append("comment_exception is required when comment_policy is ignore")
    return issues


def target_spec_issues(spec, target_text):
    issues = []
    if not isinstance(spec, dict):
        return ["target spec is invalid"]
    if spec.get("graph_source") != target_text:
        issues.append("target spec does not match target graph text")
    nodes = spec.get("nodes")
    if not isinstance(nodes, list):
        issues.append("target spec nodes are invalid")
    else:
        seen = set()
        for index, node in enumerate(nodes):
            if not isinstance(node, dict):
                issues.append(f"target spec node[{index}] is invalid")
                continue
            node_id = node.get("id")
            if not isinstance(node_id, str) or not node_id:
                issues.append(f"target spec node[{index}] id is invalid")
            elif node_id in seen:
                issues.append(f"target spec node id is duplicated: {node_id}")
            else:
                seen.add(node_id)
            for key in ("name", "display_name"):
                if not isinstance(node.get(key), str):
                    issues.append(f"target spec node[{index}] {key} is invalid")
            for direction in ("inputs", "outputs"):
                if not isinstance(node.get(direction), list):
                    issues.append(f"target spec node[{index}] {direction} is invalid")
    if not isinstance(spec.get("graph_outputs"), list):
        issues.append("target spec graph_outputs are invalid")
    return issues


def pin_map_by_source(pin_maps):
    return {value["source"]: value for value in pin_maps}


def spec_pin_by_name(values):
    return {value["name"]: value for value in values}


def pin_report(source_pin, target_pin, mapping):
    return {
        "source": source_pin["name"],
        "source_label": source_pin_label(source_pin),
        "source_type": source_pin_type(source_pin),
        "target": target_pin["name"],
        "target_type": target_pin.get("data_type"),
        "target_binding": mapping.get("target_binding"),
    }


def audit_pins(source, target_node, target_spec_node, pin_maps, direction, differences):
    source_pins = visible_pins(source, direction)
    map_by_source = pin_map_by_source(pin_maps)
    source_by_name = {pin["name"]: pin for pin in source_pins}
    spec_key = "inputs" if direction == "Input" else "outputs"
    target_by_name = spec_pin_by_name(target_spec_node[spec_key])
    missing_source = sorted(set(source_by_name) - set(map_by_source))
    unknown_source = sorted(set(map_by_source) - set(source_by_name))
    mapped_targets = {value["target"] for value in pin_maps}
    missing_target = sorted(set(target_by_name) - mapped_targets)
    unknown_target = sorted(mapped_targets - set(target_by_name))
    prefix = "input" if direction == "Input" else "output"
    if missing_source:
        differences.append({f"missing_source_{prefix}_pins": {"node": source["object_path"], "pins": missing_source}})
    if unknown_source:
        differences.append({f"unknown_source_{prefix}_pins": {"node": source["object_path"], "pins": unknown_source}})
    if missing_target:
        differences.append({f"unmapped_target_{prefix}_pins": {"node": target_node["id"], "pins": missing_target}})
    if unknown_target:
        differences.append({f"unknown_target_{prefix}_pins": {"node": target_node["id"], "pins": unknown_target}})
    bindings = source_binding_paths(source)
    rows = []
    for source_name, value in map_by_source.items():
        source_pin = source_by_name.get(source_name)
        target_pin = target_by_name.get(value["target"])
        if source_pin is None or target_pin is None:
            continue
        row = pin_report(source_pin, target_pin, value)
        if target_pin.get("name") != source_pin_label(source_pin) and "display_name_exception" not in value:
            differences.append({f"{prefix}_pin_display_name": row})
        if target_pin.get("data_type") != source_pin_type(source_pin) and "type_exception" not in value:
            differences.append({f"{prefix}_pin_type": row})
        source_binding = bindings.get(source_name)
        if source_binding is not None:
            target_binding = value.get("target_binding")
            if target_binding is None or target_binding not in target_node["entry"]:
                differences.append({f"{prefix}_pin_binding": {**row, "source_binding": source_binding}})
        rows.append(row)
    return rows


def edge_key(edge):
    return (
        edge["source"]["kind"],
        edge["source"].get("node"),
        edge["source"].get("pin"),
        edge["target"]["kind"],
        edge["target"].get("node"),
        edge["target"].get("pin"),
    )


def edge_description(edge):
    return {"source": edge["source"], "target": edge["target"]}


def endpoint_mapping(endpoint, source_root_path, node_map, output_pins, direction):
    node_path = endpoint["node_object_path"]
    pin_name = endpoint["pin_name"]
    if node_path == source_root_path:
        if direction != "Input":
            raise ValueError("source root direction is invalid")
        mapping = pin_map_by_source(output_pins).get(pin_name)
        if mapping is None:
            raise ValueError(f"source root pin is unmapped: {pin_name}")
        return {"kind": "output_data", "node": None, "pin": mapping["target"]}
    node = node_map.get(node_path)
    if node is None:
        raise ValueError(f"source link node is unmapped: {node_path}")
    pin_maps = node["output_pins"] if direction == "Output" else node["input_pins"]
    mapping = pin_map_by_source(pin_maps).get(pin_name)
    if mapping is None:
        raise ValueError(f"source link pin is unmapped: {node_path} {pin_name}")
    return {"kind": "node_data", "node": node["target_node"], "pin": mapping["target"]}


def expected_edges(contract, node_map, output_pins):
    root_path = source_root(contract)["object_path"]
    return [
        {
            "source": endpoint_mapping(link["source"], root_path, node_map, output_pins, "Output"),
            "target": endpoint_mapping(link["target"], root_path, node_map, output_pins, "Input"),
        }
        for link in contract["links"]
    ]


def target_edges(text):
    values, _ = parse_edges(text)
    return [{"source": edge["source"], "target": edge["target"]} for edge in values]


def audit(root, mapping):
    issues = map_shape_issues(mapping)
    report = {"issues": issues}
    if issues:
        report["pass"] = False
        return report
    source_path = root / mapping["source_contract"]
    target_path = root / mapping["target_graph"]
    target_spec_path = root / mapping["target_spec"]
    missing_paths = [
        value
        for value, path in (
            (mapping["source_contract"], source_path),
            (mapping["target_graph"], target_path),
            (mapping["target_spec"], target_spec_path),
        )
        if not path.is_file()
    ]
    if missing_paths:
        report["issues"] = [f"required path is missing: {value}" for value in missing_paths]
        report["pass"] = False
        return report
    contract = read_json(source_path)
    if contract.get("format") != SOURCE_FORMAT:
        report["issues"] = ["source contract format is invalid"]
        report["pass"] = False
        return report
    target_text = target_path.read_text(encoding="utf-8")
    spec = read_json(target_spec_path)
    spec_issues = target_spec_issues(spec, target_text)
    if spec_issues:
        report["issues"] = spec_issues
        report["pass"] = False
        return report
    target_by_id, target_ordered = parse_nodes(target_text)
    target_position_by_id = target_positions(target_text)
    target_spec_by_id = {node["id"]: node for node in spec["nodes"]}
    source_visible = source_visible_nodes(contract)
    source_comments = [node for node in source_nodes(contract) if is_comment(node)]
    source_by_path = {node["object_path"]: node for node in source_visible}
    node_map = {value["source_node"]: value for value in mapping["node_map"]}
    target_ids = {node["id"] for node in target_ordered}
    mapped_source = set(node_map)
    mapped_target = {value["target_node"] for value in node_map.values()}
    differences = []
    missing_source = sorted(set(source_by_path) - mapped_source)
    unknown_source = sorted(mapped_source - set(source_by_path))
    missing_target = sorted(target_ids - mapped_target)
    unknown_target = sorted(mapped_target - target_ids)
    if missing_source:
        differences.append({"missing_source_nodes": missing_source})
    if unknown_source:
        differences.append({"unknown_source_nodes": unknown_source})
    if missing_target:
        differences.append({"unmapped_target_nodes": missing_target})
    if unknown_target:
        differences.append({"unknown_target_nodes": unknown_target})
    expected_node_order = [
        node_map[node["object_path"]]["target_node"]
        for node in source_visible
        if node["object_path"] in node_map
    ]
    actual_node_order = [node["id"] for node in target_ordered]
    if expected_node_order != actual_node_order:
        differences.append(
            {
                "node_order": {
                    "source_mapped_target_order": expected_node_order,
                    "target_order": actual_node_order,
                }
            }
        )
    mapped_nodes = []
    for source_path_value, node_mapping in node_map.items():
        source = source_by_path.get(source_path_value)
        target_id = node_mapping["target_node"]
        target = target_by_id.get(target_id)
        target_spec_node = target_spec_by_id.get(target_id)
        if source is None or target is None:
            continue
        if target_spec_node is None:
            differences.append({"target_node_spec_missing": target_id})
            continue
        source_position = [float(source["position_x"]), float(source["position_y"])]
        row = {
            "source_node": source_path_value,
            "source_title": source_title(source),
            "target_node": target_id,
            "target_name": target["name"],
            "source_position": source_position,
            "target_position": target_position_by_id.get(target_id),
        }
        if target["name"] != source_title(source):
            differences.append({"node_name": row})
        if target_spec_node["name"] != target["name"]:
            differences.append({"target_spec_node_name": row})
        if target_position_by_id.get(target_id) != source_position:
            differences.append({"node_position": row})
        row["input_pins"] = audit_pins(source, target, target_spec_node, node_mapping["input_pins"], "Input", differences)
        row["output_pins"] = audit_pins(source, target, target_spec_node, node_mapping["output_pins"], "Output", differences)
        mapped_nodes.append(row)
    source_root_node = source_root(contract)
    source_output = [float(source_root_node["position_x"]), float(source_root_node["position_y"])]
    target_output = target_output_position(target_text)
    if source_output != target_output:
        differences.append({"output_position": {"source": source_output, "target": target_output}})
    root_rows = audit_pins(
        source_root_node,
        {"id": "Output", "entry": target_text},
        {"inputs": spec["graph_outputs"], "outputs": []},
        mapping["root_output_pins"],
        "Input",
        differences,
    )
    try:
        expected = expected_edges(contract, node_map, mapping["root_output_pins"])
    except ValueError as error:
        differences.append({"source_link_mapping": str(error)})
        expected = []
    actual = target_edges(target_text)
    expected_by_key = {edge_key(edge): edge for edge in expected}
    actual_by_key = {edge_key(edge): edge for edge in actual}
    missing_edges = [edge_description(expected_by_key[key]) for key in sorted(set(expected_by_key) - set(actual_by_key))]
    extra_edges = [edge_description(actual_by_key[key]) for key in sorted(set(actual_by_key) - set(expected_by_key))]
    if missing_edges:
        differences.append({"missing_source_links": missing_edges})
    if extra_edges:
        differences.append({"extra_target_links": extra_edges})
    if source_comments and mapping.get("comment_policy", "require") == "require":
        differences.append(
            {
                "missing_comment_groups": [
                    {"source_node": node["object_path"], "comment": node["comment"], "position": [node["position_x"], node["position_y"]]}
                    for node in source_comments
                ]
            }
        )
    report.update(
        {
            "source_contract": mapping["source_contract"],
            "target_graph": mapping["target_graph"],
            "target_spec": mapping["target_spec"],
            "source_visible_nodes": len(source_visible),
            "target_nodes": len(target_ordered),
            "source_comments": len(source_comments),
            "target_comments": 0,
            "comment_policy": mapping.get("comment_policy", "require"),
            "ignored_source_comments": source_comments if mapping.get("comment_policy", "require") == "ignore" else [],
            "source_links": len(contract["links"]),
            "target_edges": len(actual),
            "mapped_nodes": mapped_nodes,
            "root_output_pins": root_rows,
            "differences": differences,
            "pass": not differences,
        }
    )
    return report


def run_self_test():
    pose_type = [{"name": "PinSubCategoryObject", "exported_text": "/Script/Engine.PoseLink'"}]
    contract = {
        "format": SOURCE_FORMAT,
        "source_graph": {
            "nodes": [
                {"object_path": "/Asset:AnimGraph.Root", "class_path": "/Script/AnimGraph.AnimGraphNode_Root", "position_x": 0, "position_y": 0, "title": "Output Pose\nAnimGraph", "pins": [{"name": "Result", "direction": "Input", "hidden": False, "pin_type_properties": pose_type}]},
                {"object_path": "/Asset:AnimGraph.Source", "class_path": "/Script/AnimGraph.AnimGraphNode_Test", "position_x": -128, "position_y": 0, "title": "Source", "binding": None, "pins": [{"name": "Pose", "direction": "Output", "hidden": False, "pin_type_properties": pose_type}]},
                {"object_path": "/Asset:AnimGraph.Target", "class_path": "/Script/AnimGraph.AnimGraphNode_Test", "position_x": -64, "position_y": 0, "title": "Target", "binding": None, "pins": [{"name": "Source", "direction": "Input", "hidden": False, "pin_type_properties": pose_type}, {"name": "Pose", "direction": "Output", "hidden": False, "pin_type_properties": pose_type}]},
            ]
        },
        "links": [
            {"source": {"node_object_path": "/Asset:AnimGraph.Source", "pin_name": "Pose"}, "target": {"node_object_path": "/Asset:AnimGraph.Target", "pin_name": "Source"}},
            {"source": {"node_object_path": "/Asset:AnimGraph.Target", "pin_name": "Pose"}, "target": {"node_object_path": "/Asset:AnimGraph.Root", "pin_name": "Result"}},
        ],
    }
    graph = (
        "(\n"
        'nodes: [(id: "source", name: "Source", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }, should_debug: false), (id: "target", name: "Target", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/target.animgraph.ron") }, should_debug: false)],\n'
        'edges_inverted: { NodeData("target", "Source"): NodeData("source", "Pose"), OutputData("Result"): NodeData("target", "Pose") },\n'
        'io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "Result": Pose }, input_order: {}, output_order: { Data("Result"): 0 }),\n'
        "default_data: {},\n"
        'editor_metadata: (node_positions: { "source": (-128.0, 0.0), "target": (-64.0, 0.0) }, input_position: (-256.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n'
        ")\n"
    )
    spec = {
        "graph": "graph.animgraph.ron",
        "graph_source": graph,
        "nodes": [
            {"id": "source", "name": "Source", "display_name": "Source", "inputs": [], "outputs": [{"name": "Pose", "data_type": "Pose"}]},
            {"id": "target", "name": "Target", "display_name": "Target", "inputs": [{"name": "Source", "data_type": "Pose"}], "outputs": [{"name": "Pose", "data_type": "Pose"}]},
        ],
        "graph_outputs": [{"name": "Result", "data_type": "Pose"}],
    }
    mapping = {
        "format": FORMAT,
        "source_contract": "contract.json",
        "target_graph": "graph.animgraph.ron",
        "target_spec": "spec.json",
        "node_map": [
            {"source_node": "/Asset:AnimGraph.Source", "target_node": "source", "input_pins": [], "output_pins": [{"source": "Pose", "target": "Pose"}]},
            {"source_node": "/Asset:AnimGraph.Target", "target_node": "target", "input_pins": [{"source": "Source", "target": "Source"}], "output_pins": [{"source": "Pose", "target": "Pose"}]},
        ],
        "root_output_pins": [{"source": "Result", "target": "Result"}],
        "comment_groups": [],
        "comment_policy": "require",
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "contract.json").write_text(json.dumps(contract), encoding="utf-8")
        (root / "graph.animgraph.ron").write_text(graph, encoding="utf-8")
        (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
        assert audit(root, mapping)["pass"]
        broken = dict(mapping)
        broken["node_map"] = mapping["node_map"][:-1]
        assert not audit(root, broken)["pass"]
        stale = dict(spec)
        stale["graph_source"] = "stale"
        (root / "spec.json").write_text(json.dumps(stale), encoding="utf-8")
        assert not audit(root, mapping)["pass"]
    return {
        "exact_visible_node_pin_link_contract": "PASS",
        "unmapped_source_node_rejected": "PASS",
        "stale_target_spec_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.map is None or not args.check:
        raise ValueError("map and --check are required")
    root = args.root.resolve()
    mapping = read_json(root / args.map)
    report = audit(root, mapping)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
