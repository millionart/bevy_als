import argparse
import hashlib
import json
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IMPORT_ROOT = ROOT / "tools" / "import"
sys.path.insert(0, str(IMPORT_ROOT))

from capture_animgraph_ron_topology import entries, parse_positions
from normalize_animgraph_io_order import field_span, split_top_level, top_level_colon


MANIFEST_FORMAT = "als_animgraph_context_manifest_v1"
BASELINE_FORMAT = "als_animgraph_context_baseline_v1"


def compact(value):
    return re.sub(r"\s+", "", value)


def sha256_text(value):
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def relative_path(path, root):
    return path.resolve().relative_to(root.resolve()).as_posix()


def repo_path(root, value):
    path = (root / value).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError as error:
        raise ValueError(f"path escapes repository root: {value}") from error
    return path


def read_text(path):
    with path.open("r", encoding="utf-8", newline="") as file:
        return file.read()


def read_json(path):
    return json.loads(read_text(path))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def quoted(value):
    match = re.fullmatch(r'"([^"\\]*(?:\\.[^"\\]*)*)"', value.strip())
    if match is None:
        raise ValueError(f"expected quoted string: {value}")
    return json.loads(match.group(0))


def passthrough(value):
    match = re.fullmatch(r'Passthrough\("([^"\\]*(?:\\.[^"\\]*)*)"\)', value.strip())
    if match is None:
        raise ValueError(f"expected passthrough key: {value}")
    return json.loads(f'"{match.group(1)}"')


def parse_data_map(text, field_name, input_keys):
    _, start, end = field_span(text, field_name)
    result = {}
    for entry in split_top_level(text[start + 1 : end], ","):
        key, value = top_level_colon(entry)
        name = passthrough(key) if input_keys else quoted(key)
        if name in result:
            raise ValueError(f"duplicate data key {name} in {field_name}")
        result[name] = value.strip()
    return result


def parse_io_spec(path):
    text = read_text(path)
    return {
        "input_data": parse_data_map(text, "input_data", True),
        "output_data": parse_data_map(text, "output_data", False),
    }


def parse_string_array(entry, field_name):
    try:
        _, start, end = field_span(entry, field_name)
    except ValueError:
        return []
    values = []
    for value in split_top_level(entry[start + 1 : end], ","):
        values.append(quoted(value))
    if len(values) != len(set(values)):
        raise ValueError(f"duplicate {field_name} value")
    return values


def graph_handle(entry):
    match = re.search(r'\b(?:graph|interface)\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"', entry)
    if match is None:
        return None
    return json.loads(f'"{match.group(1)}"')


def parse_nodes(text):
    _, _, _, raw_nodes = entries(text, "nodes")
    nodes = {}
    ordered = []
    for index, entry in enumerate(raw_nodes):
        id_match = re.search(r'\bid\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"', entry)
        name_match = re.search(r'\bname\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"', entry)
        if id_match is None or name_match is None:
            raise ValueError("node is missing id or name")
        node_id = json.loads(f'"{id_match.group(1)}"')
        if node_id in nodes:
            raise ValueError(f"duplicate node id {node_id}")
        node = {
            "id": node_id,
            "name": json.loads(f'"{name_match.group(1)}"'),
            "index": index,
            "graph": graph_handle(entry),
            "curve_names": parse_string_array(entry, "curve_names"),
        }
        nodes[node_id] = node
        ordered.append(node)
    return nodes, ordered


def parse_pin(value):
    text = value.strip()
    data_match = re.fullmatch(r'NodeData\("([^"\\]*(?:\\.[^"\\]*)*)",\s*"([^"\\]*(?:\\.[^"\\]*)*)"\)', text)
    if data_match is not None:
        return {
            "kind": "node_data",
            "node": json.loads(f'"{data_match.group(1)}"'),
            "pin": json.loads(f'"{data_match.group(2)}"'),
        }
    time_match = re.fullmatch(r'NodeTime\("([^"\\]*(?:\\.[^"\\]*)*)"(?:,\s*"([^"\\]*(?:\\.[^"\\]*)*)")?\)', text)
    if time_match is not None:
        return {
            "kind": "node_time",
            "node": json.loads(f'"{time_match.group(1)}"'),
            "pin": None if time_match.group(2) is None else json.loads(f'"{time_match.group(2)}"'),
        }
    return {"kind": "other", "raw": compact(text)}


