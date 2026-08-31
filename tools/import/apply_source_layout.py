import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT.parent))

from capture_animgraph_ron_topology import collection_text, entries, parse_positions


def source_text(node):
    return "\n".join(
        entry.get("exported_text", "") for entry in node.get("properties", [])
    )


def source_nodes(source_graph):
    return [
        node
        for node in source_graph.get("nodes", [])
        if "EdGraphNode_Comment" not in node.get("class_path", "")
    ]


def all_source_nodes(source_graphs):
    result = {}
    for source_graph in source_graphs.values():
        for node in source_nodes(source_graph):
            result[node["object_path"]] = node
    return result


def source_owner(source_graph, source_by_path):
    graph_path = source_graph["object_path"]
    candidates = [
        node
        for path, node in source_by_path.items()
        if graph_path.startswith(f"{path}.")
    ]
    if candidates:
        return max(candidates, key=lambda node: len(node["object_path"]))
    roots = [
        node
        for node in source_nodes(source_graph)
        if "AnimGraphNode_Root" in node.get("class_path", "")
    ]
    if roots:
        return roots[0]
    nodes = source_nodes(source_graph)
    return nodes[0] if nodes else None


def target_nodes(text):
    _, _, _, values = entries(text, "nodes")
    result = []
    for index, value in enumerate(values):
        identifier = re.search(r'\bid:\s*"([^"]+)"', value)
        name = re.search(r'\bname:\s*"([^"]+)"', value)
        if identifier is None or name is None:
            raise ValueError("node entry is missing id or name")
        result.append(
            {
                "id": identifier.group(1),
                "name": name.group(1),
                "entry": value,
                "index": index,
            }
        )
    return result


def target_edges(text):
    _, _, _, value = collection_text(text, "edges_inverted")
    pattern = re.compile(
        r'Node(?:Data|Time)\("([^"]+)"[^)]*\):\s*Node(?:Data|Time)\("([^"]+)"'
    )
    return [(source, target) for target, source in pattern.findall(value)]


def node_asset_leaf(node):
    match = re.search(r'clip:\s*"[^"]*/([^/"\\]+)\.anim\.ron"', node["entry"])
    return match.group(1) if match is not None else None


def connected_asset_hints(nodes, edges):
    node_by_id = {node["id"]: node for node in nodes}
    adjacency = {node_id: set() for node_id in node_by_id}
    for source, target in edges:
        if source in adjacency and target in adjacency:
            adjacency[source].add(target)
            adjacency[target].add(source)
    clip_assets = {
        node_id: node_asset_leaf(node)
        for node_id, node in node_by_id.items()
        if node_asset_leaf(node) is not None
    }
    result = {}
    for node_id in node_by_id:
        direct = clip_assets.get(node_id)
        if direct is not None:
            result[node_id] = direct
            continue
        frontier = [node_id]
        distances = {node_id: 0}
        nearest = []
        while frontier and not nearest:
            current = frontier.pop(0)
            for adjacent in sorted(adjacency[current]):
                if adjacent in distances:
                    continue
                distances[adjacent] = distances[current] + 1
                asset = clip_assets.get(adjacent)
                if asset is not None:
                    nearest.append(asset)
                frontier.append(adjacent)
        unique = sorted(set(nearest))
        if len(unique) == 1:
            result[node_id] = unique[0]
    return result


def normalized(value):
    return re.sub(r"[^a-z0-9]", "", value.lower())


def source_anchor(node, source, source_by_path, owner, connected_asset):
    leaf = node_asset_leaf(node) or connected_asset
    if leaf:
        candidates = [source_node for source_node in source if leaf in source_text(source_node)]
        if len(candidates) == 1:
            return candidates[0], "asset" if node_asset_leaf(node) else "connected_asset"
    target_name = normalized(node["name"])
    candidates = [
        source_node
        for source_node in source
        if target_name and target_name in normalized(source_node.get("title", ""))
    ]
    if len(candidates) == 1:
        return candidates[0], "title"
    if "blendspace" in target_name:
        candidates = [
            source_node
            for source_node in source
            if "BlendSpace" in source_node.get("class_path", "")
        ]
        if len(candidates) == 1:
            return candidates[0], "blendspace"
    if source:
        return None, "source_graph_projection"
    if owner is not None:
        return owner, "source_owner_expansion"
    if source_by_path:
        return next(iter(source_by_path.values())), "asset_owner_expansion"
    return None, "missing_source_anchor"


