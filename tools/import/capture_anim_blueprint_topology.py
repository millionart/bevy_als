import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPOSITORY_ROOT = ROOT.parent.parent
sys.path.insert(0, str(ROOT))

from uasset_base import UAsset
from uasset_reader import PropReader


def status(stage, **values):
    print(json.dumps({"stage": stage, **values}, ensure_ascii=False), flush=True)


def export_class(asset, index):
    export = asset.exports[index]
    if export.class_index < 0:
        imported = asset.imports[-export.class_index - 1]
        return imported.object_name if imported.class_name == "Class" else imported.class_name
    if export.class_index > 0:
        parent = asset.exports[export.class_index - 1]
        if parent.class_index < 0:
            imported = asset.imports[-parent.class_index - 1]
            return imported.object_name if imported.class_name == "Class" else imported.class_name
        return parent.object_name
    return ""


def outer_path(asset, index):
    parts = []
    current = index + 1
    while current > 0:
        export = asset.exports[current - 1]
        parts.append(export.object_name)
        current = export.outer_index
    return "/".join(reversed(parts))


def is_graph_node(class_name, path):
    prefixes = (
        "AnimGraphNode_",
        "AlsAnimGraphNode_",
        "AnimStateNode",
        "AnimStateTransitionNode",
        "K2Node_",
        "EdGraphNode_",
    )
    return bool(class_name) and class_name.startswith(prefixes) and "/" in path


def node_ref(blueprint_ref, path):
    parts = path.split("/", 1)
    if len(parts) != 2 or not parts[1]:
        return None
    return f"{blueprint_ref}:{parts[1].replace('/', '.')}"


