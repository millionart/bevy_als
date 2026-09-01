import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from apply_source_layout import (
    all_source_nodes,
    place_nodes,
    source_nodes,
    source_owner,
    target_edges,
    target_nodes,
)


def source_kind(path):
    generated_markers = (
        "ExecuteUbergraph",
        "EvaluateGraphExposedInputs",
        "_MERGED",
        "__AnimFunc",
        "AnimNode_CallFunction_EventStub",
        "AnimNotify_",
    )
    return "compiler_generated" if any(marker in path for marker in generated_markers) else "authored"


def pin_count(nodes):
    return sum(len(node.get("pins", [])) for node in nodes)


def source_path(mapping, entry):
    return f'{mapping["source_asset"]}:{entry["source_suffix"]}'


def best_target_scope(graph_path, mapping, graph_entries):
    candidates = [
        entry
        for entry in graph_entries
        if graph_path.startswith(source_path(mapping, entry))
    ]
    if candidates:
        return max(candidates, key=lambda entry: len(source_path(mapping, entry)))
    root_entries = [entry for entry in graph_entries if entry["source_suffix"] == "AnimGraph"]
    return root_entries[0] if root_entries else None


def payload(source_payload, target_root, mapping):
    source_graphs = {entry["object_path"]: entry for entry in source_payload["graphs"]}
    source_by_path = all_source_nodes(source_graphs)
    target_rows = []
    graph_rows = []
    missing_source_graphs = []
    missing_source_anchors = []
    for entry in mapping["graphs"]:
        source_graph_path = source_path(mapping, entry)
        source_graph = source_graphs.get(source_graph_path)
        if source_graph is None:
            missing_source_graphs.append(source_graph_path)
            continue
        if entry.get("graph_scope_only"):
            graph_rows.append(
                {
                    "target": entry["target"],
                    "source": source_graph_path,
                    "target_nodes": 0,
                    "source_nodes": len(source_nodes(source_graph)),
                    "source_pins": pin_count(source_nodes(source_graph)),
                    "graph_scope_only": True,
                }
            )
            continue
        target_path = target_root / entry["target"]
        text = target_path.read_text(encoding="utf-8")
        nodes = target_nodes(text)
        _, node_rows = place_nodes(
            nodes,
            target_edges(text),
            source_nodes(source_graph),
            source_by_path,
            source_owner(source_graph, source_by_path),
            entry.get("node_overrides", []),
        )
        for row in node_rows:
            target_rows.append(
                {
                    "target": entry["target"],
                    "source_scope": source_graph_path,
                    **row,
                }
            )
            if row["source_node"] is None:
                missing_source_anchors.append(f'{entry["target"]}:{row["id"]}')
        graph_rows.append(
            {
                "target": entry["target"],
                "source": source_graph_path,
                "target_nodes": len(nodes),
                "source_nodes": len(source_nodes(source_graph)),
                "source_pins": pin_count(source_nodes(source_graph)),
            }
        )
    anchors = {}
    for row in target_rows:
        for source_node in row.get("source_nodes", [row["source_node"]]):
            anchors.setdefault(source_node, []).append(
                {"target": row["target"], "id": row["id"], "name": row["name"]}
            )
    source_rows = []
    unexplained_authored_graphs = []
    for graph in source_payload["graphs"]:
        path = graph["object_path"]
        kind = source_kind(path)
        scope = best_target_scope(path, mapping, mapping["graphs"])
        represented = scope is not None
        if kind == "authored" and not represented:
            unexplained_authored_graphs.append(path)
        for node in graph["nodes"]:
            if "EdGraphNode_Comment" in node.get("class_path", ""):
                continue
            source_rows.append(
                {
                    "source_graph": path,
                    "source_graph_kind": kind,
                    "object_path": node["object_path"],
                    "class_path": node["class_path"],
                    "title": node["title"],
                    "position": [node["position_x"], node["position_y"]],
                    "pin_count": len(node.get("pins", [])),
                    "target_scope": None if scope is None else scope["target"],
                    "anchored_targets": anchors.get(node["object_path"], []),
                    "resolution": "compiler_generated"
                    if kind == "compiler_generated"
                    else "node_anchor"
                    if node["object_path"] in anchors
                    else "graph_scope_equivalent"
                    if represented
                    else "unexplained",
                }
            )
    unexplained_source_nodes = [
        row["object_path"] for row in source_rows if row["resolution"] == "unexplained"
    ]
    source_graph_summary = []
    for graph in source_payload["graphs"]:
        path = graph["object_path"]
        scope = best_target_scope(path, mapping, mapping["graphs"])
        source_graph_summary.append(
            {
                "object_path": path,
                "kind": source_kind(path),
                "nodes": len(graph["nodes"]),
                "pins": pin_count(graph["nodes"]),
                "target_scope": None if scope is None else scope["target"],
            }
        )
    return {
        "format": "als_source_target_graph_coverage_v1",
        "source_asset": source_payload["asset_path"],
        "target_root": target_root.as_posix(),
        "source_graphs": source_graph_summary,
        "mapped_graphs": graph_rows,
        "target_nodes": target_rows,
        "source_nodes": source_rows,
        "missing_source_graphs": missing_source_graphs,
        "missing_source_anchors": missing_source_anchors,
        "unexplained_authored_graphs": unexplained_authored_graphs,
        "unexplained_source_nodes": unexplained_source_nodes,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--target-root", required=True, type=Path)
    parser.add_argument("--mapping", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.output is None and not args.check:
        raise ValueError("output or --check is required")
    if args.output is not None and args.check:
        raise ValueError("output and --check are mutually exclusive")
    source_payload = json.loads(args.source.read_text(encoding="utf-8"))
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    report = payload(source_payload, args.target_root, mapping)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    result = {
        "mapped_graphs": len(report["mapped_graphs"]),
        "target_nodes": len(report["target_nodes"]),
        "source_nodes": len(report["source_nodes"]),
        "missing_source_graphs": len(report["missing_source_graphs"]),
        "missing_source_anchors": len(report["missing_source_anchors"]),
        "unexplained_authored_graphs": len(report["unexplained_authored_graphs"]),
        "unexplained_source_nodes": len(report["unexplained_source_nodes"]),
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0 if not any(
        result[key]
        for key in (
            "missing_source_graphs",
            "missing_source_anchors",
            "unexplained_authored_graphs",
            "unexplained_source_nodes",
        )
    ) else 1


if __name__ == "__main__":
    raise SystemExit(main())