def pin_key(pin):
    if pin["kind"] == "other":
        return ("other", pin["raw"])
    return (pin["kind"], pin["node"], pin["pin"])


def pin_text(pin):
    if pin["kind"] == "other":
        return pin["raw"]
    if pin["kind"] == "node_data":
        return f'NodeData("{pin["node"]}", "{pin["pin"]}")'
    if pin["pin"] is None:
        return f'NodeTime("{pin["node"]}")'
    return f'NodeTime("{pin["node"]}", "{pin["pin"]}")'


def parse_edges(text):
    _, start, end = field_span(text, "edges_inverted")
    result = []
    targets = {}
    for index, entry in enumerate(split_top_level(text[start + 1 : end], ",")):
        target_text, source_text = top_level_colon(entry)
        target = parse_pin(target_text)
        source = parse_pin(source_text)
        key = pin_key(target)
        if key in targets:
            raise ValueError(f"duplicate edge target {target_text}")
        edge = {
            "index": index,
            "target": target,
            "source": source,
            "target_text": target_text.strip(),
            "source_text": source_text.strip(),
        }
        targets[key] = edge
        result.append(edge)
    return result, targets


def data_pin(node, pin):
    return {"kind": "node_data", "node": node, "pin": pin}


def time_pin(node, pin=None):
    return {"kind": "node_time", "node": node, "pin": pin}


def edge_for(targets, target):
    return targets.get(pin_key(target))


def node_channels(node, asset_root, direction, prefix, cache):
    if node["graph"] is not None:
        graph_path = repo_path(asset_root, node["graph"])
        if graph_path not in cache:
            cache[graph_path] = parse_io_spec(graph_path)
        field = "output_data" if direction == "output" else "input_data"
        values = [name for name, spec in cache[graph_path][field].items() if spec == "F32"]
    else:
        values = list(node["curve_names"])
        if direction == "input":
            values = [f"{prefix}{value}" for value in values]
    return set(values)


def node_description(node):
    return {"id": node["id"], "name": node["name"]}


def context_shape_issues(manifest):
    issues = []
    if manifest.get("format") != MANIFEST_FORMAT:
        issues.append("manifest format is invalid")
    if not isinstance(manifest.get("asset_root"), str):
        issues.append("asset_root is missing")
    if not isinstance(manifest.get("graph"), str):
        issues.append("graph is missing")
    if manifest.get("attributes", "reject") != "reject":
        issues.append("attributes must be reject")
    if manifest.get("events", "reject") != "reject":
        issues.append("events must be reject")
    contexts = manifest.get("contexts")
    if not isinstance(contexts, list) or not contexts:
        issues.append("contexts must be a non-empty array")
        return issues
    seen_ids = set()
    for index, context in enumerate(contexts):
        label = f"context[{index}]"
        if not isinstance(context, dict):
            issues.append(f"{label} must be an object")
            continue
        context_id = context.get("id")
        if not isinstance(context_id, str) or not context_id:
            issues.append(f"{label} id is invalid")
        elif context_id in seen_ids:
            issues.append(f"duplicate context id {context_id}")
        else:
            seen_ids.add(context_id)
        if "attributes" in context or "events" in context or "controls" in context:
            issues.append(f"{label} contains a non-context channel")
        source = context.get("source")
        target = context.get("target")
        if not isinstance(source, dict) or not isinstance(target, dict):
            issues.append(f"{label} source and target are required")
        else:
            for side, endpoint in (("source", source), ("target", target)):
                if not isinstance(endpoint.get("node"), str) or not isinstance(endpoint.get("pose"), str):
                    issues.append(f"{label} {side} endpoint is invalid")
        time = context.get("time")
        if not isinstance(time, dict) or set(time) != {"target"} or not isinstance(time.get("target"), str):
            issues.append(f"{label} time must declare only target")
        curves = context.get("curves")
        if not isinstance(curves, dict):
            issues.append(f"{label} curves must be an object")
        elif set(curves) != {"mode", "target_prefix"}:
            issues.append(f"{label} curves has unsupported fields")
        elif curves.get("mode") != "inherit" or not isinstance(curves.get("target_prefix"), str):
            issues.append(f"{label} curves inheritance is invalid")
    return issues


