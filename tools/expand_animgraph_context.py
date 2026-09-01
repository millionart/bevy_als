import argparse
import json
import re
import tempfile
from pathlib import Path

from animgraph_context_contract import (
    BASELINE_FORMAT,
    MANIFEST_FORMAT,
    compact,
    context_shape_issues,
    data_pin,
    node_channels,
    parse_edges,
    parse_nodes,
    parse_pin,
    pin_key,
    pin_text,
    read_json,
    read_text,
    repo_path,
    relative_path,
    time_pin,
)
from normalize_animgraph_io_order import field_span


def newline_for(text):
    return "\r\n" if "\r\n" in text else "\n"


def edge_text(target, source):
    return f"{pin_text(target)}: {pin_text(source)}"


def context_edges(manifest, graph_text, asset_root):
    issues = context_shape_issues(manifest)
    if issues:
        raise ValueError("; ".join(issues))
    nodes, _ = parse_nodes(graph_text)
    cache = {}
    result = []
    targets = {}
    for context_index, context in enumerate(manifest["contexts"]):
        context_id = context["id"]
        source_id = context["source"]["node"]
        target_id = context["target"]["node"]
        source_node = nodes.get(source_id)
        target_node = nodes.get(target_id)
        if source_node is None or target_node is None:
            raise ValueError(f"context node is missing: {context_id}")
        planned = [
            {
                "context": context_id,
                "kind": "pose",
                "target": data_pin(target_id, context["target"]["pose"]),
                "source": data_pin(source_id, context["source"]["pose"]),
                "order": (context_index, 0, ""),
            },
            {
                "context": context_id,
                "kind": "time",
                "target": time_pin(target_id, context["time"]["target"]),
                "source": time_pin(source_id),
                "order": (context_index, 1, ""),
            },
        ]
        prefix = context["curves"]["target_prefix"]
        source_channels = node_channels(source_node, asset_root, "output", "", cache)
        target_channels = node_channels(target_node, asset_root, "input", prefix, cache)
        for channel in sorted(source_channels):
            if f"{prefix}{channel}" not in target_channels:
                continue
            planned.append(
                {
                    "context": context_id,
                    "kind": "curve",
                    "target": data_pin(target_id, f"{prefix}{channel}"),
                    "source": data_pin(source_id, channel),
                    "order": (context_index, 2, channel),
                }
            )
        for edge in planned:
            key = pin_key(edge["target"])
            if key in targets:
                raise ValueError(
                    f"context target is shared by {targets[key]['context']} and {context_id}"
                )
            targets[key] = edge
            result.append(edge)
    return result


def expected_edge_map(edges):
    return {pin_key(edge["target"]): edge for edge in edges}


def validate_complete_graph(graph_text, expected):
    _, targets = parse_edges(graph_text)
    issues = []
    for edge in expected:
        actual = targets.get(pin_key(edge["target"]))
        if actual is None:
            issues.append(f"missing {edge['context']} {edge['kind']} edge")
            continue
        if pin_key(actual["source"]) != pin_key(edge["source"]):
            issues.append(f"mismatched {edge['context']} {edge['kind']} edge")
    return issues