def ranks(nodes, edges):
    ids = {node["id"] for node in nodes}
    result = {node_id: 0 for node_id in ids}
    for _ in range(len(ids)):
        changed = False
        for source, target in edges:
            if source not in ids or target not in ids:
                continue
            value = result[source] + 1
            if value > result[target]:
                result[target] = value
                changed = True
        if not changed:
            break
    return result


def source_bounds(source):
    if not source:
        return (-960.0, 0.0, -480.0, 480.0)
    xs = [float(node.get("position_x", 0.0)) for node in source]
    ys = [float(node.get("position_y", 0.0)) for node in source]
    min_x = min(xs)
    max_x = max(xs)
    min_y = min(ys)
    max_y = max(ys)
    if max_x - min_x < 240.0:
        max_x = min_x + 960.0
    if max_y - min_y < 240.0:
        max_y = min_y + 480.0
    return (min_x, max_x, min_y, max_y)


def anchor_position(node, anchor, strategy):
    x = float(anchor.get("position_x", 0.0))
    y = float(anchor.get("position_y", 0.0))
    entry = node["entry"]
    if "TimePositionNode" in entry:
        return x - 256.0, y
    if "ConstantF32Node" in entry:
        return x - 256.0, y + 96.0
    if "LoopNode" in entry:
        return x + 256.0, y
    if "ClipNode" in entry:
        return x, y
    if strategy in {"source_owner_expansion", "asset_owner_expansion"}:
        return x, y
    return x, y


def projection_anchor(nodes, node, rank, max_rank, source):
    min_x, max_x, min_y, max_y = source_bounds(source)
    desired_x = min_x + (max_x - min_x) * (rank[node["id"]] + 1) / (max_rank + 2)
    desired_y = min_y + (max_y - min_y) * (node["index"] + 1) / (len(nodes) + 1)
    width = max(max_x - min_x, 1.0)
    height = max(max_y - min_y, 1.0)
    return min(
        source,
        key=lambda source_node: (
            ((float(source_node.get("position_x", 0.0)) - desired_x) / width) ** 2
            + ((float(source_node.get("position_y", 0.0)) - desired_y) / height) ** 2,
            source_node["object_path"],
        ),
    )


def override_lookup(overrides):
    result = {}
    for override in overrides:
        identifier = override.get("target_id")
        if not identifier:
            raise ValueError("node override is missing target_id")
        if identifier in result:
            raise ValueError(f"duplicate node override: {identifier}")
        result[identifier] = override
    return result


def override_sources(override, source):
    indices = override.get("source_indices")
    if indices is None:
        indices = [override.get("source_index")]
    if not indices or any(index is None for index in indices):
        raise ValueError("node override is missing source_index or source_indices")
    by_index = {node.get("index"): node for node in source}
    result = []
    for index in indices:
        node = by_index.get(index)
        if node is None:
            raise ValueError(f"node override source index is missing: {index}")
        result.append(node)
    return result


def place_nodes(nodes, edges, source, source_by_path, owner, overrides):
    rank = ranks(nodes, edges)
    max_rank = max(rank.values(), default=0)
    min_x, max_x, min_y, max_y = source_bounds(source)
    asset_hints = connected_asset_hints(nodes, edges)
    override_by_id = override_lookup(overrides)
    positions = {}
    report = []
    occupied = set()
    for node in sorted(nodes, key=lambda value: (rank[value["id"]], value["index"], value["id"])):
        override = override_by_id.get(node["id"])
        override_nodes = []
        if override is not None:
            override_nodes = override_sources(override, source)
            anchor = override_nodes[0]
            strategy = override.get("strategy", "source_override")
        else:
            anchor, strategy = source_anchor(
                node,
                source,
                source_by_path,
                owner,
                asset_hints.get(node["id"]),
            )
            if anchor is None and strategy == "source_graph_projection":
                anchor = projection_anchor(nodes, node, rank, max_rank, source)
        if anchor is not None:
            if override is not None:
                x = float(anchor.get("position_x", 0.0))
                y = float(anchor.get("position_y", 0.0))
                offset = override.get("offset", [0.0, 0.0])
                if len(offset) != 2:
                    raise ValueError("node override offset must have two values")
                x += float(offset[0])
                y += float(offset[1])
            else:
                x, y = anchor_position(node, anchor, strategy)
                if "ClipNode" in node["entry"] and strategy == "asset":
                    strategy = "asset_split_clip"
                elif "ClipNode" in node["entry"] and strategy == "connected_asset":
                    strategy = "connected_asset_split_clip"
                elif "TimePositionNode" in node["entry"] and strategy != "source_owner_expansion":
                    strategy = f"{strategy}_split_time"
                elif "ConstantF32Node" in node["entry"] and strategy != "source_owner_expansion":
                    strategy = f"{strategy}_split_constant"
                elif "LoopNode" in node["entry"] and strategy != "source_owner_expansion":
                    strategy = f"{strategy}_split_loop"
            source_ref = anchor.get("object_path")
            source_position = (
                float(anchor.get("position_x", 0.0)),
                float(anchor.get("position_y", 0.0)),
            )
            source_refs = [item.get("object_path") for item in override_nodes] or [source_ref]
            source_positions = [
                (float(item.get("position_x", 0.0)), float(item.get("position_y", 0.0)))
                for item in override_nodes
            ] or [source_position]
        else:
            x = min_x + (max_x - min_x) * (rank[node["id"]] + 1) / (max_rank + 2)
            y = min_y + (max_y - min_y) * (node["index"] + 1) / (len(nodes) + 1)
            source_ref = None
            source_position = None
            source_refs = []
            source_positions = []
        candidate = (round(x, 3), round(y, 3))
        while candidate in occupied:
            candidate = (candidate[0], round(candidate[1] + 96.0, 3))
        occupied.add(candidate)
        positions[node["id"]] = candidate
        report.append(
            {
                "id": node["id"],
                "name": node["name"],
                "position": candidate,
                "strategy": strategy,
                "source_node": source_ref,
                "source_position": source_position,
                "source_nodes": source_refs,
                "source_positions": source_positions,
                "offset": None
                if source_position is None
                else (
                    round(candidate[0] - source_position[0], 3),
                    round(candidate[1] - source_position[1], 3),
                ),
            }
        )
    return positions, report


