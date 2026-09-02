import argparse
import json
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools" / "import"))

from audit_ue_visible_animgraph import (
    EXPERIMENTAL_NODE_MAINTENANCE_NOTE,
    source_experimental_warning,
    source_root,
    source_visible_nodes,
)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def expected_notes(contract, mapping):
    node_map = {entry["source_node"]: entry["target_node"] for entry in mapping["node_map"]}
    entries = [(node, node_map[node["object_path"]]) for node in source_visible_nodes(contract)]
    entries.append((source_root(contract), "Output"))
    notes = []
    for node, target_node in entries:
        warning = source_experimental_warning(node)
        if not warning:
            continue
        notes.append(
            {
                "source_node": node["object_path"],
                "target_node": target_node,
                "source_warning": warning,
                "maintenance_note": EXPERIMENTAL_NODE_MAINTENANCE_NOTE,
            }
        )
    return notes


def map_paths(root, explicit_paths, pattern):
    paths = [root / path for path in explicit_paths]
    if pattern:
        paths.extend(root.glob(pattern))
    resolved = sorted({path.resolve() for path in paths})
    if not resolved:
        raise ValueError("no maps selected")
    for path in resolved:
        if not path.is_file():
            raise ValueError(f"map is missing: {path}")
    return resolved


def update_map(root, path, write):
    mapping = read_json(path)
    if "node_map" not in mapping:
        return {
            "map": path.relative_to(root).as_posix(),
            "expected_notes": 0,
            "actual_notes": 0,
            "changed": False,
            "skipped": "non_animgraph_map",
            "pass": True,
        }
    contract_path = root / mapping["source_contract"]
    if not contract_path.is_file():
        raise ValueError(f"source contract is missing: {mapping['source_contract']}")
    expected = expected_notes(read_json(contract_path), mapping)
    actual = mapping.get("experimental_node_notes")
    changed = False
    if write:
        if expected:
            if actual != expected:
                mapping["experimental_node_notes"] = expected
                changed = True
        elif "experimental_node_notes" in mapping:
            mapping.pop("experimental_node_notes")
            changed = True
        if changed:
            write_json(path, mapping)
            actual = mapping.get("experimental_node_notes")
    return {
        "map": path.relative_to(root).as_posix(),
        "expected_notes": len(expected),
        "actual_notes": len(actual or []),
        "changed": changed,
        "pass": actual == expected if expected else actual in (None, []),
    }


def run_self_test():
    contract = {
        "source_graph": {
            "nodes": [
                {
                    "object_path": "/Asset:AnimGraph.Root",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Root",
                    "properties": [],
                },
                {
                    "object_path": "/Asset:AnimGraph.Node",
                    "class_path": "/Script/AnimGraph.AnimGraphNode_Test",
                    "properties": [
                        {
                            "name": "ErrorMsg",
                            "exported_text": "Node - Node is experimental",
                        }
                    ],
                },
            ]
        }
    }
    mapping = {
        "source_contract": "contract.json",
        "node_map": [
            {
                "source_node": "/Asset:AnimGraph.Node",
                "target_node": "node",
            }
        ],
    }
    expected = expected_notes(contract, mapping)
    assert len(expected) == 1
    assert expected[0]["target_node"] == "node"
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        write_json(root / "contract.json", contract)
        write_json(root / "map.json", mapping)
        written = update_map(root, root / "map.json", True)
        assert written["pass"] and written["changed"]
        checked = update_map(root, root / "map.json", False)
        assert checked["pass"] and not checked["changed"]
        contract["source_graph"]["nodes"][1]["properties"] = []
        write_json(root / "contract.json", contract)
        stale = update_map(root, root / "map.json", False)
        assert not stale["pass"]
        cleared = update_map(root, root / "map.json", True)
        assert cleared["pass"] and cleared["changed"]
    return {
        "experimental_notes_generated": "PASS",
        "experimental_notes_checked": "PASS",
        "warning_clearance_removes_stale_note": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", action="append", default=[])
    parser.add_argument("--glob")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.write == args.check:
        raise ValueError("select exactly one of --write or --check")
    root = args.root.resolve()
    reports = [update_map(root, path, args.write) for path in map_paths(root, args.map, args.glob)]
    report = {"maps": reports, "pass": all(value["pass"] for value in reports)}
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
