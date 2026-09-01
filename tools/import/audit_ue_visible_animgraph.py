import argparse
import json
import math
import re
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

import sys

sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from animgraph_context_contract import parse_edges, parse_nodes
from capture_animgraph_ron_topology import parse_positions


FORMAT = "als_ue_visible_animgraph_map_v1"
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
    return [
        node
        for node in source_nodes(contract)
        if not is_root(node) and not is_comment(node)
    ]


def source_root(contract):
    values = [node for node in source_nodes(contract) if is_root(node)]
    if len(values) != 1:
        raise ValueError("source root node is missing or ambiguous")
    return values[0]


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


def map_shape_issues(mapping):
    issues = []
    if mapping.get("format") != FORMAT:
        issues.append("visible graph map format is invalid")
    for key in ("source_contract", "target_graph"):
        if not isinstance(mapping.get(key), str) or not mapping[key]:
            issues.append(f"{key} is invalid")
    nodes = mapping.get("node_map")
    if not isinstance(nodes, list):
        issues.append("node_map must be an array")
    else:
        sources = set()
        targets = set()
        for index, value in enumerate(nodes):
            label = f"node_map[{index}]"
            if not isinstance(value, dict) or set(value) != {"source_node", "target_node"}:
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
    comments = mapping.get("comment_groups")
    if not isinstance(comments, list):
        issues.append("comment_groups must be an array")
    policy = mapping.get("comment_policy", "require")
    if policy not in COMMENT_POLICIES:
        issues.append("comment_policy is invalid")
    if policy == "ignore" and (
        not isinstance(mapping.get("comment_exception"), str)
        or not mapping["comment_exception"]
    ):
        issues.append("comment_exception is required when comment_policy is ignore")
    return issues


def audit(root, mapping):
    issues = map_shape_issues(mapping)
    report = {"issues": issues}
    if issues:
        report["pass"] = False
        return report
    source_path = root / mapping["source_contract"]
    target_path = root / mapping["target_graph"]
    if not source_path.is_file() or not target_path.is_file():
        missing = []
        if not source_path.is_file():
            missing.append(mapping["source_contract"])
        if not target_path.is_file():
            missing.append(mapping["target_graph"])
        report["issues"] = [f"required path is missing: {value}" for value in missing]
        report["pass"] = False
        return report
    contract = read_json(source_path)
    if contract.get("format") != "als_ue_visible_graph_contract_v1":
        report["issues"] = ["source contract format is invalid"]
        report["pass"] = False
        return report
    target_text = target_path.read_text(encoding="utf-8")
    target_by_id, target_ordered = parse_nodes(target_text)
    target_position_by_id = target_positions(target_text)
    source_visible = source_visible_nodes(contract)
    source_comments = [node for node in source_nodes(contract) if is_comment(node)]
    source_by_path = {
        node["object_path"]: node
        for node in source_visible
    }
    node_map = {
        value["source_node"]: value["target_node"]
        for value in mapping["node_map"]
    }
    target_ids = {node["id"] for node in target_ordered}
    mapped_source = set(node_map)
    mapped_target = set(node_map.values())
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
    mapped_nodes = []
    for source_path_value, target_id in node_map.items():
        source = source_by_path.get(source_path_value)
        target = target_by_id.get(target_id)
        if source is None or target is None:
            continue
        source_title = source["title"].replace("\n", " / ")
        target_position = target_position_by_id.get(target_id)
        source_position = [float(source["position_x"]), float(source["position_y"])]
        row = {
            "source_node": source_path_value,
            "source_title": source_title,
            "target_node": target_id,
            "target_name": target["name"],
            "source_position": source_position,
            "target_position": target_position,
        }
        if target["name"] != source_title:
            differences.append({"node_name": row})
        if target_position != source_position:
            differences.append({"node_position": row})
        mapped_nodes.append(row)
    source_output = [float(source_root(contract)["position_x"]), float(source_root(contract)["position_y"])]
    target_output = target_output_position(target_text)
    if source_output != target_output:
        differences.append(
            {
                "output_position": {
                    "source": source_output,
                    "target": target_output,
                }
            }
        )
    source_links = len(contract["links"])
    target_edges, _ = parse_edges(target_text)
    if source_links != len(target_edges):
        differences.append(
            {
                "edge_count": {
                    "source": source_links,
                    "target": len(target_edges),
                }
            }
        )
    if source_comments and mapping.get("comment_policy", "require") == "require":
        differences.append(
            {
                "missing_comment_groups": [
                    {
                        "source_node": node["object_path"],
                        "comment": node["comment"],
                        "position": [node["position_x"], node["position_y"]],
                    }
                    for node in source_comments
                ]
            }
        )
    report.update(
        {
            "source_contract": mapping["source_contract"],
            "target_graph": mapping["target_graph"],
            "source_visible_nodes": len(source_visible),
            "target_nodes": len(target_ordered),
            "source_comments": len(source_comments),
            "target_comments": 0,
            "comment_policy": mapping.get("comment_policy", "require"),
            "ignored_source_comments": source_comments
            if mapping.get("comment_policy", "require") == "ignore"
            else [],
            "source_links": source_links,
            "target_edges": len(target_edges),
            "mapped_nodes": mapped_nodes,
            "differences": differences,
            "pass": not differences,
        }
    )
    return report


def run_self_test():
    contract = {
        "format": "als_ue_visible_graph_contract_v1",
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Root",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Root",
                    "position_x": 0,
                    "position_y": 0,
                    "title": "Output Pose\nAnimGraph",
                },
                {
                    "object_path": "/Asset:AnimGraph.Node",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Test",
                    "position_x": -64,
                    "position_y": 0,
                    "title": "Source",
                },
            ]
        },
        "links": [{"source": {}, "target": {}}],
    }
    graph = (
        "(\n"
        'nodes: [(id: "node", name: "Source", inner: { "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }, should_debug: false)],\n'
        'edges_inverted: { OutputData("pose"): NodeData("node", "pose") },\n'
        'io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "pose": Pose }, input_order: {}, output_order: { Data("pose"): 0 }),\n'
        "default_data: {},\n"
        'editor_metadata: (node_positions: { "node": (-64.0, 0.0) }, input_position: (-128.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n'
        ")\n"
    )
    mapping = {
        "format": FORMAT,
        "source_contract": "contract.json",
        "target_graph": "graph.animgraph.ron",
        "node_map": [
            {
                "source_node": "/Asset:AnimGraph.Node",
                "target_node": "node",
            }
        ],
        "comment_groups": [],
        "comment_policy": "require",
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "contract.json").write_text(
            json.dumps(contract),
            encoding="utf-8",
        )
        (root / "graph.animgraph.ron").write_text(graph, encoding="utf-8")
        valid = audit(root, mapping)
        assert valid["pass"]
        broken = dict(mapping)
        broken["node_map"] = []
        assert not audit(root, broken)["pass"]
    return {
        "exact_visible_node_contract": "PASS",
        "unmapped_source_node_rejected": "PASS",
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
