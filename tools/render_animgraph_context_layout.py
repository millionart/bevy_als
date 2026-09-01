import argparse
import html
import json
import re
import tempfile
from pathlib import Path

from animgraph_context_contract import (
    context_shape_issues,
    parse_edges,
    parse_nodes,
    parse_positions,
    read_json,
    read_text,
    repo_path,
    relative_path,
)


def parse_output_position(text):
    number = r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"
    match = re.search(
        rf"\boutput_position\s*:\s*\(\s*({number})\s*,\s*({number})\s*\)",
        text,
    )
    if match is None:
        raise ValueError("output position is missing")
    return float(match.group(1)), float(match.group(2))


def output_source_node(text):
    edges, _ = parse_edges(text)
    for edge in edges:
        if edge["target_text"].strip() != 'OutputData("pose")':
            continue
        source = edge["source"]
        if source["kind"] != "node_data":
            raise ValueError("output pose source is not a node pose")
        return source["node"]
    raise ValueError("output pose edge is missing")


def layout_model(manifest, graph_text):
    issues = context_shape_issues(manifest)
    if issues:
        raise ValueError("; ".join(issues))
    nodes, ordered_nodes = parse_nodes(graph_text)
    positions = parse_positions(graph_text, "node_positions")
    if positions["malformed"]:
        raise ValueError("node position metadata is malformed")
    position_by_id = {entry["id"]: tuple(entry["position"]) for entry in positions["entries"]}
    visible_ids = []
    links = []
    for context in manifest["contexts"]:
        source = context["source"]["node"]
        target = context["target"]["node"]
        if source not in nodes or target not in nodes:
            raise ValueError(f"context node is missing: {context['id']}")
        for node_id in (source, target):
            if node_id not in position_by_id:
                raise ValueError(f"context node position is missing: {node_id}")
            if node_id not in visible_ids:
                visible_ids.append(node_id)
        links.append(
            {
                "id": context["id"],
                "source": source,
                "target": target,
            }
        )
    output_id = "__output_pose__"
    source = output_source_node(graph_text)
    if source not in nodes or source not in position_by_id:
        raise ValueError("output source node is not positioned")
    if source not in visible_ids:
        visible_ids.append(source)
    links.append({"id": "output_pose", "source": source, "target": output_id})
    visible = [node for node in ordered_nodes if node["id"] in visible_ids]
    visible.append({"id": output_id, "name": "Output Pose", "index": len(visible)})
    positions = {node_id: position_by_id[node_id] for node_id in visible_ids}
    positions[output_id] = parse_output_position(graph_text)
    return {"nodes": visible, "positions": positions, "links": links}


def screen_positions(model):
    values = list(model["positions"].values())
    min_x = min(position[0] for position in values)
    max_x = max(position[0] for position in values)
    min_y = min(position[1] for position in values)
    max_y = max(position[1] for position in values)
    scale = 0.78
    margin_x = 130.0
    margin_y = 100.0
    width = int((max_x - min_x) * scale + margin_x * 2 + 180)
    height = int((max_y - min_y) * scale + margin_y * 2 + 130)
    positions = {
        node_id: (
            (position[0] - min_x) * scale + margin_x,
            (position[1] - min_y) * scale + margin_y,
        )
        for node_id, position in model["positions"].items()
    }
    return width, height, positions


def link_color(link_id):
    if "overlay" in link_id or "inertialization" in link_id:
        return "#d8a837"
    return "#c98cf1"


def edge_path(source, target):
    source_x, source_y = source
    target_x, target_y = target
    start_x = source_x + 88.0
    end_x = target_x - 88.0
    bend = max(48.0, abs(end_x - start_x) * 0.42)
    return (
        f"M {start_x:.2f} {source_y:.2f} "
        f"C {start_x + bend:.2f} {source_y:.2f}, "
        f"{end_x - bend:.2f} {target_y:.2f}, {end_x:.2f} {target_y:.2f}"
    )


def node_markup(node, position):
    x, y = position
    name = html.escape(node["name"])
    fill = "#3f3768" if node["id"] == "__output_pose__" else "#20252d"
    stroke = "#867bc9" if node["id"] == "__output_pose__" else "#6e7280"
    subtitle = "Output" if node["id"] == "__output_pose__" else "Animation Context"
    return (
        f'<g transform="translate({x - 88:.2f},{y - 33:.2f})">'
        f'<rect width="176" height="66" rx="8" fill="{fill}" stroke="{stroke}" stroke-width="2"/>'
        f'<text x="12" y="26" fill="#f3f5f7" font-size="15" font-family="Segoe UI, sans-serif">{name}</text>'
        f'<text x="12" y="48" fill="#aeb4c0" font-size="11" font-family="Segoe UI, sans-serif">{subtitle}</text>'
        "</g>"
    )


