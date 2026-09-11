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
from ue_animgraph_pose_contract import node_struct_fields, visible_input_default
from ue_exported_text import field, parse_bool, parse_number, parse_list

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

def compiled_asset_differences(output, assets_root, paths=None):
    paths = list(output.rglob("*.ron")) if paths is None else paths
    failures = []
    for path in paths:
        relative = path.relative_to(output)
        installed = assets_root / relative
        if not installed.is_file() or installed.read_text(encoding="utf-8") != path.read_text(encoding="utf-8"):
            failures.append({"kind": "compiled_asset_drift", "asset": relative.as_posix()})
    return len(paths), failures

def source_runtime_field_mismatches(node, target):
    failures = []
    expected_values = []
    if node["class_path"].endswith("AnimGraphNode_StateMachine"):
        record = next(entry for entry in node["properties"] if entry["name"] == "Node")
        fields = {entry["name"]: entry["exported_text"] for entry in record.get("fields", [])}
        for source_name, target_name in (("MaxTransitionsPerFrame", "max_transitions_per_frame"), ("bSkipFirstUpdateTransition", "skip_first_update_transition"), ("bReinitializeOnBecomingRelevant", "reinitialize_on_becoming_relevant")):
            match = re.search(r"\b" + source_name + r"=(True|False|\d+)", record["exported_text"])
            value = fields.get(source_name, match.group(1) if match else None)
            if value is not None and f"{target_name}: {value.lower()}" not in target:
                failures.append(source_name)
    elif node["class_path"].endswith("AnimGraphNode_SequenceEvaluator"):
        fields = node_struct_fields(node, "Node")
        if parse_bool(field(fields, "bUseExplicitFrame", "False")):
            expected = visible_input_default(node, "ExplicitFrame", parse_number(field(fields, "ExplicitFrame", "0")))
            actual = re.search(r"\bexplicit_frame:\s*([-+0-9.eE]+)", target)
            if actual is None or float(actual.group(1)) != expected:
                failures.append("ExplicitFrame")
    elif node["class_path"].endswith("AnimGraphNode_SequencePlayer"):
        fields = node_struct_fields(node, "Node")
        expected_values = [
            (name, target_name, visible_input_default(node, name, parse_number(field(fields, name, default))))
            for name, target_name, default in (("PlayRate", "play_rate", "1"), ("StartPosition", "start_position", "0"))
        ]
    elif node["class_path"].endswith("AnimGraphNode_BlendListByEnum"):
        fields = node_struct_fields(node, "Node")
        times = [parse_number(value) for value in parse_list(field(fields, "BlendTime"))]
        expected_values = [
            (f"BlendTime_{index}", target_name, visible_input_default(node, f"BlendTime_{index}", times[index]))
            for index, target_name in enumerate(("default_blend_time", "selected_blend_time"))
        ]
    elif node["class_path"].endswith("AnimGraphNode_LayeredBoneBlend"):
        fields = node_struct_fields(node, "Node")
        value = parse_bool(field(fields, "bUpdateBasePoseFirst", "False"))
        if f"update_base_pose_first: {str(value).lower()}" not in target:
            failures.append("bUpdateBasePoseFirst")
    for name, target_name, expected in expected_values:
        actual = re.search(r"\b" + target_name + r":\s*([-+0-9.eE]+)", target)
        if actual is None or float(actual.group(1)) != expected:
            failures.append(name)
    return failures

