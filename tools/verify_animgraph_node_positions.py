import argparse
import json
import math
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent / "import"))

from capture_animgraph_ron_topology import entries, parse_positions


def graph_files(root, includes):
    roots = [root / value for value in includes] if includes else [root]
    paths = []
    for value in roots:
        if value.is_file() and value.name.endswith(".animgraph.ron"):
            paths.append(value)
        elif value.is_dir():
            paths.extend(value.rglob("*.animgraph.ron"))
        else:
            raise ValueError(f"included path does not exist or is not an animgraph: {value}")
    return sorted(set(paths))


def node_ids(text):
    _, _, _, values = entries(text, "nodes")
    ids = []
    malformed = []
    for value in values:
        marker = 'id: "'
        start = value.find(marker)
        if start < 0:
            malformed.append(value)
            continue
        start += len(marker)
        end = value.find('"', start)
        if end < 0:
            malformed.append(value)
            continue
        ids.append(value[start:end])
    return ids, malformed


def issues(text):
    ids, malformed_nodes = node_ids(text)
    positions = parse_positions(text, "node_positions")
    known = set(ids)
    by_id = {}
    duplicate_ids = []
    invalid = []
    by_position = {}
    for entry in positions["entries"]:
        node_id = entry["id"]
        if node_id in by_id:
            duplicate_ids.append(node_id)
        else:
            by_id[node_id] = entry
        position = entry["position"]
        if position is None or not all(math.isfinite(value) for value in position):
            invalid.append(node_id)
        elif node_id in known:
            by_position.setdefault(position, []).append(node_id)
    missing = [node_id for node_id in ids if node_id not in by_id]
    extra = sorted(node_id for node_id in by_id if node_id not in known)
    overlaps = [
        sorted(node_ids_at_position)
        for node_ids_at_position in by_position.values()
        if len(node_ids_at_position) > 1
    ]
    return {
        "malformed_node_entries": malformed_nodes,
        "malformed_position_entries": positions["malformed"],
        "missing_positions": missing,
        "extra_positions": extra,
        "duplicate_position_ids": sorted(set(duplicate_ids)),
        "invalid_positions": sorted(set(invalid)),
        "overlapping_positions": sorted(overlaps),
    }


def has_issues(report):
    return any(report.values())


def run(root, includes):
    failures = {}
    for path in graph_files(root, includes):
        text = path.read_text(encoding="utf-8")
        report = issues(text)
        if has_issues(report):
            failures[path.relative_to(root).as_posix()] = report
    return failures


def run_self_test():
    complete = """
(
nodes: [(id: "a"), (id: "b")],
editor_metadata: (node_positions: { "a": (0.0, 0.0), "b": (10.0, 10.0) }),
)
"""
    incomplete = """
(
nodes: [(id: "a"), (id: "b")],
editor_metadata: (node_positions: { "a": (0.0, 0.0), "b": (0.0, 0.0) }),
)
"""
    assert not has_issues(issues(complete))
    assert has_issues(issues(incomplete))
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        path = root / "graph.animgraph.ron"
        path.write_text(complete, encoding="utf-8")
        assert not run(root, [])
    return {"animgraph_node_positions": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("assets/graphs"))
    parser.add_argument("--include", action="append", default=[])
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), indent=2))
        return 0
    failures = run(args.root.resolve(), args.include)
    print(json.dumps({"failures": failures}, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
