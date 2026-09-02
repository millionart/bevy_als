import argparse
import importlib.util
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
FORMAT = "als_ue_visible_recursive_audit_index_v1"
GRAPH_FORMAT = "als_ue_visible_animgraph_map_v2"
FSM_FORMAT = "als_ue_visible_fsm_map_v1"


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, filename)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


REBIND = load_module(
    "rebind_ue_visible_maps_to_recursive_contracts",
    ROOT / "tools" / "import" / "rebind_ue_visible_maps_to_recursive_contracts.py",
)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def relative_path(root, path):
    return path.resolve().relative_to(root.resolve()).as_posix()


def source_entries(root, index_path):
    index = read_json(index_path)
    entries = index.get("entries")
    if index.get("format") != "als_ue_visible_graph_contract_batch_v1" or not isinstance(entries, list):
        raise ValueError("recursive source contract index is invalid")
    result = []
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("recursive source contract entry is invalid")
        source_graph = entry.get("source_graph")
        kind = entry.get("kind")
        contract = entry.get("contract")
        if not isinstance(source_graph, str) or not source_graph:
            raise ValueError("recursive source graph is invalid")
        if kind not in {"graph", "fsm", "transition"}:
            raise ValueError("recursive source kind is invalid")
        if not isinstance(contract, str) or not contract:
            raise ValueError("recursive source contract is invalid")
        contract_path = index_path.parent / contract
        if not contract_path.is_file():
            raise ValueError(f"recursive source contract is missing: {contract_path.as_posix()}")
        result.append(
            {
                "source_graph": source_graph,
                "kind": kind,
                "source_contract": relative_path(root, contract_path),
            }
        )
    return result


def map_candidates(root, map_glob):
    result = {}
    paths = sorted(root.glob(map_glob))
    if not paths:
        raise ValueError("map glob matched no files")
    for path in paths:
        mapping = read_json(path)
        map_format = mapping.get("format")
        if map_format not in {GRAPH_FORMAT, FSM_FORMAT}:
            continue
        source_graph = REBIND.map_source_graph(root, mapping)
        target_key = "target_fsm" if map_format == FSM_FORMAT else "target_graph"
        target_path = mapping.get(target_key)
        if not isinstance(target_path, str) or not target_path:
            raise ValueError(f"map target path is invalid: {path.as_posix()}")
        target = target_path.replace("\\", "/")
        if not (root / target).is_file():
            raise ValueError(f"map target is missing: {target}")
        result.setdefault(source_graph, []).append(
            {
                "map": relative_path(root, path),
                "format": map_format,
                "target_path": target,
            }
        )
    return result


def select_candidate(entry, candidates):
    expected_format = FSM_FORMAT if entry["kind"] == "fsm" else GRAPH_FORMAT
    matches = [value for value in candidates if value["format"] == expected_format]
    if entry["kind"] == "transition":
        tools_matches = [value for value in matches if value["target_path"].startswith("tools/")]
        if len(tools_matches) == 1:
            return tools_matches[0]
        if len(tools_matches) > 1:
            raise ValueError(f"transition source graph has ambiguous tool maps: {entry['source_graph']}")
        raise ValueError(f"transition source graph has no tool audit map: {entry['source_graph']}")
    if len(matches) != 1:
        raise ValueError(f"source graph has ambiguous or missing map: {entry['source_graph']}")
    return matches[0]


def build(root, index_path, map_glob):
    entries = source_entries(root, index_path)
    candidates = map_candidates(root, map_glob)
    result = []
    known = {entry["source_graph"] for entry in entries}
    unknown = sorted(set(candidates) - known)
    if unknown:
        raise ValueError("maps reference unknown source graphs: " + ", ".join(unknown))
    for entry in entries:
        candidate = select_candidate(entry, candidates.get(entry["source_graph"], []))
        if entry["kind"] == "transition" and not candidate["target_path"].startswith("tools/"):
            raise ValueError("transition audit graph is under the runtime asset tree")
        result.append(
            {
                **entry,
                "map": candidate["map"],
                "target_path": candidate["target_path"],
            }
        )
    return {
        "format": FORMAT,
        "source_contract_index": relative_path(root, index_path),
        "entries": result,
    }


def check(path, expected):
    if not path.is_file():
        return ["recursive audit index is missing"]
    current = read_json(path)
    return [] if canonical(current) == canonical(expected) else ["recursive audit index differs"]


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        contracts = root / "contracts"
        maps = root / "maps"
        assets = root / "assets"
        tools = root / "tools"
        contracts.mkdir()
        maps.mkdir()
        assets.mkdir()
        tools.mkdir()
        graph_contract = contracts / "graph.source.json"
        transition_contract = contracts / "transition.source.json"
        write_json(graph_contract, {"source_graph": "AnimGraph"})
        write_json(transition_contract, {"source_graph": "AnimGraph.Transition"})
        index_path = contracts / "index.json"
        write_json(
            index_path,
            {
                "format": "als_ue_visible_graph_contract_batch_v1",
                "entries": [
                    {"source_graph": "AnimGraph", "kind": "graph", "contract": graph_contract.name},
                    {"source_graph": "AnimGraph.Transition", "kind": "transition", "contract": transition_contract.name},
                ],
            },
        )
        runtime_graph = assets / "graph.animgraph.ron"
        runtime_transition = assets / "transition.animgraph.ron"
        audit_transition = tools / "transition.animgraph.ron"
        for path in (runtime_graph, runtime_transition, audit_transition):
            path.write_text("()\n", encoding="utf-8")
        write_json(
            maps / "graph.visible.json",
            {
                "format": GRAPH_FORMAT,
                "source_contract": "contracts/graph.source.json",
                "target_graph": "assets/graph.animgraph.ron",
            },
        )
        write_json(
            maps / "transition.runtime.visible.json",
            {
                "format": GRAPH_FORMAT,
                "source_contract": "contracts/transition.source.json",
                "target_graph": "assets/transition.animgraph.ron",
            },
        )
        write_json(
            maps / "transition.audit.visible.json",
            {
                "format": GRAPH_FORMAT,
                "source_contract": "contracts/transition.source.json",
                "target_graph": "tools/transition.animgraph.ron",
            },
        )
        value = build(root, index_path, "maps/*.visible.json")
        assert value["entries"][1]["map"] == "maps/transition.audit.visible.json"
        output = root / "index.json"
        write_json(output, value)
        assert not check(output, value)
    return {
        "recursive_source_map_coverage": "PASS",
        "tool_transition_map_preferred": "PASS",
        "write_check_round_trip": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source-index", type=Path)
    parser.add_argument("--map-glob")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source_index is None
        or args.map_glob is None
        or args.output is None
        or args.write == args.check
    ):
        raise ValueError("source-index, map-glob, output, and exactly one mode are required")
    root = args.root.resolve()
    index_path = args.source_index if args.source_index.is_absolute() else root / args.source_index
    output_path = args.output if args.output.is_absolute() else root / args.output
    value = build(root, index_path, args.map_glob)
    if args.write:
        write_json(output_path, value)
        print(json.dumps({"entries": len(value["entries"])}, ensure_ascii=False))
        return 0
    issues = check(output_path, value)
    print(json.dumps({"issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
