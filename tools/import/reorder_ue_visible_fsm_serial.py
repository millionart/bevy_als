import argparse
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

import sys

sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tools" / "import"))

from normalize_fsm_editor_layout import field_span, split_top_level, top_level_colon


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def collection(text, field_name):
    _, start, end = field_span(text, field_name)
    return start, end, text[start : end + 1]


def raw_entries(text, field_name):
    _, _, value = collection(text, field_name)
    return split_top_level(value[1:-1], ",")


def quoted(entry, field_name):
    prefix = f'{field_name}: "'
    start = entry.find(prefix)
    if start < 0:
        raise ValueError(f"{field_name} is missing")
    start += len(prefix)
    end = entry.find('"', start)
    if end < 0:
        raise ValueError(f"{field_name} is malformed")
    return entry[start:end]


def source_nodes(contract, class_suffix):
    return [
        node
        for node in contract["source_graph"]["nodes"]
        if node["class_path"].endswith(class_suffix)
    ]


def target_entries_by_id(text, field_name):
    result = {}
    for entry in raw_entries(text, field_name):
        identifier = quoted(entry, "id")
        if identifier in result:
            raise ValueError(f"duplicate {field_name} id")
        result[identifier] = entry.strip()
    return result


def expected_order(contract, mapping, source_kind, map_key, target_key):
    source = source_nodes(contract, source_kind)
    values = {entry["source_node"]: entry[target_key] for entry in mapping[map_key]}
    if set(values) != {node["object_path"] for node in source}:
        raise ValueError(f"{map_key} does not cover the source nodes")
    return [values[node["object_path"]] for node in source]


def reordered_text(text, mapping, contract):
    state_order = expected_order(
        contract,
        mapping,
        "AnimStateNode",
        "state_map",
        "target_state",
    )
    transition_order = expected_order(
        contract,
        mapping,
        "AnimStateTransitionNode",
        "transition_map",
        "target_transition",
    )
    result = text
    for field_name, order in (("states", state_order), ("transitions", transition_order)):
        entries = target_entries_by_id(result, field_name)
        if set(entries) != set(order):
            raise ValueError(f"target {field_name} does not match mapped ids")
        start, end, _ = collection(result, field_name)
        replacement = "[\n" + ",\n".join(entries[value] for value in order) + "\n]"
        result = result[:start] + replacement + result[end + 1 :]
    return result


def run(root, map_path, write):
    mapping = read_json(root / map_path)
    source_path = root / mapping["source_contract"]
    target_path = root / mapping["target_fsm"]
    if not source_path.is_file() or not target_path.is_file():
        raise ValueError("source contract or target FSM is missing")
    current = target_path.read_text(encoding="utf-8")
    expected = reordered_text(current, mapping, read_json(source_path))
    if write:
        target_path.write_text(expected, encoding="utf-8")
        return {"changed": current != expected}
    if current != expected:
        raise ValueError("FSM state or transition order differs from source order")
    return {"changed": False}


def fixture_contract():
    return {
        "source_graph": {
            "nodes": [
                {"object_path": "/Asset:Fsm.Run", "class_path": "AnimStateNode"},
                {"object_path": "/Asset:Fsm.Idle", "class_path": "AnimStateNode"},
                {"object_path": "/Asset:Fsm.Go", "class_path": "AnimStateTransitionNode"},
                {"object_path": "/Asset:Fsm.Back", "class_path": "AnimStateTransitionNode"},
            ]
        }
    }


def fixture_mapping():
    return {
        "source_contract": "source.json",
        "target_fsm": "target.fsm.ron",
        "state_map": [
            {"source_node": "/Asset:Fsm.Run", "target_state": "run", "target_graph": "run.ron"},
            {"source_node": "/Asset:Fsm.Idle", "target_state": "idle", "target_graph": "idle.ron"},
        ],
        "transition_map": [
            {"source_node": "/Asset:Fsm.Go", "target_transition": "go"},
            {"source_node": "/Asset:Fsm.Back", "target_transition": "back"},
        ],
    }


def run_self_test():
    text = (
        "(\n"
        'states: [(id: "idle", label: "Idle"), (id: "run", label: "Run")],\n'
        'transitions: [(id: "back", source: "run", target: "idle"), (id: "go", source: "idle", target: "run")],\n'
        ")\n"
    )
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "source.json").write_text(json.dumps(fixture_contract()), encoding="utf-8")
        (root / "map.json").write_text(json.dumps(fixture_mapping()), encoding="utf-8")
        (root / "target.fsm.ron").write_text(text, encoding="utf-8")
        try:
            run(root, Path("map.json"), False)
        except ValueError:
            pass
        else:
            raise AssertionError("unordered FSM was accepted")
        result = run(root, Path("map.json"), True)
        assert result["changed"]
        assert not run(root, Path("map.json"), False)["changed"]
    return {
        "source_state_order_enforced": "PASS",
        "source_transition_order_enforced": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--map", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.map is None or args.write == args.check:
        raise ValueError("map and exactly one mode are required")
    print(json.dumps(run(args.root.resolve(), args.map, args.write), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
