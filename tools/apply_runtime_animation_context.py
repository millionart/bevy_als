import argparse
import json
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "import"))

from animgraph_context_contract import parse_edges, read_json, read_text, repo_path
from capture_animgraph_ron_topology import entries
from normalize_animgraph_io_order import field_span, split_top_level, top_level_colon


def node_id(entry):
    match = re.search(r'\bid\s*:\s*"([^"\\]*(?:\\.[^"\\]*)*)"', entry)
    if match is None:
        raise ValueError("node id is missing")
    return json.loads(f'"{match.group(1)}"')


def newline_for(text):
    return "\r\n" if "\r\n" in text else "\n"


def render_collection(text, field_name, values):
    _, start, end = field_span(text, field_name)
    newline = newline_for(text)
    line_start = text.rfind("\n", 0, start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    if values:
        replacement = "[" + newline
        replacement += "".join(f"{inner_indent}{value},{newline}" for value in values)
        replacement += f"{base_indent}]"
    else:
        replacement = "[]"
    return text[:start] + replacement + text[end + 1 :]


def render_map(text, field_name, values):
    _, start, end = field_span(text, field_name)
    newline = newline_for(text)
    line_start = text.rfind("\n", 0, start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    if values:
        replacement = "{" + newline
        replacement += "".join(f"{inner_indent}{value},{newline}" for value in values)
        replacement += f"{base_indent}}}"
    else:
        replacement = "{}"
    return text[:start] + replacement + text[end + 1 :]


def context_inputs(manifest):
    grouped = {}
    for context in manifest["contexts"]:
        target = context["target"]["node"]
        grouped.setdefault(target, []).append(
            {
                "pose_input": context["target"]["pose"],
                "time_input": context["time"]["target"],
                "curve_prefix": context["curves"]["target_prefix"],
                "missing_curve_value": context.get("missing_curve_value"),
            }
        )
    return grouped


def render_context_input(value):
    default = (
        "None"
        if value["missing_curve_value"] is None
        else f"Some({float(value['missing_curve_value']):.1f})"
    )
    return (
        f'(pose_input: "{value["pose_input"]}", '
        f'time_input: "{value["time_input"]}", '
        f'curve_prefix: "{value["curve_prefix"]}", '
        f"missing_curve_value: {default})"
    )


def runtime_fields(config, inputs):
    values = [f"capture_context: {'true' if config['capture_context'] else 'false'}"]
    values.append(
        f"expose_context_curves: {'true' if config['expose_context_curves'] else 'false'}"
    )
    outputs = ", ".join(json.dumps(value) for value in config["exposed_output_data"])
    values.append(f"exposed_output_data: [{outputs}]")
    if config["mode"] == "child":
        rendered = ", ".join(render_context_input(value) for value in inputs)
        values.insert(0, f"context_inputs: [{rendered}]")
    return ", ".join(values)


def inject_node_fields(entry, config, inputs):
    if "capture_context:" in entry:
        return entry
    if config["mode"] == "child" and "LayeredGraphNode" not in entry:
        raise ValueError(f"child context mode has an unsupported node: {config['node']}")
    if config["mode"] == "named" and not (
        "InertializationPassthroughNode" in entry or "ControlRigBoundaryNode" in entry
    ):
        raise ValueError(f"named context mode has an unsupported node: {config['node']}")
    marker = ") }, should_debug:"
    position = entry.rfind(marker)
    if position < 0:
        raise ValueError(f"node inner data boundary is missing: {config['node']}")
    return entry[:position] + ", " + runtime_fields(config, inputs) + entry[position:]


def removable_curve_edge(edge, manifest):
    target = edge["target"]
    source = edge["source"]
    if target["kind"] != "node_data":
        return False
    for context in manifest["contexts"]:
        if target["node"] != context["target"]["node"]:
            continue
        if target["pin"] == context["target"]["pose"]:
            continue
        prefix = context["curves"]["target_prefix"]
        source_node = context["source"]["node"]
        if prefix:
            if not target["pin"].startswith(prefix):
                continue
            if context.get("missing_curve_value") is not None:
                return True
            return source["kind"] == "node_data" and source["node"] == source_node
        if source["kind"] == "node_data" and source["node"] == source_node:
            return True
    return False


def remove_node_positions(text, node_ids):
    for node_id in node_ids:
        escaped = re.escape(node_id)
        text, count = re.subn(
            rf'\s*"{escaped}"\s*:\s*\([^()]*\)\s*,?',
            "",
            text,
        )
        if count > 1:
            raise ValueError(f"node position removal failed: {node_id}")
    return text


def transform(text, manifest):
    _, _, _, raw_nodes = entries(text, "nodes")
    configs = {value["node"]: value for value in manifest["runtime_nodes"]}
    grouped_inputs = context_inputs(manifest)
    removed_nodes = set(manifest.get("remove_nodes", []))
    transformed_nodes = []
    seen_configs = set()
    for entry in raw_nodes:
        current = node_id(entry)
        if current in removed_nodes:
            continue
        config = configs.get(current)
        if config is not None:
            entry = inject_node_fields(entry, config, grouped_inputs.get(current, []))
            seen_configs.add(current)
        transformed_nodes.append(entry)
    missing = sorted(set(configs) - seen_configs)
    if missing:
        raise ValueError(f"runtime context nodes are missing: {missing}")
    text = render_collection(text, "nodes", transformed_nodes)
    edges, _ = parse_edges(text)
    retained_edges = []
    removed_edges = []
    for edge in edges:
        if removable_curve_edge(edge, manifest):
            removed_edges.append(edge)
            continue
        if (
            edge["source"]["kind"] == "node_data"
            and edge["source"]["node"] in removed_nodes
        ):
            removed_edges.append(edge)
            continue
        retained_edges.append(f"{edge['target_text']}: {edge['source_text']}")
    text = render_map(text, "edges_inverted", retained_edges)
    text = remove_node_positions(text, removed_nodes)
    return text, {"removed_edges": len(removed_edges), "retained_edges": len(retained_edges)}


def audit(text, manifest):
    _, _, _, raw_nodes = entries(text, "nodes")
    by_id = {node_id(entry): entry for entry in raw_nodes}
    failures = []
    for config in manifest["runtime_nodes"]:
        entry = by_id.get(config["node"])
        if entry is None:
            failures.append(f"runtime node missing {config['node']}")
            continue
        if "capture_context: true" not in entry:
            failures.append(f"runtime node does not capture context {config['node']}")
    for removed_id in manifest.get("remove_nodes", []):
        if removed_id in by_id:
            failures.append(f"removed node remains {removed_id}")
    edges, _ = parse_edges(text)
    residual = [edge for edge in edges if removable_curve_edge(edge, manifest)]
    if residual:
        failures.append(f"residual context curve edges {len(residual)}")
    return {
        "nodes": len(by_id),
        "edges": len(edges),
        "residual_context_curve_edges": len(residual),
        "failures": failures,
    }


def run_self_test():
    manifest = {
        "contexts": [
            {
                "source": {"node": "source", "pose": "pose"},
                "target": {"node": "target", "pose": "Input"},
                "time": {"target": "input_time"},
                "curves": {"target_prefix": "ctx_"},
                "missing_curve_value": 0.0,
            }
        ],
        "runtime_nodes": [
            {
                "node": "source",
                "mode": "child",
                "capture_context": True,
                "expose_context_curves": False,
                "exposed_output_data": [],
            },
            {
                "node": "target",
                "mode": "child",
                "capture_context": True,
                "expose_context_curves": False,
                "exposed_output_data": [],
            },
        ],
        "remove_nodes": ["zero"],
    }
    text = (
        "(\n"
        "nodes: ["
        "(id: \"source\", name: \"Source\", inner: { \"als_ext::LayeredGraphNode\": (graph: \"graphs/source.animgraph.ron\") }, should_debug: false),"
        "(id: \"target\", name: \"Target\", inner: { \"als_ext::LayeredGraphNode\": (graph: \"graphs/target.animgraph.ron\") }, should_debug: false),"
        "(id: \"zero\", name: \"Zero\", inner: { \"als_ext::ConstantF32Node\": (value: 0.0) }, should_debug: false),"
        "],\n"
        "edges_inverted: {"
        "NodeData(\"target\", \"Input\"): NodeData(\"source\", \"pose\"),"
        "NodeTime(\"target\", \"input_time\"): NodeTime(\"source\"),"
        "NodeData(\"target\", \"ctx_Curve\"): NodeData(\"source\", \"Curve\"),"
        "NodeData(\"target\", \"ctx_Default\"): NodeData(\"zero\", \"value\"),"
        "},\n"
        "io_spec: (input_times: [], input_data: {}, output_time: true, output_data: { \"pose\": Pose }, input_order: {}, output_order: {}),\n"
        "default_data: {},\n"
        "editor_metadata: (node_positions: { \"source\": (0.0, 0.0), \"target\": (100.0, 0.0), \"zero\": (50.0, 100.0) }, input_position: (-64.0, 0.0), output_position: (200.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n"
        ")\n"
    )
    transformed, report = transform(text, manifest)
    assert report == {"removed_edges": 2, "retained_edges": 2}
    checked = audit(transformed, manifest)
    assert not checked["failures"]
    assert "ctx_Curve" not in transformed
    assert "ctx_Default" not in transformed
    assert "capture_context: true" in transformed
    rerun, rerun_report = transform(transformed, manifest)
    assert rerun == transformed
    assert rerun_report == {"removed_edges": 0, "retained_edges": 2}
    return {"runtime_context_graph_transform": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--graph", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.manifest is None or args.graph is None:
        raise ValueError("manifest and graph are required")
    root = args.root.resolve()
    manifest = read_json(repo_path(root, args.manifest))
    graph_path = repo_path(root, args.graph)
    text = read_text(graph_path)
    if args.write:
        transformed, report = transform(text, manifest)
        with graph_path.open("w", encoding="utf-8", newline="") as file:
            file.write(transformed)
    elif args.check:
        report = audit(text, manifest)
    else:
        raise ValueError("select write, check, or self-test")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not report.get("failures") else 1


if __name__ == "__main__":
    raise SystemExit(main())
