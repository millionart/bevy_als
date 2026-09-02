import argparse
import importlib.util
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, filename)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


AUDIT = load_module(
    "run_ue_visible_animgraph_audit",
    ROOT / "tools" / "import" / "run_ue_visible_animgraph_audit.py",
)
FSM_AUDIT = load_module(
    "audit_ue_visible_fsm",
    ROOT / "tools" / "import" / "audit_ue_visible_fsm.py",
)

GRAPH_MAP_FORMAT = "als_ue_visible_animgraph_map_v2"
FSM_MAP_FORMAT = "als_ue_visible_fsm_map_v1"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def map_paths(index):
    entries = index.get("entries")
    if not isinstance(entries, list) or not entries:
        raise ValueError("audit index entries are invalid")
    values = []
    for entry in entries:
        path = entry.get("map") if isinstance(entry, dict) else None
        if not isinstance(path, str) or not path:
            raise ValueError("audit index map is invalid")
        values.append(path)
    if len(values) != len(set(values)):
        raise ValueError("audit index map is duplicated")
    return values


def audit_map(root, map_path, source_path, jobs):
    mapping = read_json(root / map_path)
    map_format = mapping.get("format")
    if map_format == GRAPH_MAP_FORMAT:
        return "graph", AUDIT.run(root, Path(map_path), jobs, source_path)
    if map_format == FSM_MAP_FORMAT:
        return "fsm", FSM_AUDIT.audit(root, mapping, source_path)
    raise ValueError("audit map format is invalid")


def run(root, index_path, source_path, jobs):
    index = read_json(root / index_path)
    values = []
    for map_path in map_paths(index):
        try:
            kind, report = audit_map(root, map_path, source_path, jobs)
            values.append(
                {
                    "map": map_path,
                    "kind": kind,
                    "pass": report["pass"],
                    "source_visible_nodes": report.get("source_visible_nodes"),
                    "target_nodes": report.get("target_nodes"),
                    "source_links": report.get("source_links"),
                    "target_edges": report.get("target_edges"),
                    "source_states": report.get("source_states"),
                    "target_states": report.get("target_states"),
                    "source_transitions": report.get("source_transitions"),
                    "target_transitions": report.get("target_transitions"),
                    "differences": len(report.get("differences", [])),
                    "issues": report.get("issues", []),
                    "source_contract_fresh": report.get("source_contract_fresh", False),
                    "report": report if not report["pass"] else None,
                }
            )
        except Exception as error:
            values.append(
                {
                    "map": map_path,
                    "kind": None,
                    "pass": False,
                    "error": str(error),
                    "source_contract_fresh": False,
                }
            )
    return {
        "maps": len(values),
        "passed": sum(value["pass"] for value in values),
        "failed": sum(not value["pass"] for value in values),
        "entries": values,
    }


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "index.json"
        path.write_text(
            json.dumps({"entries": [{"map": "one.json"}, {"map": "two.json"}]}),
            encoding="utf-8",
        )
        assert map_paths(read_json(path)) == ["one.json", "two.json"]
        fsm_map = Path(directory) / "fsm.json"
        write_json(fsm_map, {"format": FSM_MAP_FORMAT})
        kind, report = audit_map(Path(directory), "fsm.json", fsm_map, 2)
        assert kind == "fsm"
        assert not report["pass"]
        try:
            map_paths({"entries": [{"map": "one.json"}, {"map": "one.json"}]})
        except ValueError:
            pass
        else:
            raise AssertionError("duplicate map was accepted")
        output = Path(directory) / "report.json"
        write_json(output, {"maps": 2})
        assert read_json(output) == {"maps": 2}
    return {
        "indexed_map_discovery": "PASS",
        "duplicate_map_rejected": "PASS",
        "machine_readable_report_round_trip": "PASS",
        "graph_and_fsm_dispatch_available": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--index", type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--summary", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.index is None or args.source is None or not args.check or args.jobs < 1:
        raise ValueError("index, source, --check, and a positive jobs value are required")
    root = args.root.resolve()
    source = args.source if args.source.is_absolute() else root / args.source
    if not source.is_file():
        raise ValueError("source extraction is missing")
    report = run(root, args.index, source, args.jobs)
    if args.output is not None:
        output_path = args.output if args.output.is_absolute() else root / args.output
        write_json(output_path, report)
    output = (
        {key: report[key] for key in ("maps", "passed", "failed")}
        if args.summary
        else report
    )
    print(json.dumps(output, ensure_ascii=False, indent=2))
    return 0 if report["failed"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
