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


def render_global_data_input(value):
    return f'(input: {json.dumps(value["input"])}, global_key: {json.dumps(value["global_key"])})'


def render_input_alias(value):
    return f'(child_input: {json.dumps(value["child_input"])}, outer_input: {json.dumps(value["outer_input"])})'


def runtime_fields(config, inputs):
    values = []
    if config["mode"] == "child":
        rendered = ", ".join(render_context_input(value) for value in inputs)
        values.append(f"context_inputs: [{rendered}]")
        aliases = ", ".join(
            render_input_alias(value) for value in config.get("input_aliases", [])
        )
        values.append(f"input_aliases: [{aliases}]")
    if config["mode"] == "linked":
        selector = config.get("selector_global_key")
        if selector is not None:
            values.append(f"selector_global_key: Some({json.dumps(selector)})")
        global_inputs = ", ".join(
            render_global_data_input(value)
            for value in config.get("global_data_inputs", [])
        )
        values.append(f"global_data_inputs: [{global_inputs}]")
    if config["mode"] != "local_ref":
        values.append(f"capture_context: {'true' if config['capture_context'] else 'false'}")
        values.append(
            f"expose_context_curves: {'true' if config['expose_context_curves'] else 'false'}"
        )
        outputs = ", ".join(json.dumps(value) for value in config["exposed_output_data"])
        values.append(f"exposed_output_data: [{outputs}]")
    if config["mode"] == "named" and config.get("pose_input") is not None:
        values.append(f"pose_input: {json.dumps(config['pose_input'])}")
    values.append(f"internalize_time: {'true' if config['internalize_time'] else 'false'}")
    return ", ".join(values)


def field_replacement(entry, field_name, replacement):
    if replacement.startswith("["):
        try:
            _, start, end = field_span(entry, field_name)
        except ValueError:
            return entry, False
        return entry[:start] + replacement + entry[end + 1 :], True
    pattern = rf'(?<![A-Za-z0-9_]){re.escape(field_name)}\s*:\s*(?:true|false|None|Some\([^)]*\)|"[^"\\]*(?:\\.[^"\\]*)*")'
    replaced, count = re.subn(pattern, f"{field_name}: {replacement}", entry, count=1)
    return replaced, count == 1


def upsert_runtime_fields(entry, config, inputs):
    rendered = runtime_fields(config, inputs)
    fields = []
    if config["mode"] == "child":
        values = ", ".join(render_context_input(value) for value in inputs)
        fields.append(("context_inputs", f"[{values}]"))
        values = ", ".join(
            render_input_alias(value) for value in config.get("input_aliases", [])
        )
        fields.append(("input_aliases", f"[{values}]"))
    if config["mode"] == "linked" and config.get("selector_global_key") is not None:
        fields.append(("selector_global_key", f"Some({json.dumps(config['selector_global_key'])})"))
    if config["mode"] == "linked":
        values = ", ".join(
            render_global_data_input(value)
            for value in config.get("global_data_inputs", [])
        )
        fields.append(("global_data_inputs", f"[{values}]"))
    if config["mode"] == "named" and config.get("pose_input") is not None:
        fields.append(("pose_input", json.dumps(config["pose_input"])))
    if config["mode"] != "local_ref":
        fields.extend(
            [
                ("capture_context", "true" if config["capture_context"] else "false"),
                (
                    "expose_context_curves",
                    "true" if config["expose_context_curves"] else "false",
                ),
                (
                    "exposed_output_data",
                    "[" + ", ".join(json.dumps(value) for value in config["exposed_output_data"]) + "]",
                ),
            ]
        )
    fields.append(("internalize_time", "true" if config["internalize_time"] else "false"))
    missing = []
    for field_name, value in fields:
        entry, replaced = field_replacement(entry, field_name, value)
        if not replaced:
            missing.append(f"{field_name}: {value}")
    if not missing:
        return entry
    marker = ") }, should_debug:"
    position = entry.rfind(marker)
    if position < 0:
        raise ValueError(f"node inner data boundary is missing: {config['node']}")
    prefix = entry[:position]
    separator = "" if prefix.rstrip().endswith("(") else ", "
    return prefix + separator + ", ".join(missing) + entry[position:]