def replace_edges(text, rendered):
    _, start, end = field_span(text, "edges_inverted")
    line_start = text.rfind("\n", 0, start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    newline = newline_for(text)
    if rendered:
        body = newline.join(f"{inner_indent}{entry}," for entry in rendered)
        replacement = f"{{{newline}{body}{newline}{base_indent}}}"
    else:
        replacement = "{}"
    return text[:start] + replacement + text[end + 1 :]


def make_template(graph_text, expected):
    issues = validate_complete_graph(graph_text, expected)
    if issues:
        raise ValueError("; ".join(issues))
    expected_targets = set(expected_edge_map(expected))
    edges, _ = parse_edges(graph_text)
    rendered = [
        f"{edge['target_text']}: {edge['source_text']}"
        for edge in edges
        if pin_key(edge["target"]) not in expected_targets
    ]
    return replace_edges(graph_text, rendered)


def baseline_order(baseline):
    if baseline is None:
        return []
    if baseline.get("format") != BASELINE_FORMAT:
        raise ValueError("baseline format is invalid")
    return [pin_key(parse_pin(edge["target"])) for edge in baseline["edges"]]


def expand_template(template_text, expected, baseline=None):
    existing, _ = parse_edges(template_text)
    expected_by_target = expected_edge_map(expected)
    existing_by_target = {pin_key(edge["target"]): edge for edge in existing}
    conflicts = sorted(set(existing_by_target) & set(expected_by_target))
    if conflicts:
        raise ValueError("template contains expanded context edges")
    rendered_by_target = {
        pin_key(edge["target"]): f"{edge['target_text']}: {edge['source_text']}"
        for edge in existing
    }
    rendered_by_target.update(
        {
            pin_key(edge["target"]): edge_text(edge["target"], edge["source"])
            for edge in expected
        }
    )
    ordered = []
    seen = set()
    for key in baseline_order(baseline):
        if key in rendered_by_target:
            ordered.append(rendered_by_target[key])
            seen.add(key)
    remaining_existing = [
        pin_key(edge["target"])
        for edge in existing
        if pin_key(edge["target"]) not in seen
    ]
    remaining_expected = [
        pin_key(edge["target"])
        for edge in sorted(expected, key=lambda edge: edge["order"])
        if pin_key(edge["target"]) not in seen
    ]
    for key in remaining_existing + remaining_expected:
        if key not in seen:
            ordered.append(rendered_by_target[key])
            seen.add(key)
    return replace_edges(template_text, ordered)


def graph_view(text):
    _, start, end = field_span(text, "edges_inverted")
    edges, _ = parse_edges(text)
    return {
        "body": compact(text[:start] + "{}" + text[end + 1 :]),
        "edges": [
            (compact(edge["target_text"]), compact(edge["source_text"]))
            for edge in edges
        ],
    }


def audit_graphs(reference_text, actual_text):
    reference = graph_view(reference_text)
    actual = graph_view(actual_text)
    differences = []
    if reference["body"] != actual["body"]:
        differences.append("non_edge_graph_content")
    if reference["edges"] != actual["edges"]:
        differences.append("edge_order_or_content")
    return {
        "reference_edges": len(reference["edges"]),
        "actual_edges": len(actual["edges"]),
        "differences": differences,
    }


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


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def fixture_manifest(source_node, target_node):
    return {
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


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        asset_root = root / "assets"
        graphs = asset_root / "graphs"
        graphs.mkdir(parents=True)
        source_path = graphs / "source.animgraph.ron"
        target_path = graphs / "target.animgraph.ron"
        source_path.write_text(
            fixture_graph('"pose": Pose, "Existing": F32', "", "", ""),
            encoding="utf-8",
        )
        target_path.write_text(
            fixture_graph(
                '"pose": Pose',
                'Passthrough("Input"): Pose, Passthrough("ctx_Existing"): F32',
                "",
                "",
            ),
            encoding="utf-8",
        )
        source_node = "source-node"
        target_node = "target-node"
        nodes = (
            f'(id: "{source_node}", name: "Source", inner: {{ "als_ext::LayeredGraphNode": (graph: "graphs/source.animgraph.ron") }}, should_debug: false), '
            f'(id: "{target_node}", name: "Target", inner: {{ "als_ext::LayeredGraphNode": (graph: "graphs/target.animgraph.ron") }}, should_debug: false)'
        )
        full_edges = (
            f'NodeData("{target_node}", "Input"): NodeData("{source_node}", "pose"), '
            f'NodeTime("{target_node}", "input_time"): NodeTime("{source_node}"), '
            f'NodeData("{target_node}", "ctx_Existing"): NodeData("{source_node}", "Existing")'
        )
        full = fixture_graph('"pose": Pose', "", nodes, full_edges)
        root_path = graphs / "root.animgraph.ron"
        root_path.write_text(full, encoding="utf-8")
        manifest = fixture_manifest(source_node, target_node)
        manifest_path = root / "manifest.json"
        write_json(manifest_path, manifest)
        expected = context_edges(manifest, full, asset_root)
        template = make_template(full, expected)
        baseline = {
            "format": BASELINE_FORMAT,
            "edges": [
                {"target": edge["target_text"], "source": edge["source_text"]}
                for edge in parse_edges(full)[0]
            ],
        }
        expanded = expand_template(template, expected, baseline)
        assert not audit_graphs(full, expanded)["differences"]
        assert expand_template(template, expected, baseline) == expanded
        source_path.write_text(
            fixture_graph('"pose": Pose, "Existing": F32, "Future": F32', "", "", ""),
            encoding="utf-8",
        )
        target_path.write_text(
            fixture_graph(
                '"pose": Pose',
                'Passthrough("Input"): Pose, Passthrough("ctx_Existing"): F32, Passthrough("ctx_Future"): F32',
                "",
                "",
            ),
            encoding="utf-8",
        )
        future_expected = context_edges(manifest, template, asset_root)
        future_expanded = expand_template(template, future_expected, baseline)
        assert f'NodeData("{target_node}", "ctx_Future"): NodeData("{source_node}", "Future")' in future_expanded
        try:
            expand_template(full, expected, baseline)
        except ValueError:
            pass
        else:
            raise AssertionError("expanded graph was accepted as a template")
    return {
        "template_round_trip": "PASS",
        "template_expansion_idempotent": "PASS",
        "automatic_new_curve_edge": "PASS",
        "expanded_template_rejected": "PASS",
    }


def manifest_and_asset_root(root, manifest_path):
    manifest = read_json(manifest_path)
    issues = context_shape_issues(manifest)
    if issues:
        raise ValueError("; ".join(issues))
    return manifest, repo_path(root, manifest["asset_root"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--template", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--make-template", action="store_true")
    parser.add_argument("--expand", action="store_true")
    parser.add_argument("--audit", action="store_true")
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--actual", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    root = args.root.resolve()
    if args.audit:
        if args.reference is None or args.actual is None:
            raise ValueError("reference and actual are required for audit")
        reference_path = repo_path(root, args.reference)
        actual_path = repo_path(root, args.actual)
        report = audit_graphs(read_text(reference_path), read_text(actual_path))
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if not report["differences"] else 1
    if args.manifest is None:
        raise ValueError("manifest is required")
    manifest_path = repo_path(root, args.manifest)
    manifest, asset_root = manifest_and_asset_root(root, manifest_path)
    if args.make_template:
        if args.template is None:
            raise ValueError("template is required when making a template")
        source_path = repo_path(root, args.source) if args.source else repo_path(asset_root, manifest["graph"])
        source_text = read_text(source_path)
        expected = context_edges(manifest, source_text, asset_root)
        template_text = make_template(source_text, expected)
        template_path = repo_path(root, args.template)
        template_path.write_text(template_text, encoding="utf-8")
        print(json.dumps({"template": relative_path(template_path, root), "contexts": len(manifest["contexts"]), "managed_edges": len(expected)}, ensure_ascii=False, indent=2))
        return 0
    if args.expand:
        if args.template is None or args.output is None:
            raise ValueError("template and output are required for expansion")
        template_path = repo_path(root, args.template)
        output_path = repo_path(root, args.output)
        template_text = read_text(template_path)
        expected = context_edges(manifest, template_text, asset_root)
        baseline = None
        if args.baseline is not None:
            baseline = read_json(repo_path(root, args.baseline))
        expanded = expand_template(template_text, expected, baseline)
        output_path.write_text(expanded, encoding="utf-8")
        print(json.dumps({"output": relative_path(output_path, root), "contexts": len(manifest["contexts"]), "managed_edges": len(expected)}, ensure_ascii=False, indent=2))
        return 0
    raise ValueError("select make-template, expand, audit, or self-test")


if __name__ == "__main__":
    raise SystemExit(main())
