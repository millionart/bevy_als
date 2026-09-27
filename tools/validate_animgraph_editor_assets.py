import argparse
import json
import tempfile
from pathlib import Path

from normalize_animgraph_io_order import (
    has_issues as input_order_has_issues,
    input_order_issues,
    normalize_input_order,
    run_self_test as input_order_self_test,
)
from normalize_fsm_editor_layout import (
    fsm_topology,
    has_issues as layout_has_issues,
    layout_issues,
    normalize_state_positions,
    run_self_test as fsm_layout_self_test,
)


def graph_paths(root):
    return sorted(
        list(root.rglob("*.animgraph.ron")) + list(root.rglob("*.fsm.ron"))
    )


def relative(path, root):
    return path.relative_to(root).as_posix()


def repair_asset(path):
    with path.open("r", encoding="utf-8", newline="") as file:
        text = file.read()
    updated, input_repairs = normalize_input_order(text)
    layout_repairs = {}
    if path.name.endswith(".fsm.ron"):
        updated, layout_repairs = normalize_state_positions(updated)
    if updated != text:
        with path.open("w", encoding="utf-8", newline="") as file:
            file.write(updated)
    repairs = {}
    if input_repairs and updated != text:
        repairs["input_order"] = input_repairs
    if layout_repairs and updated != text:
        repairs["fsm_layout"] = layout_repairs
    return repairs


def validate_asset(path):
    with path.open("r", encoding="utf-8", newline="") as file:
        text = file.read()
    failures = {}
    input_issues = input_order_issues(text)
    if input_order_has_issues(input_issues):
        failures["input_order"] = input_issues
    self_transitions = []
    if path.name.endswith(".fsm.ron"):
        fsm_layout_issues = layout_issues(text)
        if layout_has_issues(fsm_layout_issues):
            failures["fsm_layout"] = fsm_layout_issues
        topology = fsm_topology(text)
        topology_failures = {
            name: value
            for name, value in topology.items()
            if name != "self_transitions" and value
        }
        if topology_failures:
            failures["fsm_topology"] = topology_failures
        self_transitions = topology["self_transitions"]
    return failures, self_transitions


def run(root, fix):
    paths = graph_paths(root)
    repairs = {}
    if fix:
        for path in paths:
            path_repairs = repair_asset(path)
            if path_repairs:
                repairs[relative(path, root)] = path_repairs
    failures = {} if paths else {"root": {"error": "No animation graph or state machine assets found"}}
    self_transitions = []
    for path in paths:
        name = relative(path, root)
        try:
            path_failures, path_self_transitions = validate_asset(path)
        except (OSError, ValueError) as error:
            failures[name] = {"parse_error": str(error)}
            continue
        if path_failures:
            failures[name] = path_failures
        self_transitions.extend(
            {"path": name, **transition} for transition in path_self_transitions
        )
    report = {
        "fix": fix,
        "checked_files": len(paths),
        "repairs": repairs,
        "failures": failures,
        "self_transitions": self_transitions,
    }
    return report, not failures


def run_self_test():
    results = {
        "input_order": input_order_self_test(),
        "fsm_layout": fsm_layout_self_test(),
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        for empty_root in (root, root / "missing"):
            report, passed = run(empty_root, False)
            assert not passed and report["checked_files"] == 0
        (root / "broken.animgraph.ron").write_text(
            '(\n'
            '    input_times: ["tick"],\n'
            '    input_data: { "pose": Pose },\n'
            '    output_time: true,\n'
            '    output_data: {},\n'
            '    input_order: {},\n'
            '    output_order: {},\n'
            ')\n',
            encoding="utf-8",
        )
        (root / "broken.fsm.ron").write_text(
            '(\n'
            '    states: [(id: "a"), (id: "b")],\n'
            '    transitions: [(id: "loop", source: "a", target: "a")],\n'
            '    node_spec: (\n'
            '        input_times: ["tick"],\n'
            '        input_data: {},\n'
            '        output_time: true,\n'
            '        output_data: {},\n'
            '        input_order: {},\n'
            '        output_order: {},\n'
            '    ),\n'
            '    editor_metadata: (states: { "a": (160.0, 140.0) }),\n'
            ')\n',
            encoding="utf-8",
        )
        report, passed = run(root, True)
        assert passed
        assert report["repairs"]
        assert len(report["self_transitions"]) == 1
    results["composite_repair"] = "PASS"
    results["empty_roots_rejected"] = "PASS"
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("assets"))
    parser.add_argument("--fix", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), indent=2))
        return 0
    report, passed = run(args.root.resolve(), args.fix)
    print(json.dumps(report, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
