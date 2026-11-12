import gzip
import importlib.util
import json
from pathlib import Path


AUDIT = Path(__file__).resolve().parent
ROOT = AUDIT.parents[3]


def samples_for(position, f32):
    x, y = position
    if x + y <= 1.0:
        weights = [(1, f32(1.0 - x - y)), (0, x), (3, y)]
    else:
        weights = [(0, f32(1.0 - y)), (2, f32(x + y - 1.0)), (3, f32(1.0 - x))]
    weights = [(index, weight) for index, weight in weights if weight > f32(1e-5)]
    for end in reversed(range(1, len(weights))):
        smallest = min(range(end + 1), key=lambda index: weights[index][1])
        weights[smallest], weights[end] = weights[end], weights[smallest]
    total = 0.0
    for _, weight in weights:
        total = f32(total + weight)
    return sorted([index, f32(weight / total)] for index, weight in weights)


def main():
    reference = AUDIT.parent / "d210/replay_filter_clock.py"
    spec = importlib.util.spec_from_file_location("source_filter_clock", reference)
    clock = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(clock)
    path = AUDIT / "slots.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)["sync_group_clock_probe"]
    request = json.loads((AUDIT / "slots.request.json").read_bytes())
    graph = (ROOT / "assets/ALS/Character/AnimationInstances/Stances/AB_Als_Standing/AnimGraph.animgraph.ron").read_text()
    assert graph.count("x_window_seconds: 0.2, y_window_seconds: 0.4") == 6
    assert graph.count("triangles: [(1, 0, 3), (0, 2, 3)]") == 6
    windows = [clock.f32(0.2), clock.f32(0.4)]
    cases = []
    summary = {}
    for index, (requested, observed) in enumerate(zip(request["samples"], native["samples"], strict=True)):
        name = requested["name"]
        assert name == observed["name"]
        expected_asset = request["players"][index]["asset"]
        assert native["players"][index]["asset_path"] == expected_asset
        asset_name = expected_asset.split(".")[-1]
        assert asset_name in graph
        asset = ROOT / f"assets/ALS/Animations/Grounded/WalkRun/{asset_name}.json"
        source_samples = json.loads(asset.read_bytes())["samples"]
        assert [sample["value"][:2] for sample in source_samples] == [[1, 0], [0, 0], [1, 1], [0, 1]]
        frames = [{"delta_seconds": step["delta_seconds"], "x": step["blend_positions"][index][0],
                   "y": step["blend_positions"][index][1]} for step in requested["steps"]]
        for frame in frames[:25]:
            assert frame["delta_seconds"] > 1e-4
        assert sum(frame["delta_seconds"] for frame in frames[:25]) < windows[0]
        positions = {mode: [[axis[0] for axis in values] for values in zip(
            *(clock.run(frames, axis, window, mode) for axis, window in zip("xy", windows, strict=True)), strict=True)]
            for mode in ("ring", "clock_only")}
        differences = {"filtered_position": 0, "sample_weights": 0}
        compact = []
        for frame_index, (requested_frame, frame) in enumerate(zip(frames, observed["frames"], strict=True)):
            assert frame["order"] == [index] and frame["leader"] == index
            player = frame["players"][index]
            position = [player["filtered_position"][axis] for axis in "xy"]
            assert position == positions["ring"][frame_index], (name, frame_index)
            for sample in player["samples"]:
                assert source_samples[sample["sample_index"]]["animation"] == sample["animation"]
            expected = sorted([sample["sample_index"], sample["weight"]] for sample in player["samples"])
            assert samples_for(position, clock.f32) == expected, (name, frame_index, expected)
            chrono = positions["clock_only"][frame_index]
            differences["filtered_position"] += chrono != position
            differences["sample_weights"] += samples_for(chrono, clock.f32) != expected
            compact.append({"input": [requested_frame["x"], requested_frame["y"]], "delta": requested_frame["delta_seconds"],
                            "position": position, "samples": expected})
        assert differences["filtered_position"] > 0 and differences["sample_weights"] > 0, name
        cases.append({"name": name, "frames": compact})
        summary[name] = differences
    print(json.dumps({"windows": windows, "cases": cases, "chronological_ablation": summary}, indent=2))


if __name__ == "__main__":
    main()
