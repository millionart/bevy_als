import argparse
import hashlib
import json
import tempfile
from pathlib import Path


FORMAT = "als_ue_authored_graph_inventory_v1"
CLASS_KINDS = {
    "/Script/AnimGraph.AnimationGraph": "graph",
    "/Script/AnimGraph.AnimationStateGraph": "graph",
    "/Script/AnimGraph.AnimationStateMachineGraph": "fsm",
    "/Script/AnimGraph.AnimationTransitionGraph": "transition",
}


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def canonical_hash(value):
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def graph_suffix(source, graph):
    asset_path = source.get("asset_path")
    object_path = graph.get("object_path")
    prefix = f"{asset_path}:"
    if not isinstance(asset_path, str) or not asset_path:
        raise ValueError("source asset path is invalid")
    if not isinstance(object_path, str) or not object_path.startswith(prefix):
        raise ValueError("source graph object path is invalid")
    return object_path[len(prefix) :]


def inventory(source, root_suffix):
    if not isinstance(root_suffix, str) or not root_suffix:
        raise ValueError("root graph suffix is invalid")
    values = []
    for graph in source.get("graphs", []):
        class_path = graph.get("class_path")
        if class_path not in CLASS_KINDS:
            continue
        suffix = graph_suffix(source, graph)
        if suffix != root_suffix and not suffix.startswith(f"{root_suffix}."):
            continue
        values.append(
            {
                "source_graph": suffix,
                "kind": CLASS_KINDS[class_path],
                "class_path": class_path,
                "name": graph.get("name"),
                "index": graph.get("index"),
            }
        )
    values.sort(key=lambda value: (value["index"], value["source_graph"]))
    if not values or values[0]["source_graph"] != root_suffix:
        raise ValueError("root authored graph is missing")
    source_graphs = {value["source_graph"] for value in values}
    for value in values:
        parents = [
            candidate
            for candidate in source_graphs
            if candidate != value["source_graph"]
            and value["source_graph"].startswith(f"{candidate}.")
        ]
        value["parent_source_graph"] = max(parents, key=len) if parents else None
    return {
        "format": FORMAT,
        "source_asset": source["asset_path"],
        "root_source_graph": root_suffix,
        "entries": values,
    }


def check(expected, actual):
    return canonical_hash(expected) == canonical_hash(actual)


def fixture_source():
    return {
        "asset_path": "/Asset",
        "graphs": [
            {
                "index": 0,
                "object_path": "/Asset:AnimGraph",
                "class_path": "/Script/AnimGraph.AnimationGraph",
                "name": "AnimGraph",
            },
            {
                "index": 1,
                "object_path": "/Asset:AnimGraph.StateMachine.Run",
                "class_path": "/Script/AnimGraph.AnimationStateMachineGraph",
                "name": "Run",
            },
            {
                "index": 2,
                "object_path": "/Asset:AnimGraph.StateMachine.Run.State.Idle",
                "class_path": "/Script/AnimGraph.AnimationStateGraph",
                "name": "Idle",
            },
            {
                "index": 3,
                "object_path": "/Asset:AnimGraph.StateMachine.Run.Transition.Go",
                "class_path": "/Script/AnimGraph.AnimationTransitionGraph",
                "name": "Go",
            },
            {
                "index": 4,
                "object_path": "/Asset:EventGraph",
                "class_path": "/Script/Engine.EdGraph",
                "name": "EventGraph",
            },
        ],
    }


def run_self_test():
    value = inventory(fixture_source(), "AnimGraph")
    assert [entry["kind"] for entry in value["entries"]] == [
        "graph",
        "fsm",
        "graph",
        "transition",
    ]
    assert value["entries"][2]["parent_source_graph"] == "AnimGraph.StateMachine.Run"
    assert value["entries"][3]["parent_source_graph"] == "AnimGraph.StateMachine.Run"
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "inventory.json"
        write_json(output, value)
        assert check(value, read_json(output))
    return {
        "recursive_authored_graph_discovery": "PASS",
        "state_machine_state_transition_classification": "PASS",
        "round_trip": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path)
    parser.add_argument("--root-graph-suffix")
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
        or args.root_graph_suffix is None
        or args.output is None
        or args.write == args.check
    ):
        raise ValueError("source, root-graph-suffix, output, and exactly one mode are required")
    generated = inventory(read_json(args.source), args.root_graph_suffix)
    if args.write:
        write_json(args.output, generated)
        return 0
    if not args.output.is_file() or not check(generated, read_json(args.output)):
        raise ValueError("authored graph inventory differs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
