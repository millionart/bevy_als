import argparse
import importlib.util
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def load_module(name, path):
    specification = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


AUDIT = load_module("audit_ue_visible_fsm", ROOT / "tools" / "import" / "audit_ue_visible_fsm.py")
TOPOLOGY = load_module("capture_animgraph_ron_topology", ROOT / "tools" / "import" / "capture_animgraph_ron_topology.py")


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def state_positions(target):
    return {
        value["id"]: [float(value["position"][0]), float(value["position"][1])]
        for value in target["state_positions"]["entries"]
        if value["position"] is not None
    }


def build(contract, target, source_contract, target_fsm, comment_policy, comment_exception):
    source_states = AUDIT.source_states(contract)
    source_transitions = AUDIT.source_transitions(contract)
    source_entries = AUDIT.source_entries(contract)
    target_states = target["states"]
    target_by_label = {}
    for state in target_states:
        target_by_label.setdefault(state["label"], []).append(state)
    positions = state_positions(target)
    state_map = []
    source_to_target = {}
    for source in source_states:
        candidates = target_by_label.get(source["title"], [])
        position = [float(source["position_x"]), float(source["position_y"])]
        candidates = [
            candidate
            for candidate in candidates
            if positions.get(candidate["id"]) == position
        ]
        if len(candidates) != 1:
            raise ValueError(f"source state does not map uniquely: {source['object_path']}")
        target_state = candidates[0]
        source_to_target[source["object_path"]] = target_state["id"]
        state_map.append(
            {
                "source_node": source["object_path"],
                "target_state": target_state["id"],
                "target_graph": target_state["graph"],
            }
        )
    unused = list(target["transitions"])
    transition_map = []
    for source in source_transitions:
        incoming, outgoing = AUDIT.source_transition_endpoints(contract, source)
        if len(incoming) != 1 or len(outgoing) != 1:
            raise ValueError(f"source transition endpoints are invalid: {source['object_path']}")
        source_id = source_to_target[incoming[0]]
        target_id = source_to_target[outgoing[0]]
        candidates = [
            transition
            for transition in unused
            if transition["source"] == source_id and transition["target"] == target_id
        ]
        if not candidates:
            raise ValueError(f"source transition has no target endpoint: {source['object_path']}")
        target_transition = candidates[0]
        unused.remove(target_transition)
        transition_map.append(
            {
                "source_node": source["object_path"],
                "target_transition": target_transition["id"],
                "property_assertions": [],
            }
        )
    if unused:
        raise ValueError("target FSM has unmapped transitions")
    if len(source_entries) != 1:
        raise ValueError("source FSM entry is missing or ambiguous")
    entry_targets = AUDIT.source_entry_target(contract, source_entries[0])
    if len(entry_targets) != 1:
        raise ValueError("source FSM entry target is invalid")
    value = {
        "format": AUDIT.FORMAT,
        "source_contract": source_contract,
        "target_fsm": target_fsm,
        "state_map": state_map,
        "transition_map": transition_map,
        "entry": {
            "source_node": source_entries[0]["object_path"],
            "target_state": source_to_target[entry_targets[0]],
        },
        "comment_groups": [],
        "comment_policy": comment_policy,
    }
    if comment_policy == "ignore":
        value["comment_exception"] = comment_exception
    return value


def run_self_test():
    contract = {
        "source_graph": {
            "nodes": [
                {"object_path": "entry", "class_path": "AnimStateEntryNode", "title": "Entry", "position_x": 0, "position_y": 0},
                {"object_path": "idle", "class_path": "AnimStateNode", "title": "Idle", "position_x": 0, "position_y": 64},
                {"object_path": "move", "class_path": "AnimStateNode", "title": "Move", "position_x": 128, "position_y": 64},
                {"object_path": "transition", "class_path": "AnimStateTransitionNode", "title": "Move", "position_x": 64, "position_y": 64, "properties": []},
            ]
        },
        "links": [
            {"source": {"node_object_path": "entry"}, "target": {"node_object_path": "idle"}},
            {"source": {"node_object_path": "idle"}, "target": {"node_object_path": "transition"}},
            {"source": {"node_object_path": "transition"}, "target": {"node_object_path": "move"}},
        ],
    }
    target = {
        "states": [
            {"id": "idle", "label": "Idle", "graph": "idle.ron"},
            {"id": "move", "label": "Move", "graph": "move.ron"},
        ],
        "transitions": [{"id": "transition", "source": "idle", "target": "move"}],
        "state_positions": {"entries": [{"id": "idle", "position": [0, 64]}, {"id": "move", "position": [128, 64]}]},
    }
    value = build(contract, target, "contract.json", "states.fsm.ron", "ignore", "fixture")
    assert value["entry"]["target_state"] == "idle"
    assert value["transition_map"][0]["target_transition"] == "transition"
    assert value["target_fsm"] == "states.fsm.ron"
    return {
        "state_label_position_mapping": "PASS",
        "transition_endpoint_order_mapping": "PASS",
        "entry_mapping": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source-contract")
    parser.add_argument("--target-fsm")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--comment-policy", choices=["require", "ignore"], default="require")
    parser.add_argument("--comment-exception", default="")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        not args.source_contract
        or not args.target_fsm
        or args.output is None
        or not args.write
    ):
        raise ValueError("source-contract, target-fsm, output, and write are required")
    root = args.root.resolve()
    source_contract = root / args.source_contract
    target_fsm = root / args.target_fsm
    source_contract_value = Path(args.source_contract).as_posix()
    target_fsm_value = Path(args.target_fsm).as_posix()
    mapping = build(
        read_json(source_contract),
        TOPOLOGY.capture_fsm(target_fsm.read_text(encoding="utf-8")),
        source_contract_value,
        target_fsm_value,
        args.comment_policy,
        args.comment_exception,
    )
    output = args.output if args.output.is_absolute() else root / args.output
    write_json(output, mapping)
    print(json.dumps({"output": output.relative_to(root).as_posix(), "states": len(mapping["state_map"]), "transitions": len(mapping["transition_map"])}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
