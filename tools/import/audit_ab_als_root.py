import argparse
import hashlib
import json
import re
from pathlib import Path

from apply_source_layout import (
    all_source_nodes,
    place_nodes,
    source_nodes,
    source_owner,
    target_edges,
    target_nodes,
)
from capture_animgraph_ron_topology import parse_positions


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT / "target" / "bf109" / "source_native" / "AB_Als.ue.json"
DEFAULT_CONTRACT = ROOT / "docs" / "bf109" / "source_contracts" / "AB_Als.source.json"
DEFAULT_TARGET = ROOT / "assets" / "graphs" / "ab_als" / "ab_als.animgraph.ron"
DEFAULT_MAPPING = ROOT / "tools" / "import" / "ab_als_source_layout_map.json"
DEFAULT_REPORT = ROOT / "target" / "bf109" / "audits" / "AB_Als.audit.json"


def canonical_hash(value):
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def find_graph(source, name):
    for graph in source["graphs"]:
        if graph["name"] == name:
            return graph
    raise ValueError(f"missing graph: {name}")


def source_indices(entry):
    values = entry.get("source_indices")
    if values is None:
        values = [entry.get("source_index")]
    return [value for value in values if value is not None]


def editor_position(text, name):
    pattern = re.compile(
        rf'\b{re.escape(name)}:\s*\(\s*([-+0-9.eE]+)\s*,\s*([-+0-9.eE]+)\s*\)'
    )
    match = pattern.search(text)
    return None if match is None else [float(match.group(1)), float(match.group(2))]


def audit(source, contract, target_text, mapping):
    differences = []
    if contract.get("source_asset") != source.get("asset_path"):
        differences.append("contract source asset")
    if contract.get("source_class") != source.get("asset_class"):
        differences.append("contract source class")
    if contract.get("source_payload") != source:
        differences.append("contract source payload")
    if contract.get("source_sha256") != canonical_hash(source):
        differences.append("contract source hash")
    anim_graph = find_graph(source, "AnimGraph")
    source_graphs = {graph["object_path"]: graph for graph in source["graphs"]}
    source_by_path = all_source_nodes(source_graphs)
    root_entry = next(
        (
            entry
            for entry in mapping["graphs"]
            if entry["source_suffix"] == "AnimGraph" and not entry.get("graph_scope_only")
        ),
        None,
    )
    if root_entry is None:
        differences.append("root layout entry")
        return differences, {}
    nodes = target_nodes(target_text)
    node_ids = {node["id"] for node in nodes}
    overrides = root_entry.get("node_overrides", [])
    override_ids = {entry["target_id"] for entry in overrides}
    if node_ids != override_ids:
        differences.append("layout target node coverage")
    source_index_set = {node["index"] for node in source_nodes(anim_graph)}
    mapped_indices = {index for entry in overrides for index in source_indices(entry)}
    if not source_index_set.issubset(mapped_indices):
        differences.append("layout source node coverage")
    graph_scope_suffixes = {
        entry["source_suffix"]
        for entry in mapping["graphs"]
        if entry.get("graph_scope_only")
    }
    if not {"Overlay", "EventGraph"}.issubset(graph_scope_suffixes):
        differences.append("graph scope coverage")
    positions, node_rows = place_nodes(
        nodes,
        target_edges(target_text),
        source_nodes(anim_graph),
        source_by_path,
        source_owner(anim_graph, source_by_path),
        overrides,
    )
    actual_positions = {
        entry["id"]: entry["position"]
        for entry in parse_positions(target_text, "node_positions")["entries"]
    }
    if actual_positions != positions:
        differences.append("target node positions")
    for name, expected in root_entry.get("editor_positions", {}).items():
        if editor_position(target_text, name) != expected:
            differences.append(f"editor position {name}")
    required_fragments = [
        'name: "AB_Als_Grounded Slot Transition"',
        'name: "AB_Als_Locomotion Slot PostLocomotion"',
        'slot_name: Some("Transition")',
        'slot_name: Some("PostLocomotion")',
        "always_update_source_pose: false",
        'name: "ALI_Overlay Overlay Inertialization"',
        'selector: "layer_index"',
        "reset_on_becoming_relevant: true",
        "forward_requests_through_skipped_cached_poses: true",
        'name: "ControlRig CR_Als"',
        'rig_class: "CR_Als"',
        "alpha_enabled: true",
        "transfer_input_pose: true",
        "transfer_input_curves: true",
        "transfer_pose_in_global_space: false",
        'graph: "graphs/ab_als/layering.animgraph.ron"',
        'graph: "graphs/ab_als/head.animgraph.ron"',
        'graph: "graphs/ab_als/ragdolling.animgraph.ron"',
        'implementations: ["graphs/ab_als/overlays/default.animgraph.ron"',
        'OutputData("ragdoll_config")',
    ]
    for fragment in required_fragments:
        if fragment not in target_text:
            differences.append(f"target fragment {fragment}")
    report = {
        "source_asset": source["asset_path"],
        "source_sha256": canonical_hash(source),
        "source_graphs": len(source["graphs"]),
        "source_nodes": sum(len(graph["nodes"]) for graph in source["graphs"]),
        "source_pins": sum(len(node["pins"]) for graph in source["graphs"] for node in graph["nodes"]),
        "source_asset_objects": len(source["asset_objects"]),
        "anim_graph_source_nodes": len(anim_graph["nodes"]),
        "anim_graph_source_pins": sum(len(node["pins"]) for node in anim_graph["nodes"]),
        "target_nodes": len(nodes),
        "layout_nodes": node_rows,
        "differences": differences,
        "pass": not differences,
    }
    return differences, report


def self_test():
    payload = {"value": [1, 2, 3]}
    if canonical_hash(payload) != canonical_hash({"value": [1, 2, 3]}):
        raise AssertionError("canonical hash")
    if editor_position("output_position: (0.0, 0.0)", "output_position") != [0.0, 0.0]:
        raise AssertionError("editor position")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--target", type=Path, default=DEFAULT_TARGET)
    parser.add_argument("--mapping", type=Path, default=DEFAULT_MAPPING)
    parser.add_argument("--report", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("AB_Als root audit self-test PASS")
        return
    source = json.loads(args.source.read_text(encoding="utf-8"))
    contract = json.loads(args.contract.read_text(encoding="utf-8"))
    target_text = args.target.read_text(encoding="utf-8")
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    differences, report = audit(source, contract, target_text, mapping)
    if not args.check:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        "AB_Als root audit "
        f"graphs={report.get('source_graphs', 0)} "
        f"nodes={report.get('source_nodes', 0)} "
        f"pins={report.get('source_pins', 0)} "
        f"target_nodes={report.get('target_nodes', 0)} "
        f"differences={len(differences)} "
        f"-> {'PASS' if not differences else 'FAIL'}"
    )
    if differences:
        for difference in differences:
            print(f"DIFF {difference}")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
