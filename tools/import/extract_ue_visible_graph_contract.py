import argparse
import hashlib
import json
import tempfile
from pathlib import Path


FORMAT = "als_ue_visible_graph_contract_v1"


def canonical_hash(value):
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def source_graph(source, suffix):
    asset_path = source.get("asset_path")
    if not isinstance(asset_path, str) or not asset_path:
        raise ValueError("source asset path is invalid")
    object_path = f"{asset_path}:{suffix}"
    matches = [
        graph
        for graph in source.get("graphs", [])
        if graph.get("object_path") == object_path
    ]
    if len(matches) != 1:
        raise ValueError(f"source graph is missing or ambiguous: {object_path}")
    return matches[0]


def links(graph):
    result = []
    seen = set()
    for node in graph.get("nodes", []):
        node_path = node.get("object_path")
        for pin in node.get("pins", []):
            if pin.get("direction") != "Output":
                continue
            source = {
                "node_object_path": node_path,
                "node_guid": node.get("node_guid"),
                "pin_id": pin.get("pin_id"),
                "pin_name": pin.get("name"),
                "direction": pin.get("direction"),
            }
            for target in pin.get("linked_to", []):
                if target.get("node_object_path") == "None":
                    continue
                value = {
                    "source": source,
                    "target": {
                        "node_object_path": target.get("node_object_path"),
                        "node_guid": target.get("node_guid"),
                        "pin_id": target.get("pin_id"),
                        "pin_name": target.get("pin_name"),
                        "direction": target.get("direction"),
                    },
                }
                key = canonical_hash(value)
                if key not in seen:
                    seen.add(key)
                    result.append(value)
    return sorted(result, key=canonical_hash)


def contract(source, suffix):
    graph = source_graph(source, suffix)
    graph_payload = {
        "index": graph.get("index"),
        "object_path": graph.get("object_path"),
        "class_path": graph.get("class_path"),
        "name": graph.get("name"),
        "properties": graph.get("properties"),
        "nodes": graph.get("nodes"),
    }
    return {
        "format": FORMAT,
        "source_asset": source["asset_path"],
        "source_graph_suffix": suffix,
        "source_graph": graph_payload,
        "links": links(graph_payload),
        "source_graph_sha256": canonical_hash(graph_payload),
    }


def run_self_test():
    source = {
        "asset_path": "/Asset",
        "graphs": [
            {
                "index": 0,
                "object_path": "/Asset:AnimGraph",
                "class_path": "/Script/Engine.EdGraph",
                "name": "AnimGraph",
                "properties": [],
                "nodes": [
                    {
                        "object_path": "/Asset:AnimGraph.Source",
                        "node_guid": "source",
                        "pins": [
                            {
                                "pin_id": "source-pin",
                                "name": "Pose",
                                "direction": "Output",
                                "linked_to": [
                                    {
                                        "node_object_path": "/Asset:AnimGraph.Target",
                                        "node_guid": "target",
                                        "pin_id": "target-pin",
                                        "pin_name": "Pose",
                                        "direction": "Input",
                                    }
                                ],
                            }
                        ],
                    },
                    {
                        "object_path": "/Asset:AnimGraph.Target",
                        "node_guid": "target",
                        "pins": [],
                    },
                ],
            }
        ],
    }
    value = contract(source, "AnimGraph")
    assert value["source_graph"]["nodes"] == source["graphs"][0]["nodes"]
    assert len(value["links"]) == 1
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "contract.json"
        write_json(output, value)
        assert read_json(output) == value
    return {
        "complete_source_graph_preserved": "PASS",
        "pin_links_preserved": "PASS",
        "round_trip": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path)
    parser.add_argument("--graph-suffix")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source is None
        or args.graph_suffix is None
        or args.output is None
        or args.write == args.check
    ):
        raise ValueError("source, graph-suffix, output, and exactly one mode are required")
    generated = contract(read_json(args.source), args.graph_suffix)
    if args.write:
        write_json(args.output, generated)
        print(
            json.dumps(
                {
                    "output": args.output.as_posix(),
                    "nodes": len(generated["source_graph"]["nodes"]),
                    "links": len(generated["links"]),
                    "source_graph_sha256": generated["source_graph_sha256"],
                },
                ensure_ascii=False,
            )
        )
        return 0
    if not args.output.is_file():
        raise ValueError("source graph contract is missing")
    current = read_json(args.output)
    differences = [
        key
        for key in sorted(set(generated) | set(current))
        if generated.get(key) != current.get(key)
    ]
    print(
        json.dumps(
            {
                "output": args.output.as_posix(),
                "differences": differences,
            },
            ensure_ascii=False,
        )
    )
    return 0 if not differences else 1


if __name__ == "__main__":
    raise SystemExit(main())