def audit(source_directory, output, spec_path, assets_root, source_asset=None):
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
    paths = None
    if source_asset is not None:
        if source_asset not in sources:
            raise ValueError("requested source asset is outside the complete conversion closure")
        sources = {source_asset: sources[source_asset]}
        manifest_path = output / ("materialization.manifest.json" if source_asset == root_asset else "linked_manifests/" + source_identifier_key(source_asset) + ".json")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        paths = list((output / Path(manifest["root_graph"]).parent).rglob("*.ron"))
    totals["compiled_assets"], compiled_failures = compiled_asset_differences(output, assets_root, paths)
    failures.extend(compiled_failures)
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
                for field in source_runtime_field_mismatches(node, by_id[node["node_guid"]]["entry"]):
                    failures.append({"kind": "source_runtime_field", "graph": target_path, "node": node["object_path"], "field": field})
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
    result = {"pass": not failures, "source_assets": len(sources), "source_scope": source_asset, **totals, "failures": failures, "graphs": rows}
    return result

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-directory", type=Path)
    parser.add_argument("--materialized-directory", type=Path)
    parser.add_argument("--spec", type=Path)
    parser.add_argument("--assets-root", type=Path, default=root / "assets")
    parser.add_argument("--source-asset")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        import tempfile
        machine = {"class_path": "/Script/AnimGraph.AnimGraphNode_StateMachine", "properties": [{
            "name": "Node", "exported_text": "(MaxTransitionsPerFrame=3)",
            "fields": [{"name": "bSkipFirstUpdateTransition", "exported_text": "False"}],
        }]}
        assert source_runtime_field_mismatches(machine, "max_transitions_per_frame: 3, skip_first_update_transition: true") == ["bSkipFirstUpdateTransition"]
        assert not source_runtime_field_mismatches(machine, "max_transitions_per_frame: 3, skip_first_update_transition: false")
        evaluator = {"class_path": "/Script/AnimGraph.AnimGraphNode_SequenceEvaluator", "properties": [{
            "name": "Node", "exported_text": "(ExplicitFrame=21,bUseExplicitFrame=True)",
        }], "pins": [{"name": "ExplicitFrame", "direction": "Input", "hidden": False, "default_value": "21"}]}
        assert source_runtime_field_mismatches(evaluator, "explicit_frame: 0.0") == ["ExplicitFrame"]
        assert not source_runtime_field_mismatches(evaluator, "explicit_frame: 21.0")
        player = {"class_path": "/Script/AnimGraph.AnimGraphNode_SequencePlayer", "properties": [{
            "name": "Node", "exported_text": "(StartPosition=0.25,PlayRate=0)",
        }]}
        assert source_runtime_field_mismatches(player, "start_position: 0.0, play_rate: 1.0") == ["PlayRate", "StartPosition"]
        assert not source_runtime_field_mismatches(player, "start_position: 0.25, play_rate: 0.0")
        originals = {"a", "b", "c"}
        assert reachable_pair({"a": ["helper"], "helper": ["b"]}, "a", "b", originals)
        assert not reachable_pair({"a": ["c"], "c": ["b"]}, "a", "b", originals)
        assert not reachable_pair({"a": ["helper"], "helper": ["helper"]}, "a", "b", originals)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            compiled = path / "compiled"
            installed = path / "installed"
            compiled.mkdir()
            installed.mkdir()
            (compiled / "state.fsm.ron").write_text("source", encoding="utf-8")
            assert compiled_asset_differences(compiled, installed)[1]
            (installed / "state.fsm.ron").write_text("source", encoding="utf-8")
            assert compiled_asset_differences(compiled, installed) == (1, [])
            (compiled / "other.animgraph.ron").write_text("other owner", encoding="utf-8")
            assert compiled_asset_differences(compiled, installed)[1]
            assert compiled_asset_differences(compiled, installed, [compiled / "state.fsm.ron"]) == (1, [])
            (installed / "state.fsm.ron").write_text("changed", encoding="utf-8")
            assert compiled_asset_differences(compiled, installed)[1]
            (path / "spec.json").write_text("{}", encoding="utf-8")
            (path / "conversion.report.json").write_text('{"pass":true,"unsupported":[],"source":{"package_path":"/Test/A.A"}}', encoding="utf-8")
            (path / "integration.report.json").write_text('{"conversion_status":"ready","source_closure":[{"source_asset":"/Test/A.A"}]}', encoding="utf-8")
            try:
                audit(path, path, path / "spec.json", path)
            except ValueError as error:
                assert "native source closure is incomplete" in str(error)
            else:
                raise AssertionError("an empty source closure was accepted")
        print('{"compiler_helper_path": "PASS", "unrelated_source_node_rejected": "PASS", "helper_cycle_terminates": "PASS", "compiled_asset_drift_rejected": "PASS", "structured_fsm_default_rejected": "PASS", "wrong_explicit_frame_rejected": "PASS"}')
        return 0
    try:
        if not all((args.source_directory, args.materialized_directory, args.spec)):
            raise ValueError("source directory, materialized directory and spec are required")
        result = audit(args.source_directory, args.materialized_directory, args.spec, args.assets_root, args.source_asset)
    except (OSError, ValueError, KeyError, StopIteration) as error:
        result = {"pass": False, "failures": [{"kind": type(error).__name__, "detail": str(error)}]}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key != "graphs"}, indent=2))
    return int(not result["pass"])


if __name__ == "__main__":
    raise SystemExit(main())