def source_matches(edge, expected):
    return pin_key(edge["source"]) == pin_key(expected)


def build_context(context, nodes, targets, asset_root, cache):
    issues = []
    source_id = context["source"]["node"]
    target_id = context["target"]["node"]
    context_id = context["id"]
    if source_id not in nodes:
        return None, [f"{context_id} source node is missing"]
    if target_id not in nodes:
        return None, [f"{context_id} target node is missing"]
    source_node = nodes[source_id]
    target_node = nodes[target_id]
    pose_target = data_pin(target_id, context["target"]["pose"])
    pose_source = data_pin(source_id, context["source"]["pose"])
    pose_edge = edge_for(targets, pose_target)
    if pose_edge is None:
        issues.append(f"{context_id} pose edge is missing")
    elif not source_matches(pose_edge, pose_source):
        issues.append(f"{context_id} pose edge crosses context")
    time_target = time_pin(target_id, context["time"]["target"])
    time_source = time_pin(source_id)
    time_edge = edge_for(targets, time_target)
    if time_edge is None:
        issues.append(f"{context_id} time edge is missing")
    elif not source_matches(time_edge, time_source):
        issues.append(f"{context_id} time edge crosses context")
    prefix = context["curves"]["target_prefix"]
    source_channels = node_channels(source_node, asset_root, "output", "", cache)
    target_channels = node_channels(target_node, asset_root, "input", prefix, cache)
    discovered = sorted(channel for channel in source_channels if f"{prefix}{channel}" in target_channels)
    curve_edges = []
    for channel in discovered:
        target = data_pin(target_id, f"{prefix}{channel}")
        source = data_pin(source_id, channel)
        edge = edge_for(targets, target)
        if edge is None:
            issues.append(f"{context_id} curve edge is missing for {channel}")
        elif not source_matches(edge, source):
            issues.append(f"{context_id} curve edge crosses context for {channel}")
        else:
            curve_edges.append(edge)
    for edge in targets.values():
        target = edge["target"]
        source = edge["source"]
        if target["kind"] != "node_data" or source["kind"] != "node_data":
            continue
        if target["node"] != target_id or source["node"] != source_id:
            continue
        if pin_key(target) == pin_key(pose_target):
            continue
        if not target["pin"].startswith(prefix):
            continue
        suffix = target["pin"][len(prefix) :]
        if source["pin"] not in source_channels:
            issues.append(f"{context_id} curve source is undeclared for {target['pin']}")
        elif suffix != source["pin"]:
            issues.append(f"{context_id} curve mapping is not inherited for {target['pin']}")
    managed = []
    if pose_edge is not None and source_matches(pose_edge, pose_source):
        managed.append(pose_edge)
    if time_edge is not None and source_matches(time_edge, time_source):
        managed.append(time_edge)
    managed.extend(curve_edges)
    return {
        "id": context_id,
        "source": node_description(source_node),
        "target": node_description(target_node),
        "pose": {"source": pin_text(pose_source), "target": pin_text(pose_target)},
        "time": {"source": pin_text(time_source), "target": pin_text(time_target)},
        "curve_inheritance": {
            "target_prefix": prefix,
            "source_channels": sorted(source_channels),
            "target_channels": sorted(target_channels),
            "discovered": discovered,
        },
        "managed_edges": [
            {"target": edge["target_text"], "source": edge["source_text"]}
            for edge in managed
        ],
        "managed_target_keys": [pin_key(edge["target"]) for edge in managed],
    }, issues


