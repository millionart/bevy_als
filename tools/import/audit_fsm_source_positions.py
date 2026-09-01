import argparse
import json
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent

from capture_animgraph_ron_topology import capture_fsm, parse_positions


FORMAT = "als_fsm_source_position_map_v1"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def target_positions(text):
    parsed = parse_positions(text, "states")
    if parsed["malformed"]:
        raise ValueError("target state positions are malformed")
    result = {}
    for entry in parsed["entries"]:
        if entry["id"] in result or entry["position"] is None:
            raise ValueError("target state position is invalid")
        result[entry["id"]] = [float(entry["position"][0]), float(entry["position"][1])]
    return result


def source_states(graph):
    result = {}
    for node in graph["nodes"]:
        if not node.get("class_path", "").endswith("AnimStateNode"):
            continue
        title = node.get("title")
        if not isinstance(title, str) or not title:
            continue
        if title in result:
            raise ValueError(f"duplicate source state title {title}")
        result[title] = {
            "position": [float(node["position_x"]), float(node["position_y"])],
            "object_path": node["object_path"],
        }
    return result


def audit(source, target_root, mapping):
    if mapping.get("format") != FORMAT:
        raise ValueError("FSM source position map format is invalid")
    source_graphs = {graph["object_path"]: graph for graph in source["graphs"]}
    result = {"fsms": [], "differences": []}
    for entry in mapping.get("fsms", []):
        source_path = f'{mapping["source_asset"]}:{entry["source_suffix"]}'
        source_graph = source_graphs.get(source_path)
        row = {
            "target": entry["target"],
            "source": source_path,
            "states": [],
            "differences": [],
        }
        if source_graph is None:
            row["differences"].append("source graph is missing")
            result["fsms"].append(row)
            result["differences"].append(f"{entry['target']}: source graph is missing")
            continue
        target_path = target_root / entry["target"]
        text = target_path.read_text(encoding="utf-8")
        target = capture_fsm(text)
        positions = target_positions(text)
        source_by_label = source_states(source_graph)
        target_labels = []
        for state in target["states"]:
            label = state["label"]
            target_labels.append(label)
            source_state = source_by_label.get(label)
            position = positions.get(state["id"])
            state_row = {
                "id": state["id"],
                "label": label,
                "target_position": position,
                "source_position": None if source_state is None else source_state["position"],
                "source_node": None if source_state is None else source_state["object_path"],
            }
            if source_state is None:
                row["differences"].append(f"missing source state {label}")
            elif position != source_state["position"]:
                row["differences"].append(f"position {label}")
            row["states"].append(state_row)
        extras = sorted(set(source_by_label) - set(target_labels))
        if extras:
            row["differences"].append(f"unrepresented source states {','.join(extras)}")
        result["fsms"].append(row)
        result["differences"].extend(
            f"{entry['target']}: {difference}" for difference in row["differences"]
        )
    return result


def run_self_test():
    source = {
        "graphs": [
            {
                "object_path": "/Asset:Graph",
                "nodes": [
                    {
                        "class_path": "/Script/AnimGraph.AnimStateNode",
                        "title": "Idle",
                        "position_x": 0,
                        "position_y": 0,
                        "object_path": "/Asset:Graph.Idle",
                    }
                ],
            }
        ]
    }
    mapping = {
        "format": FORMAT,
        "source_asset": "/Asset",
        "fsms": [{"target": "state.fsm.ron", "source_suffix": "Graph"}],
    }
    text = (
        "(\n"
        'states: [(id: "idle", label: "Idle", graph: "idle.animgraph.ron")],\n'
        "transitions: [],\n"
        'node_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "pose": Pose }, input_order: {}, output_order: { Data("pose"): 0 }),\n'
        "default_data: {},\n"
        'editor_metadata: (states: { "idle": (0.0, 0.0) }),\n'
        ")\n"
    )
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "state.fsm.ron").write_text(text, encoding="utf-8")
        report = audit(source, root, mapping)
        assert not report["differences"]
        changed = source.copy()
        changed["graphs"] = [dict(source["graphs"][0])]
        changed["graphs"][0]["nodes"] = [dict(source["graphs"][0]["nodes"][0], position_x=1)]
        assert audit(changed, root, mapping)["differences"]
    return {
        "exact_fsm_state_positions": "PASS",
        "position_mismatch_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path)
    parser.add_argument("--target-root", type=Path)
    parser.add_argument("--mapping", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.source is None or args.target_root is None or args.mapping is None:
        raise ValueError("source, target-root, and mapping are required")
    if args.output is None and not args.check:
        raise ValueError("output or --check is required")
    if args.output is not None and args.check:
        raise ValueError("output and --check are mutually exclusive")
    report = audit(
        read_json(args.source),
        args.target_root,
        read_json(args.mapping),
    )
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "fsms": len(report["fsms"]),
                "differences": len(report["differences"]),
            },
            ensure_ascii=False,
        )
    )
    return 0 if not report["differences"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
