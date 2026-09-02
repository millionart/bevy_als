import argparse
import json
import re
import tempfile
from pathlib import Path

from audit_ue_visible_animgraph import (
    FORMAT,
    SOURCE_FORMAT,
    source_root,
    root_behavior_function_names,
    source_title,
    source_visible_nodes,
    target_positions,
    visible_pins,
)
from animgraph_context_contract import parse_nodes


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def pin_label(pin):
    friendly = pin.get("friendly_name")
    return friendly if isinstance(friendly, str) and friendly else pin["name"]


def target_spec_by_id(spec):
    return {node["id"]: node for node in spec["nodes"]}


def target_pin_names(spec_node, direction):
    key = "inputs" if direction == "Input" else "outputs"
    return {pin["name"] for pin in spec_node[key]}


def target_binding(target_node, pin):
    entry = target_node["entry"]
    for target_pin, key in re.findall(r'\(pin:\s*"([^"]+)",\s*key:\s*"([^"]+)"(?:,[^\)]*)?\)', entry):
        if target_pin == pin:
            return key
    alpha = re.search(r'alpha_key:\s*"([^"]+)".*?alpha_pin:\s*"([^"]+)"', entry)
    if alpha is not None and alpha.group(2) == pin:
        return alpha.group(1)
    active = re.search(r'active_key:\s*"([^"]+)"', entry)
    if active is not None and pin == "Active Tag":
        return active.group(1)
    return None


def retained_pin_metadata(existing, source_node, direction, source_pin):
    if not isinstance(existing, dict):
        return {}
    for node in existing.get("node_map", []):
        if node.get("source_node") != source_node:
            continue
        key = "input_pins" if direction == "Input" else "output_pins"
        for pin in node.get(key, []):
            if pin.get("source") != source_pin:
                continue
            return {
                field: pin[field]
                for field in ("type_exception", "display_name_exception")
                if isinstance(pin.get(field), str) and pin[field]
            }
    return {}


def retained_property_assertions(existing, source_node):
    if not isinstance(existing, dict):
        return []
    for node in existing.get("node_map", []):
        if node.get("source_node") != source_node:
            continue
        assertions = node.get("property_assertions")
        if isinstance(assertions, list):
            return assertions
    return []


def retained_root_property_assertions(existing):
    if not isinstance(existing, dict):
        return []
    assertions = existing.get("root_property_assertions")
    return assertions if isinstance(assertions, list) else []


def pin_maps(source_node, target_node, spec_node, direction, existing):
    names = target_pin_names(spec_node, direction)
    values = []
    for pin in visible_pins(source_node, direction):
        label = pin_label(pin)
        if label not in names:
            raise ValueError(
                f"target pin is missing for {source_node['object_path']}: {label}"
            )
        value = {"source": pin["name"], "target": label}
        binding = target_binding(target_node, label)
        if binding is not None:
            value["target_binding"] = binding
        value.update(retained_pin_metadata(existing, source_node["object_path"], direction, pin["name"]))
        values.append(value)
    return values


def build_map(root, source_contract, target_graph, target_spec, comment_policy, comment_exception, existing=None):
    contract = read_json(root / source_contract)
    if contract.get("format") != SOURCE_FORMAT:
        raise ValueError("source contract format is invalid")
    target_text = (root / target_graph).read_text(encoding="utf-8")
    target_by_id, target_nodes = parse_nodes(target_text)
    positions = target_positions(target_text)
    spec = read_json(root / target_spec)
    spec_by_id = target_spec_by_id(spec)
    candidates = {}
    for target in target_nodes:
        candidates.setdefault((target["name"], tuple(positions[target["id"]])), []).append(target)
    node_map = []
    for source in source_visible_nodes(contract):
        key = (
            source_title(source),
            (float(source["position_x"]), float(source["position_y"])),
        )
        matches = candidates.get(key, [])
        if len(matches) != 1:
            raise ValueError(f"source node does not map uniquely: {source['object_path']}")
        target = matches[0]
        spec_node = spec_by_id.get(target["id"])
        if spec_node is None:
            raise ValueError(f"target spec node is missing: {target['id']}")
        entry = {
            "source_node": source["object_path"],
            "target_node": target["id"],
            "input_pins": pin_maps(source, target, spec_node, "Input", existing),
            "output_pins": pin_maps(source, target, spec_node, "Output", existing),
        }
        assertions = retained_property_assertions(existing, source["object_path"])
        if assertions:
            entry["property_assertions"] = assertions
        node_map.append(entry)
    root_node = source_root(contract)
    root_names = {pin["name"] for pin in spec.get("graph_outputs", [])}
    root_output_pins = []
    for pin in visible_pins(root_node, "Input"):
        label = pin_label(pin)
        if label not in root_names:
            raise ValueError(f"target root output pin is missing: {label}")
        root_output_pins.append({"source": pin["name"], "target": label})
    output = {
        "format": FORMAT,
        "source_contract": source_contract,
        "target_graph": target_graph,
        "target_spec": target_spec,
        "node_map": node_map,
        "root_output_pins": root_output_pins,
        "root_property_assertions": retained_root_property_assertions(existing),
        "require_root_property_assertions": bool(
            root_behavior_function_names(root_node)
        ),
        "comment_groups": [],
        "comment_policy": comment_policy,
    }
    if comment_policy == "ignore":
        output["comment_exception"] = comment_exception
    return output