def build_baseline(root, manifest_path):
    manifest = read_json(manifest_path)
    shape_issues = context_shape_issues(manifest)
    if shape_issues:
        return None, shape_issues
    asset_root = repo_path(root, manifest["asset_root"])
    graph_path = repo_path(asset_root, manifest["graph"])
    graph_text = read_text(graph_path)
    nodes, ordered_nodes = parse_nodes(graph_text)
    edges, targets = parse_edges(graph_text)
    cache = {}
    contexts = []
    issues = []
    managed_targets = {}
    for context in manifest["contexts"]:
        result, context_issues = build_context(context, nodes, targets, asset_root, cache)
        issues.extend(context_issues)
        if result is None:
            continue
        contexts.append(result)
        for target in result["managed_target_keys"]:
            if target in managed_targets:
                issues.append(f"managed target is shared by {managed_targets[target]} and {result['id']}")
            else:
                managed_targets[target] = result["id"]
    positions = parse_positions(graph_text, "node_positions")
    if positions["malformed"]:
        issues.append("node position metadata is malformed")
    normalized_positions = [
        {
            "id": entry["id"],
            "position": list(entry["position"]),
            "index": entry["index"],
        }
        for entry in positions["entries"]
    ]
    baseline = {
        "format": BASELINE_FORMAT,
        "manifest": relative_path(manifest_path, root),
        "manifest_sha256": sha256_text(read_text(manifest_path)),
        "asset_root": manifest["asset_root"],
        "graph": manifest["graph"],
        "graph_sha256": sha256_text(graph_text),
        "nodes": [node_description(node) for node in ordered_nodes],
        "node_positions": normalized_positions,
        "edges": [
            {"target": edge["target_text"], "source": edge["source_text"]}
            for edge in edges
        ],
        "contexts": [
            {key: value for key, value in context.items() if key != "managed_target_keys"}
            for context in contexts
        ],
        "unmanaged_edges": [
            {"target": edge["target_text"], "source": edge["source_text"]}
            for edge in edges
            if pin_key(edge["target"]) not in managed_targets
        ],
    }
    return baseline, issues


def compare_baseline(expected, actual):
    if expected == actual:
        return []
    differences = []
    for key in sorted(set(expected) | set(actual)):
        if expected.get(key) != actual.get(key):
            differences.append(key)
    return differences