def inject_node_fields(entry, config, inputs):
    if config["mode"] == "child" and "LayeredGraphNode" not in entry:
        raise ValueError(f"child context mode has an unsupported node: {config['node']}")
    if config["mode"] == "named" and not ("InertializationPassthroughNode" in entry or "ControlRigBoundaryNode" in entry):
        raise ValueError(f"named context mode has an unsupported node: {config['node']}")
    if config["mode"] == "linked" and "InertializedLinkedAnimLayerNode" not in entry:
        raise ValueError(f"linked context mode has an unsupported node: {config['node']}")
    if config["mode"] == "local_ref" and "LocalRefPoseNode" not in entry:
        raise ValueError(f"local reference context mode has an unsupported node: {config['node']}")
    return upsert_runtime_fields(entry, config, inputs)


def removable_curve_edge(edge, manifest):
    target = edge["target"]
    source = edge["source"]
    if target["kind"] != "node_data":
        return False
    configs = {value["node"]: value for value in manifest["runtime_nodes"]}
    for context in manifest["contexts"]:
        if target["node"] != context["target"]["node"]:
            continue
        config = configs.get(target["node"])
        if (
            target["pin"] == context["target"]["pose"]
            or config is not None
            and outer_input_name(config, target["pin"]) == context["target"]["pose"]
        ):
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


def output_data_name(edge):
    match = re.fullmatch(r'OutputData\("([^"\\]*(?:\\.[^"\\]*)*)"\)', edge["target_text"])
    return None if match is None else json.loads(f'"{match.group(1)}"')


def removable_root_output_edge(edge, manifest):
    name = output_data_name(edge)
    return name is not None and name not in set(manifest.get("root_output_data", []))


def internalized_nodes(manifest):
    return {
        value["node"]
        for value in manifest["runtime_nodes"]
        if value.get("internalize_time")
    }


def removable_time_edge(edge, manifest):
    nodes = internalized_nodes(manifest)
    target = edge["target"]
    source = edge["source"]
    if target["kind"] == "node_time" and target["node"] in nodes:
        return True
    return edge["target_text"] == "OutputTime" and source["kind"] == "node_time" and source["node"] in nodes


def map_entries(text, field_name):
    _, start, end = field_span(text, field_name)
    return split_top_level(text[start + 1 : end], ",")


def replace_output_time(text, value):
    pattern = r"(?<![A-Za-z0-9_])output_time\s*:\s*(?:true|false)"
    updated, count = re.subn(pattern, f"output_time: {'true' if value else 'false'}", text, count=1)
    if count != 1:
        raise ValueError("output_time replacement failed")
    return updated


def remove_root_io_noise(text, manifest):
    retained = manifest.get("root_output_data", [])
    text = render_collection(text, "input_times", [])
    text = render_map(text, "input_order", [])
    output_data = []
    for entry in map_entries(text, "output_data"):
        key, value = top_level_colon(entry)
        if json.loads(key.strip()) in retained:
            output_data.append(f"{key.strip()}: {value}")
    text = render_map(text, "output_data", output_data)
    output_order = [f'Data({json.dumps(name)}): {index}' for index, name in enumerate(retained)]
    text = render_map(text, "output_order", output_order)
    return replace_output_time(text, False)


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


def outer_input_name(config, pin):
    for alias in config.get("input_aliases", []):
        if alias["child_input"] == pin:
            return alias["outer_input"]
    if config.get("pose_input") is not None and pin == "source_pose":
        return config["pose_input"]
    return pin


def rendered_target(edge, configs):
    target = edge["target"]
    if target["kind"] != "node_data":
        return edge["target_text"]
    config = configs.get(target["node"])
    if config is None:
        return edge["target_text"]
    pin = outer_input_name(config, target["pin"])
    return f'NodeData("{target["node"]}", {json.dumps(pin)})'


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
        if removable_time_edge(edge, manifest):
            removed_edges.append(edge)
            continue
        if removable_root_output_edge(edge, manifest):
            removed_edges.append(edge)
            continue
        if (
            edge["source"]["kind"] == "node_data"
            and edge["source"]["node"] in removed_nodes
        ):
            removed_edges.append(edge)
            continue
        retained_edges.append(f"{rendered_target(edge, configs)}: {edge['source_text']}")
    text = render_map(text, "edges_inverted", retained_edges)
    text = remove_node_positions(text, removed_nodes)
    text = remove_root_io_noise(text, manifest)
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
        if config.get("capture_context") and "capture_context: true" not in entry:
            failures.append(f"runtime node does not capture context {config['node']}")
        if config.get("internalize_time") and "internalize_time: true" not in entry:
            failures.append(f"runtime node does not internalize time {config['node']}")
    for removed_id in manifest.get("remove_nodes", []):
        if removed_id in by_id:
            failures.append(f"removed node remains {removed_id}")
    edges, _ = parse_edges(text)
    residual = [edge for edge in edges if removable_curve_edge(edge, manifest)]
    if residual:
        failures.append(f"residual context curve edges {len(residual)}")
    residual_time = [edge for edge in edges if removable_time_edge(edge, manifest)]
    if residual_time:
        failures.append(f"residual context time edges {len(residual_time)}")
    residual_outputs = [edge for edge in edges if removable_root_output_edge(edge, manifest)]
    if residual_outputs:
        failures.append(f"residual root output edges {len(residual_outputs)}")
    return {
        "nodes": len(by_id),
        "edges": len(edges),
        "residual_context_curve_edges": len(residual),
        "residual_context_time_edges": len(residual_time),
        "residual_root_output_edges": len(residual_outputs),
        "failures": failures,
    }


