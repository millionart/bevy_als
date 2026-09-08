import argparse
import collections
import json
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
for directory in ("tools", "tools/import", "tools/animgraph_authoring"):
    sys.path.insert(0, str(root / directory))
from animgraph_context_contract import parse_nodes, parse_edges
from extract_ue_visible_graph_contract import contract
from ue_animgraph_reachability import traverse
from ue_animgraph_converter import source_identifier_key
from audit_ue_visible_animgraph import source_pin_type

def reachable_pair(edges, start, end, originals):
    pending = [start]
    seen = set()
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        for target in edges.get(current, []):
            if target == end:
                return True
            if target not in originals:
                pending.append(target)
    return False

def audit(source_directory, output, spec_path, assets_root):
    specs = json.loads(spec_path.read_text(encoding="utf-8"))
    sources = {}
    for path in source_directory.glob("*.ue.json"):
        source = json.loads(path.read_text(encoding="utf-8-sig"))
        if source["asset_class"] == "/Script/Engine.AnimBlueprint":
            sources[source["asset_path"]] = source
    failures = []
    rows = []
    totals = collections.Counter()

    conversion = json.loads((output / "conversion.report.json").read_text(encoding="utf-8"))
    integration = json.loads((output / "integration.report.json").read_text(encoding="utf-8"))
    if not conversion.get("pass") or conversion.get("unsupported") or integration.get("conversion_status") != "ready":
        raise ValueError("generated source closure is not ready")
    expected_sources = {entry["source_asset"] for entry in integration["source_closure"]}
    if not expected_sources or expected_sources - set(sources):
        raise ValueError("native source closure is incomplete")
    sources = {asset: sources[asset] for asset in expected_sources}
    root_asset = conversion["source"]["package_path"]
    for asset, source in sources.items():
        manifest_path = output / ("materialization.manifest.json" if asset == root_asset else "linked_manifests/" + source_identifier_key(asset) + ".json")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        root_suffix = next(name for name, path in manifest["graphs"].items() if path == manifest["root_graph"])
        closure = traverse(source, root_suffix)
        totals["source_subgraphs"] += len(closure["graphs"])
        reachable = {row["source_graph"]: set(row["reachable_nodes"]) for row in closure["graphs"]}
        for suffix, target_path in manifest["graphs"].items():
            source_graph = contract(source, suffix)
            active = reachable[suffix]
            raw = source_graph["source_graph"]["nodes"]
            selected = [node for node in raw if node["object_path"] in active and not node["class_path"].endswith(("EdGraphNode_Comment", "K2Node_Knot", "AnimGraphNode_Reroute"))]
            text = (assets_root / target_path).read_text(encoding="utf-8")
            by_id, ordered = parse_nodes(text)
            spec = specs[target_path]
            if spec["graph_source"].replace("\r\n", "\n") != text:
                failures.append({"kind": "stale_target_spec", "graph": target_path})
            spec_nodes = {node["id"]: node for node in spec["nodes"]}
            original_ids = {node["node_guid"] for node in selected}
            selected_order = [node["node_guid"] for node in selected]
            actual_order = [node["id"] for node in ordered if node["id"] in original_ids]
            if selected_order != actual_order:
                failures.append({"kind": "source_node_order", "graph": target_path, "missing": sorted(original_ids - set(spec_nodes))})
            for node in selected:
                actual = spec_nodes.get(node["node_guid"])
                if actual is None:
                    continue
                if actual["name"].replace("\r\n", "\n") != node["title"].replace("\r\n", "\n") or actual["position"] != [node["position_x"], node["position_y"]]:
                    failures.append({"kind": "source_node_title_or_position", "graph": target_path, "node": node["object_path"]})
                if node["class_path"].endswith("AnimGraphNode_StateMachine"):
                    properties = {entry["name"]: entry["exported_text"] for entry in node["properties"]}
                    for source_name, target_name in (("MaxTransitionsPerFrame", "max_transitions_per_frame"), ("bSkipFirstUpdateTransition", "skip_first_update_transition"), ("bReinitializeOnBecomingRelevant", "reinitialize_on_becoming_relevant")):
                        match = re.search(r"\b" + source_name + r"=(True|False|\d+)", properties["Node"])
                        if match and f"{target_name}: {match.group(1).lower()}" not in by_id[node["node_guid"]]["entry"]:
                            failures.append({"kind": "fsm_runtime_field", "graph": target_path, "node": node["object_path"], "field": source_name})
            edges, _ = parse_edges(text)
            target_edges = collections.defaultdict(list)
            for edge in edges:
                if edge["source"]["kind"] == edge["target"]["kind"] == "node_data":
                    target_edges[edge["source"]["node"]].append(edge["target"]["node"])
            nodes = {node["object_path"]: node for node in raw}
            for edge in source_graph["links"]:
                start, end = edge["source"], edge["target"]
                pin = next(pin for pin in nodes[start["node_object_path"]]["pins"] if pin["pin_id"] == start["pin_id"])
                if source_pin_type(pin) != "Pose" or start["node_guid"] not in original_ids or end["node_guid"] not in original_ids:
                    continue
                totals["source_pose_links"] += 1
                if not reachable_pair(target_edges, start["node_guid"], end["node_guid"], original_ids):
                    failures.append({"kind": "source_pose_link", "graph": target_path, "source": start["node_guid"], "target": end["node_guid"]})
            totals["source_pose_nodes"] += len(selected)
            totals["pose_graphs"] += 1
            rows.append({"source_asset": asset, "source_graph": suffix, "target_graph": target_path, "nodes": len(selected)})
        totals["fsms"] += len(manifest["fsms"])
        totals["condition_graphs"] += len(manifest["conditions"])
    result = {"pass": not failures, "source_assets": len(sources), **totals, "failures": failures, "graphs": rows}
    return result

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-directory", type=Path)
    parser.add_argument("--materialized-directory", type=Path)
    parser.add_argument("--spec", type=Path)
    parser.add_argument("--assets-root", type=Path, default=root / "assets")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        import tempfile
        originals = {"a", "b", "c"}
        assert reachable_pair({"a": ["helper"], "helper": ["b"]}, "a", "b", originals)
        assert not reachable_pair({"a": ["c"], "c": ["b"]}, "a", "b", originals)
        assert not reachable_pair({"a": ["helper"], "helper": ["helper"]}, "a", "b", originals)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "spec.json").write_text("{}", encoding="utf-8")
            (path / "conversion.report.json").write_text('{"pass":true,"unsupported":[],"source":{"package_path":"/Test/A.A"}}', encoding="utf-8")
            (path / "integration.report.json").write_text('{"conversion_status":"ready","source_closure":[{"source_asset":"/Test/A.A"}]}', encoding="utf-8")
            try:
                audit(path, path, path / "spec.json", path)
            except ValueError as error:
                assert "native source closure is incomplete" in str(error)
            else:
                raise AssertionError("an empty source closure was accepted")
        print('{"compiler_helper_path": "PASS", "unrelated_source_node_rejected": "PASS", "helper_cycle_terminates": "PASS"}')
        return 0
    try:
        if not all((args.source_directory, args.materialized_directory, args.spec)):
            raise ValueError("source directory, materialized directory and spec are required")
        result = audit(args.source_directory, args.materialized_directory, args.spec, args.assets_root)
    except (OSError, ValueError, KeyError, StopIteration) as error:
        result = {"pass": False, "failures": [{"kind": type(error).__name__, "detail": str(error)}]}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key != "graphs"}, indent=2))
    return int(not result["pass"])


if __name__ == "__main__":
    raise SystemExit(main())