def run_self_test():
    contract = {
        "format": SOURCE_FORMAT,
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Root",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Root",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Output Pose\nAnimGraph",
                    "pins": [{"name": "Result", "direction": "Input", "hidden": False}],
                },
                {
                    "object_path": "/Asset:AnimGraph.Node",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Test",
                    "position_x": -64,
                    "position_y": 0,
                    "title": "Node",
                    "pins": [
                        {"name": "Source", "direction": "Input", "hidden": False},
                        {"name": "Pose", "direction": "Output", "hidden": False},
                    ],
                },
            ]
        },
    }
    graph = (
        "(nodes: [(id: \"node\", name: \"Node\", inner: { \"als_ext::SourcePosePassthroughNode\": () }, should_debug: false)], edges_inverted: {}, io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { \"Result\": Pose }, input_order: {}, output_order: { Data(\"Result\"): 0 }), default_data: {}, editor_metadata: (node_positions: { \"node\": (-64.0, 0.0) }, input_position: (-128.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}))\n"
    )
    spec = {
        "nodes": [
            {
                "id": "node",
                "inputs": [{"name": "Source"}],
                "outputs": [{"name": "Pose"}],
            }
        ],
        "graph_outputs": [{"name": "Result"}],
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "contract.json").write_text(json.dumps(contract), encoding="utf-8")
        (root / "graph.ron").write_text(graph, encoding="utf-8")
        (root / "spec.json").write_text(json.dumps(spec), encoding="utf-8")
        value = build_map(root, "contract.json", "graph.ron", "spec.json", "require", "")
        assert value["node_map"][0]["target_node"] == "node"
        assert value["root_output_pins"] == [{"source": "Result", "target": "Result"}]
        retained = build_map(
            root,
            "contract.json",
            "graph.ron",
            "spec.json",
            "require",
            "",
            {
                "node_map": [
                    {
                        "source_node": "/Asset:AnimGraph.Node",
                        "input_pins": [
                            {
                                "source": "Source",
                                "type_exception": "Retained exception.",
                            }
                        ],
                        "property_assertions": [
                            {
                                "source_contains": "source",
                                "target_contains": "target",
                            }
                        ],
                    }
                ]
            },
        )
        assert retained["node_map"][0]["input_pins"][0]["type_exception"] == "Retained exception."
        assert retained["node_map"][0]["property_assertions"][0]["target_contains"] == "target"
    return {
        "source_position_title_matching": "PASS",
        "visible_pin_mapping": "PASS",
        "exception_retention": "PASS",
        "property_assertion_retention": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source-contract")
    parser.add_argument("--target-graph")
    parser.add_argument("--target-spec")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--comment-policy", choices=["require", "ignore"], default="require")
    parser.add_argument("--comment-exception", default="")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source_contract is None
        or args.target_graph is None
        or args.target_spec is None
        or args.output is None
        or not args.write
    ):
        raise ValueError("source-contract, target-graph, target-spec, output, and --write are required")
    if args.comment_policy == "ignore" and not args.comment_exception:
        raise ValueError("comment-exception is required when comment-policy is ignore")
    root = args.root.resolve()
    output_path = root / args.output
    existing = read_json(output_path) if output_path.is_file() else None
    value = build_map(
        root,
        args.source_contract,
        args.target_graph,
        args.target_spec,
        args.comment_policy,
        args.comment_exception,
        existing,
    )
    write_json(output_path, value)
    print(json.dumps({"output": args.output.as_posix(), "nodes": len(value["node_map"])}, ensure_ascii=False))


if __name__ == "__main__":
    raise SystemExit(main())