def render_positions(text, positions):
    field_start, start, end, _ = collection_text(text, "node_positions")
    line_start = text.rfind("\n", 0, field_start) + 1
    base_indent = re.match(r"[ \t]*", text[line_start:]).group(0)
    inner_indent = f"{base_indent}    "
    newline = "\r\n" if "\r\n" in text else "\n"
    rendered = "{" + newline
    for identifier, (x, y) in positions.items():
        rendered += f'{inner_indent}"{identifier}": ({x:.3f}, {y:.3f}),{newline}'
    rendered += f"{base_indent}}}"
    return text[:start] + rendered + text[end + 1 :]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--target-root", required=True, type=Path)
    parser.add_argument("--mapping", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source_payload = json.loads(args.source.read_text(encoding="utf-8"))
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    source_graphs = {entry["object_path"]: entry for entry in source_payload["graphs"]}
    source_by_path = all_source_nodes(source_graphs)
    report = {
        "files": [],
        "missing_source_graphs": [],
        "missing_source_anchors": [],
        "position_mismatches": [],
    }
    for entry in mapping["graphs"]:
        source_path = f'{mapping["source_asset"]}:{entry["source_suffix"]}'
        source_graph = source_graphs.get(source_path)
        if source_graph is None:
            report["missing_source_graphs"].append(source_path)
            continue
        target_path = args.target_root / entry["target"]
        text = target_path.read_text(encoding="utf-8")
        nodes = target_nodes(text)
        positions, node_report = place_nodes(
            nodes,
            target_edges(text),
            source_nodes(source_graph),
            source_by_path,
            source_owner(source_graph, source_by_path),
            entry.get("node_overrides", []),
        )
        missing_anchors = [node["id"] for node in node_report if node["source_node"] is None]
        if missing_anchors:
            report["missing_source_anchors"].append(
                {"target": entry["target"], "node_ids": missing_anchors}
            )
        if args.check:
            existing = {
                item["id"]: item["position"]
                for item in parse_positions(text, "node_positions")["entries"]
            }
            mismatched = [
                identifier
                for identifier, position in positions.items()
                if existing.get(identifier) != position
            ]
            if mismatched:
                report["position_mismatches"].append(
                    {"target": entry["target"], "node_ids": mismatched}
                )
        if args.write:
            target_path.write_text(render_positions(text, positions), encoding="utf-8", newline="")
        report["files"].append(
            {
                "target": entry["target"],
                "source": source_path,
                "nodes": node_report,
            }
        )
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "files": len(report["files"]),
                "missing_source_graphs": len(report["missing_source_graphs"]),
                "missing_source_anchors": sum(
                    len(item["node_ids"]) for item in report["missing_source_anchors"]
                ),
                "position_mismatches": sum(
                    len(item["node_ids"]) for item in report["position_mismatches"]
                ),
                "positioned_nodes": sum(len(entry["nodes"]) for entry in report["files"]),
                "write": args.write,
                "check": args.check,
            },
            ensure_ascii=False,
        )
    )
    return 0 if not report["missing_source_graphs"] and not report["missing_source_anchors"] and not report["position_mismatches"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