def render_svg(model):
    width, height, positions = screen_positions(model)
    edge_markup = []
    for link in model["links"]:
        source = positions[link["source"]]
        target = positions[link["target"]]
        color = link_color(link["id"])
        edge_markup.append(
            f'<path d="{edge_path(source, target)}" fill="none" stroke="#17191e" stroke-width="10" stroke-linecap="round"/>'
        )
        edge_markup.append(
            f'<path d="{edge_path(source, target)}" fill="none" stroke="{color}" stroke-width="4" stroke-linecap="round"/>'
        )
    node_markup_values = [node_markup(node, positions[node["id"]]) for node in model["nodes"]]
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">'
        "<defs><pattern id=\"grid\" width=\"20\" height=\"20\" patternUnits=\"userSpaceOnUse\"><path d=\"M 20 0 L 0 0 0 20\" fill=\"none\" stroke=\"#252a31\" stroke-width=\"1\"/></pattern></defs>"
        f'<rect width="{width}" height="{height}" fill="#15181d"/>'
        f'<rect width="{width}" height="{height}" fill="url(#grid)"/>'
        '<text x="32" y="38" fill="#d6dbe3" font-size="22" font-family="Segoe UI, sans-serif">AB_Als · Structural Animation Context</text>'
        + "".join(edge_markup)
        + "".join(node_markup_values)
        + "</svg>"
    )


def fixture_graph():
    return (
        "(\n"
        "nodes: ["
        "(id: \"a\", name: \"Source\", inner: { \"als_ext::LayeredGraphNode\": (graph: \"graphs/source.animgraph.ron\") }, should_debug: false), "
        "(id: \"b\", name: \"Target\", inner: { \"als_ext::LayeredGraphNode\": (graph: \"graphs/target.animgraph.ron\") }, should_debug: false)"
        "],\n"
        "edges_inverted: { NodeData(\"b\", \"Input\"): NodeData(\"a\", \"pose\"), OutputData(\"pose\"): NodeData(\"b\", \"pose\") },\n"
        "io_spec: (input_times: [], input_data: {}, output_time: true, output_data: { \"pose\": Pose }, input_order: {}, output_order: {}),\n"
        "default_data: {},\n"
        "editor_metadata: (node_positions: { \"a\": (0.0, 0.0), \"b\": (240.0, 0.0) }, input_position: (-128.0, 0.0), output_position: (480.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),\n"
        ")\n"
    )


def run_self_test():
    manifest = {
        "format": "als_animgraph_context_manifest_v1",
        "asset_root": "assets",
        "graph": "graphs/root.animgraph.ron",
        "attributes": "reject",
        "events": "reject",
        "contexts": [
            {
                "id": "source_to_target",
                "source": {"node": "a", "pose": "pose"},
                "target": {"node": "b", "pose": "Input"},
                "time": {"target": "time"},
                "curves": {"mode": "inherit", "target_prefix": "ctx_"},
            }
        ],
    }
    model = layout_model(manifest, fixture_graph())
    svg = render_svg(model)
    assert len(model["nodes"]) == 3
    assert len(model["links"]) == 2
    assert "Structural Animation Context" in svg
    return {"structural_layout_render": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--graph", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.manifest is None or args.graph is None or args.output is None:
        raise ValueError("manifest, graph, and output are required")
    root = args.root.resolve()
    manifest_path = repo_path(root, args.manifest)
    graph_path = repo_path(root, args.graph)
    output_path = repo_path(root, args.output)
    svg = render_svg(layout_model(read_json(manifest_path), read_text(graph_path)))
    if args.check:
        if not output_path.is_file() or read_text(output_path) != svg:
            print(json.dumps({"output": relative_path(output_path, root), "differences": ["layout_svg"]}, ensure_ascii=False, indent=2))
            return 1
    else:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(svg, encoding="utf-8")
    print(json.dumps({"output": relative_path(output_path, root), "nodes": len(layout_model(read_json(manifest_path), read_text(graph_path))["nodes"]), "links": len(layout_model(read_json(manifest_path), read_text(graph_path))["links"])}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