def run_self_test():
    manifest = {
        "contexts": [
            {
                "source": {"node": "source", "pose": "pose"},
                "target": {"node": "target", "pose": "Outer Input"},
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
                "internalize_time": True,
            },
            {
                "node": "target",
                "mode": "child",
                "input_aliases": [
                    {"child_input": "Input", "outer_input": "Outer Input"}
                ],
                "capture_context": True,
                "expose_context_curves": False,
                "exposed_output_data": [],
                "internalize_time": True,
            },
            {
                "node": "reference",
                "mode": "local_ref",
                "internalize_time": True,
            },
        ],
        "remove_nodes": ["zero"],
        "root_output_data": ["pose"],
    }
    text = (
        "(\n"
        "nodes: ["
        "(id: \"source\", name: \"Source\", inner: { \"als_ext::LayeredGraphNode\": (graph: \"graphs/source.animgraph.ron\") }, should_debug: false),"
        "(id: \"target\", name: \"Target\", inner: { \"als_ext::LayeredGraphNode\": (graph: \"graphs/target.animgraph.ron\") }, should_debug: false),"
        "(id: \"reference\", name: \"Reference\", inner: { \"als_ext::LocalRefPoseNode\": () }, should_debug: false),"
        "(id: \"zero\", name: \"Zero\", inner: { \"als_ext::ConstantF32Node\": (value: 0.0) }, should_debug: false),"
        "],\n"
        "edges_inverted: {"
        "NodeData(\"target\", \"Input\"): NodeData(\"source\", \"pose\"),"
        "NodeTime(\"target\", \"input_time\"): NodeTime(\"source\"),"
        "NodeData(\"target\", \"ctx_Curve\"): NodeData(\"source\", \"Curve\"),"
        "NodeData(\"target\", \"ctx_Default\"): NodeData(\"zero\", \"value\"),"
        "OutputData(\"Curve\"): NodeData(\"source\", \"Curve\"),"
        "},\n"
        "io_spec: (input_times: [Passthrough(\"noise\")], input_data: {}, output_time: true, output_data: { \"pose\": Pose, \"Curve\": F32 }, input_order: { Time(Passthrough(\"noise\")): 0 }, output_order: { Data(\"pose\"): 0, Data(\"Curve\"): 1, Time: 2 }),\n"
        "default_data: {},\n"
        "editor_metadata: (node_positions: { \"source\": (0.0, 0.0), \"target\": (100.0, 0.0), \"zero\": (50.0, 100.0) }, input_position: (-64.0, 0.0), output_position: (200.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n"
        ")\n"
    )
    transformed, report = transform(text, manifest)
    assert report == {"removed_edges": 4, "retained_edges": 1}
    checked = audit(transformed, manifest)
    assert not checked["failures"]
    assert "ctx_Curve" not in transformed
    assert "ctx_Default" not in transformed
    assert 'NodeData("target", "Outer Input")' in transformed
    assert "capture_context: true" in transformed
    assert "internalize_time: true" in transformed
    assert '"als_ext::LocalRefPoseNode": (internalize_time: true)' in transformed
    assert "OutputData(\"Curve\")" not in transformed
    assert "input_times: []" in transformed
    assert "output_time: false" in transformed
    rerun, rerun_report = transform(transformed, manifest)
    assert rerun == transformed
    assert rerun_report == {"removed_edges": 0, "retained_edges": 1}
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
