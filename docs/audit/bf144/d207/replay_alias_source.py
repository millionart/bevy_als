import gzip
import importlib.util
import json
import re
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent
ROOT = AUDIT.parents[3]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    path = AUDIT / "alias.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)
    machine = native["state_machine_players"]["machines"][0]
    assert machine["name"] == "Grounded States"
    assert [state["name"] for state in machine["states"][1:3]] == ["Standing", "Crouching"]
    edges = {edge["index"]: edge for state in machine["states"] for edge in state["exits"]}
    target = ROOT / "assets/ALS/Character/AnimationInstances/AB_Als_Grounded/AnimGraph/SM11_Grounded_States.fsm.ron"
    target_machine = target.read_text().splitlines()
    for index, graph in [(4, "T19"), (6, "T9")]:
        edge = edges[index]
        line = next(line for line in target_machine if edge["transition_guid"] in line)
        assert f"/{graph}/Transition.transition.animgraph.ron" in line and "timed: Some(0.5)" in line
        ids = re.search(r'source: "([^"]+)", target: "([^"]+)"', line).groups()
        assert ids[0] != ids[1]
        source_line = next(line for line in target_machine if f'id: "{ids[0]}"' in line)
        source_graph = re.search(r'graph: "([^"]+)"', source_line)[1]
        assert "Alias" in source_line and "LocalRefPose" in (ROOT / "assets" / source_graph).read_text()
        assert edge["blend_mode"] == 14 and edge["logic"] == 0 and edge["duration"] == 0.5

    cases = []
    failures = 0
    max_lost_weight = 0.0
    curve_audit = AUDIT.parent / "d206"
    spec = importlib.util.spec_from_file_location("source_curve_order", curve_audit / "replay_curve_order.py")
    curve_order = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(curve_order)
    curve = json.loads((curve_audit / "curve.replay.json").read_bytes())["stance_keys"]
    requested = json.loads((AUDIT / "alias.request.json").read_bytes())["samples"]
    for case, request in zip(native["state_machine_entry_probe"]["samples"], requested, strict=True):
        name = case["name"]
        assert name == request["name"]
        frames = case["frames"]
        target = frames[0]["state"]
        previous = frames[1]["state"]
        assert {target, previous} == {1, 2}
        duration = f32(0.5 * f32(1.0 - frames[1]["weights"][target]))
        had_old_transition = bool(frames[1]["active_transitions"])
        elapsed = 0.0
        reference_frames = []
        for frame, step in zip(frames[2:], request["steps"][2:], strict=True):
            elapsed = f32(elapsed + f32(step["delta_seconds"]))
            if frame["active_transitions"] != 1:
                continue
            entry = frame["active_entries"][0]
            assert (entry["source"], entry["target"]) == (previous, target)
            alpha = entry["alpha"]
            assert curve_order.evaluate(curve, f32(elapsed / duration)) == alpha
            source_weight = f32(1.0 - alpha)
            assert [frame["weights"][previous], frame["weights"][target]] == [source_weight, alpha]
            assert weights_updated_once(frame, previous, target, source_weight, alpha)
            if had_old_transition:
                failures += 1
                max_lost_weight = max(max_lost_weight, source_weight)
            reference_frames.append({"elapsed": elapsed, "weights": frame["weights"][1:3]})
        assert reference_frames
        cases.append({"name": name, "target": target - 1, "previous": previous - 1,
                      "duration": duration, "interrupted": had_old_transition, "frames": reference_frames})
    assert failures == 110 and max_lost_weight > 0.99
    print(json.dumps({"cases": cases, "native_frames": sum(len(case["frames"]) for case in native["state_machine_entry_probe"]["samples"]),
                      "alias_fallback_ablation_frames": failures, "maximum_lost_weight": max_lost_weight}, indent=2))


def weights_updated_once(frame, previous, target, source_weight, alpha):
    return frame["updates"][previous] == [source_weight] and frame["updates"][target] == [alpha]


if __name__ == "__main__":
    main()