def fixture_graph(output_data, input_data, nodes, edges):
    return (
        "(\n"
        f"    nodes: [{nodes}],\n"
        f"    edges_inverted: {{{edges}}},\n"
        "    io_spec: (\n"
        "        input_times: [],\n"
        f"        input_data: {{{input_data}}},\n"
        "        output_time: true,\n"
        f"        output_data: {{{output_data}}},\n"
        "        input_order: {},\n"
        "        output_order: {},\n"
        "    ),\n"
        "    default_data: {},\n"
        "    editor_metadata: (node_positions: {}, input_position: (0.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n"
        ")\n"
    )


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        asset_root = root / "assets"
        graphs = asset_root / "graphs"
        graphs.mkdir(parents=True)
        source = fixture_graph(
            '"pose": Pose, "Existing": F32, "Future": F32',
            "",
            "",
            "",
        )
        target = fixture_graph(
            '"pose": Pose',
            'Passthrough("Input"): Pose, Passthrough("ctx_Existing"): F32, Passthrough("ctx_Future"): F32',
            "",
            "",
        )
        source_path = graphs / "source.animgraph.ron"
        target_path = graphs / "target.animgraph.ron"
        source_path.write_text(source, encoding="utf-8")
        target_path.write_text(target, encoding="utf-8")
        source_node = "source-node"
        target_node = "target-node"
        nodes = (
            f'(id: "{source_node}", name: "Source", inner: {{ "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }}, should_debug: false), '
            f'(id: "{target_node}", name: "Target", inner: {{ "als_ext::LayeredGraphNode": (graph: "graphs/target.animgraph.ron") }}, should_debug: false)'
        )
        edges = (
            f'NodeData("{target_node}", "Input"): NodeData("{source_node}", "pose"), '
            f'NodeTime("{target_node}", "input_time"): NodeTime("{source_node}"), '
            f'NodeData("{target_node}", "ctx_Existing"): NodeData("{source_node}", "Existing"), '
            f'NodeData("{target_node}", "ctx_Future"): NodeData("{source_node}", "Future")'
        )
        graph = fixture_graph('"pose": Pose', "", nodes, edges)
        graph_path = graphs / "root.animgraph.ron"
        graph_path.write_text(graph, encoding="utf-8")
        manifest = {
            "format": MANIFEST_FORMAT,
            "asset_root": "assets",
            "graph": "graphs/root.animgraph.ron",
            "attributes": "reject",
            "events": "reject",
            "contexts": [
                {
                    "id": "source_to_target",
                    "source": {"node": source_node, "pose": "pose"},
                    "target": {"node": target_node, "pose": "Input"},
                    "time": {"target": "input_time"},
                    "curves": {"mode": "inherit", "target_prefix": "ctx_"},
                }
            ],
        }
        manifest_path = root / "manifest.json"
        write_json(manifest_path, manifest)
        baseline, issues = build_baseline(root, manifest_path)
        assert not issues
        assert baseline["contexts"][0]["curve_inheritance"]["discovered"] == ["Existing", "Future"]
        malformed = dict(manifest)
        malformed["contexts"] = [dict(manifest["contexts"][0], curves=["Existing"])]
        assert context_shape_issues(malformed)
        duplicate = dict(manifest)
        duplicate["contexts"] = [manifest["contexts"][0], dict(manifest["contexts"][0])]
        assert context_shape_issues(duplicate)
        invalid_time = dict(manifest)
        invalid_time["contexts"] = [
            dict(manifest["contexts"][0], time={"target": "input_time", "mode": "packed"})
        ]
        assert context_shape_issues(invalid_time)
        invalid_event = dict(manifest)
        invalid_event["contexts"] = [dict(manifest["contexts"][0], events="inherit")]
        assert context_shape_issues(invalid_event)
        mixed = graph.replace(
            f'NodeData("{source_node}", "Existing")',
            f'NodeData("{source_node}", "Future")',
        )
        graph_path.write_text(mixed, encoding="utf-8")
        _, mixed_issues = build_baseline(root, manifest_path)
        assert mixed_issues
        undeclared = graph.replace(
            f'NodeData("{source_node}", "Existing")',
            f'NodeData("{source_node}", "Unknown")',
        )
        graph_path.write_text(undeclared, encoding="utf-8")
        _, undeclared_issues = build_baseline(root, manifest_path)
        assert any("undeclared" in issue for issue in undeclared_issues)
    return {
        "open_curve_inheritance": "PASS",
        "fixed_curve_fields_rejected": "PASS",
        "duplicate_context_rejected": "PASS",
        "illegal_time_or_event_packing_rejected": "PASS",
        "cross_context_mix_rejected": "PASS",
        "undeclared_curve_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.manifest is None:
        raise ValueError("manifest is required")
    root = args.root.resolve()
    manifest_path = repo_path(root, args.manifest)
    baseline, issues = build_baseline(root, manifest_path)
    report = {"issues": issues}
    if baseline is not None:
        report["contexts"] = len(baseline["contexts"])
        report["edges"] = len(baseline["edges"])
        report["unmanaged_edges"] = len(baseline["unmanaged_edges"])
    if issues:
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1
    if args.write_baseline:
        if args.baseline is None:
            raise ValueError("baseline is required when writing")
        baseline_path = repo_path(root, args.baseline)
        write_json(baseline_path, baseline)
        report["baseline_written"] = relative_path(baseline_path, root)
    if args.check:
        if args.baseline is None:
            raise ValueError("baseline is required when checking")
        baseline_path = repo_path(root, args.baseline)
        expected = read_json(baseline_path)
        differences = compare_baseline(expected, baseline)
        report["baseline_differences"] = differences
        if differences:
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 1
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
