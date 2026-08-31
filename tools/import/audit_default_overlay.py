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
DEFAULT_SOURCE = ROOT / "target" / "bf109" / "source_native" / "AB_Als_Default.ue.json"
DEFAULT_CONTRACT = ROOT / "assets" / "graphs" / "ab_als" / "overlays" / "default.source.json"
DEFAULT_TARGET = ROOT / "assets" / "graphs" / "ab_als" / "overlays" / "default.animgraph.ron"
DEFAULT_MAPPING = ROOT / "tools" / "import" / "default_source_layout_map.json"
DEFAULT_REPORT = ROOT / "target" / "bf109" / "audits" / "AB_Als_Default.audit.json"


CURVE_NAMES = [
    "LayerArmLeft",
    "LayerArmLeftAdditive",
    "LayerArmLeftLocalSpace",
    "LayerArmRight",
    "LayerArmRightAdditive",
    "LayerArmRightLocalSpace",
    "LayerHead",
    "LayerHeadAdditive",
    "LayerLegs",
    "LayerPelvis",
    "LayerSpine",
    "LayerSpineAdditive",
    "HandLeftIk",
    "HandRightIk",
    "HipsDirectionLock",
    "LayerHandLeft",
    "LayerHandRight",
    "PoseAiming",
]


FRAME_VALUES = {
    "frame zero position": 0.0,
    "frame one position": 0.033333335,
    "frame two position": 0.06666667,
    "frame one air position": 0.033333335,
    "frame zero air position": 0.0,
}


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


def node_constant_value(text, name):
    pattern = re.compile(
        rf'name:\s*"{re.escape(name)}".*?ConstantF32Node"\s*:\s*\(value:\s*([-+0-9.eE]+)',
        re.DOTALL,
    )
    match = pattern.search(text)
    return None if match is None else float(match.group(1))


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
    overlay = find_graph(source, "Overlay")
    source_graphs = {graph["object_path"]: graph for graph in source["graphs"]}
    source_by_path = all_source_nodes(source_graphs)
    overlay_entry = next(
        (
            entry
            for entry in mapping["graphs"]
            if entry["source_suffix"] == "Overlay" and not entry.get("graph_scope_only")
        ),
        None,
    )
    if overlay_entry is None:
        differences.append("overlay layout entry")
        return differences, {}
    nodes = target_nodes(target_text)
    node_ids = {node["id"] for node in nodes}
    overrides = overlay_entry.get("node_overrides", [])
    override_ids = {entry["target_id"] for entry in overrides}
    if node_ids != override_ids:
        differences.append("layout target node coverage")
    source_index_set = {node["index"] for node in source_nodes(overlay)}
    mapped_indices = {index for entry in overrides for index in source_indices(entry)}
    if not source_index_set.issubset(mapped_indices):
        differences.append("layout source node coverage")
    graph_scope_suffixes = {
        entry["source_suffix"]
        for entry in mapping["graphs"]
        if entry.get("graph_scope_only")
    }
    if not {"AnimGraph", "EventGraph"}.issubset(graph_scope_suffixes):
        differences.append("graph scope coverage")
    positions, node_rows = place_nodes(
        nodes,
        target_edges(target_text),
        source_nodes(overlay),
        source_by_path,
        source_owner(overlay, source_by_path),
        overrides,
    )
    actual_positions = {
        entry["id"]: entry["position"]
        for entry in parse_positions(target_text, "node_positions")["entries"]
    }
    if actual_positions != positions:
        differences.append("target node positions")
    for name, expected in overlay_entry.get("editor_positions", {}).items():
        if editor_position(target_text, name) != expected:
            differences.append(f"editor position {name}")
    required_fragments = [
        'name: "ground prediction interpolation"',
        "increasing_speed: 20.0",
        "decreasing_speed: 5.0",
        'name: "secondary motion idle loop"',
        'key: "als_secondary_motion_phase"',
        "leader_duration: 2.666667",
        "follower_duration: 2.666667",
        "looped: true",
        'name: "secondary motion additive alpha"',
        "value: 0.75",
        'clip: "animations/Base/A_Als_Idle.anim.ron"',
    ]
    for fragment in required_fragments:
        if fragment not in target_text:
            differences.append(f"target fragment {fragment}")
    if target_text.count('clip: "animations/Overlays/Other/A_Als_Default_Poses.anim.ron"') != 5:
        differences.append("default poses clip count")
    for name, expected in FRAME_VALUES.items():
        actual = node_constant_value(target_text, name)
        if actual is None or abs(actual - expected) > 1e-6:
            differences.append(f"frame value {name}")
    for name in CURVE_NAMES:
        if f'name: "{name} curve"' not in target_text:
            differences.append(f"curve node {name}")
    report = {
        "source_asset": source["asset_path"],
        "source_sha256": canonical_hash(source),
        "source_graphs": len(source["graphs"]),
        "source_nodes": sum(len(graph["nodes"]) for graph in source["graphs"]),
        "source_pins": sum(len(node["pins"]) for graph in source["graphs"] for node in graph["nodes"]),
        "source_asset_objects": len(source["asset_objects"]),
        "overlay_source_nodes": len(overlay["nodes"]),
        "overlay_source_pins": sum(len(node["pins"]) for node in overlay["nodes"]),
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
    if node_constant_value('name: "value" inner: { "ConstantF32Node": (value: 0.75) }', "value") != 0.75:
        raise AssertionError("node constant")
    if editor_position("input_position: (-1.0, 2.0)", "input_position") != [-1.0, 2.0]:
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
        print("Default overlay audit self-test PASS")
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
        "Default overlay audit "
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