def call_tool(toolset, tool_name, arguments):
    command = [
        sys.executable,
        str(REPOSITORY_ROOT / "tools" / "mcp_probe.py"),
        "call",
        toolset,
        tool_name,
        json.dumps(arguments, ensure_ascii=False, separators=(",", ":")),
    ]
    completed = subprocess.run(command, capture_output=True, text=True, encoding="utf-8")
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    if completed.returncode != 0:
        raise RuntimeError(json.dumps({"command": command[:4], "stdout": stdout, "stderr": stderr}, ensure_ascii=False))
    try:
        return json.loads(stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(json.dumps({"command": command[:4], "stdout": stdout, "stderr": stderr}, ensure_ascii=False)) from error


def read_uasset_nodes(uasset_path, blueprint_ref):
    asset = UAsset(str(uasset_path))
    reader = PropReader(asset)
    nodes = []
    for index, export in enumerate(asset.exports):
        class_name = export_class(asset, index)
        path = outer_path(asset, index)
        if not is_graph_node(class_name, path):
            continue
        properties, ok, error = reader.read_export(index)
        nodes.append(
            {
                "export_index": index,
                "class_name": class_name,
                "object_name": export.object_name,
                "outer_path": path,
                "ref_path": node_ref(blueprint_ref, path),
                "property_read_ok": ok,
                "property_read_error": error,
                "properties": properties if ok else None,
            }
        )
    return nodes


def chunks(values, size):
    for start in range(0, len(values), size):
        yield values[start : start + size]


def capture_node_info_batches(node_paths, chunk_size):
    batches = []
    failures = []
    for batch in chunks(node_paths, chunk_size):
        status("node_info_batch", requested=len(batch))
        request = {"nodes": [{"refPath": path} for path in batch]}
        try:
            response = call_tool(
                "editor_toolset.toolsets.blueprint.BlueprintTools",
                "get_node_infos",
                request,
            )
        except RuntimeError as error:
            if len(batch) == 1:
                failures.append({"ref_path": batch[0], "error": str(error)})
                continue
            midpoint = len(batch) // 2
            left_batches, left_failures = capture_node_info_batches(batch[:midpoint], chunk_size)
            right_batches, right_failures = capture_node_info_batches(batch[midpoint:], chunk_size)
            batches.extend(left_batches)
            batches.extend(right_batches)
            failures.extend(left_failures)
            failures.extend(right_failures)
            continue
        batches.append({"request": request, "response": response})
    return batches, failures


def capture_graph_coverage(blueprint_ref):
    response = call_tool(
        "editor_toolset.toolsets.blueprint.BlueprintTools",
        "list_graphs",
        {"blueprint": {"refPath": blueprint_ref}},
    )
    graph_rows = []
    graphs = response.get("returnValue", [])
    for index, graph in enumerate(graphs, start=1):
        status("graph_coverage", index=index, total=len(graphs))
        graph_ref = graph["refPath"]
        graph_response = call_tool(
            "editor_toolset.toolsets.blueprint.BlueprintTools",
            "find_nodes",
            {"graph": {"refPath": graph_ref}, "title": ""},
        )
        graph_rows.append({"graph": graph_ref, "response": graph_response})
    return response, graph_rows


def collected_node_paths(batches):
    paths = set()
    for batch in batches:
        for node in batch["response"].get("returnValue", []):
            ref_path = node.get("node", {}).get("refPath")
            if ref_path:
                paths.add(ref_path)
    return paths


def graph_node_paths(graph_rows):
    paths = set()
    for row in graph_rows:
        for node in row["response"].get("returnValue", []):
            ref_path = node.get("refPath")
            if ref_path:
                paths.add(ref_path)
    return paths


def is_comment_node_path(path):
    return ".EdGraphNode_Comment" in path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset-path", required=True)
    parser.add_argument("--uasset", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--chunk-size", type=int, default=64)
    args = parser.parse_args()
    if args.chunk_size < 1:
        raise ValueError("chunk size must be positive")
    loaded = call_tool(
        "editor_toolset.toolsets.asset.AssetTools",
        "load_asset",
        {"asset_path": args.asset_path},
    )
    blueprint_ref = loaded["returnValue"]["refPath"]
    status("asset_loaded", blueprint_ref=blueprint_ref)
    uasset_nodes = read_uasset_nodes(args.uasset, blueprint_ref)
    uasset_node_paths = {row["ref_path"] for row in uasset_nodes if row["ref_path"]}
    status("uasset_nodes", count=len(uasset_node_paths))
    graphs_response, graph_rows = capture_graph_coverage(blueprint_ref)
    graph_node_set = graph_node_paths(graph_rows)
    unexposed_uasset_node_paths = uasset_node_paths - graph_node_set
    requested_node_paths = sorted(graph_node_set)
    status("mcp_graph_nodes", count=len(requested_node_paths))
    node_info_batches, node_info_failures = capture_node_info_batches(requested_node_paths, args.chunk_size)
    requested_set = set(requested_node_paths)
    received_set = collected_node_paths(node_info_batches)
    payload = {
        "format": "als_anim_blueprint_topology_v1",
        "asset_path": args.asset_path,
        "blueprint_ref": blueprint_ref,
        "uasset_sha256": hashlib.sha256(args.uasset.read_bytes()).hexdigest(),
        "uasset_size": args.uasset.stat().st_size,
        "uasset_nodes": uasset_nodes,
        "node_info_batches": node_info_batches,
        "node_info_failures": node_info_failures,
        "graphs_response": graphs_response,
        "graph_rows": graph_rows,
        "coverage": {
            "uasset_node_paths": sorted(uasset_node_paths),
            "mcp_node_info_paths": sorted(received_set),
            "mcp_graph_node_paths": sorted(graph_node_set),
            "mcp_requested_node_paths": sorted(requested_set),
            "graph_nodes_not_returned_by_node_info": sorted(graph_node_set - received_set),
            "requested_nodes_not_returned_by_node_info": sorted(requested_set - received_set),
            "graph_nodes_not_in_uasset_capture": sorted(graph_node_set - uasset_node_paths),
            "uasset_nodes_not_in_graph_capture": sorted(uasset_node_paths - graph_node_set),
            "uasset_non_comment_nodes_without_mcp_graph_membership": sorted(
                path for path in unexposed_uasset_node_paths if not is_comment_node_path(path)
            ),
            "node_info_failures": len(node_info_failures),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2, default=str) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "output": args.output.as_posix(),
                "uasset_nodes": len(uasset_node_paths),
                "mcp_node_info_nodes": len(received_set),
                "mcp_graph_nodes": len(graph_node_set),
                "mcp_requested_nodes": len(requested_set),
                "graph_nodes_not_returned_by_node_info": len(graph_node_set - received_set),
                "requested_nodes_not_returned_by_node_info": len(requested_set - received_set),
                "graph_nodes_not_in_uasset_capture": len(graph_node_set - uasset_node_paths),
                "uasset_nodes_not_in_graph_capture": len(uasset_node_paths - graph_node_set),
                "uasset_non_comment_nodes_without_mcp_graph_membership": len(
                    [path for path in unexposed_uasset_node_paths if not is_comment_node_path(path)]
                ),
                "node_info_failures": len(node_info_failures),
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
