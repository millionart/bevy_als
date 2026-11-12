import argparse
import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232a"))
from replay_source_blend import FMA, ROOT, components, early_projection, f32, project, read_capture, source_rotations


def walk_run_poses():
    capture = read_capture(HERE.parent / "d231/blend.native.json.gz")["sync_group_clock_probe"]
    poses = [player["pose"]["bones"] for frame in capture["samples"][1]["frames"]
             for player in frame["players"] if player["pose"] is not None]
    return [poses[index] for index in (11, 43, 89)]


def write_requests():
    poses = walk_run_poses()
    blend_inputs = [{"bone_transforms": [request_transform(bone["local_transform"]) for bone in pose], "curves": {}}
                    for pose in poses]
    for stem, inputs in (("two", blend_inputs[:2]), ("multi", blend_inputs)):
        timing = json.loads((HERE.parent / f"d216/{stem}.request.json").read_text())
        for sample in timing["samples"]:
            sample["poses"] = inputs
        (HERE / f"{stem}.request.json").write_text(json.dumps(timing, separators=(",", ":")) + "\n")
    controls = []
    for a, b, c in zip(*poses):
        assert a["name"] == b["name"] == c["name"]
        first, second, third = (request_transform(bone["local_transform"]) for bone in (a, b, c))
        for mode, values, weights in (
            ("multi", (first,), (1.0,)),
            ("multi", (first, second, third), (0.25, 0.5, 0.25)),
            ("two", (first, second), (0.375, 0.625)),
        ):
            controls.append({
                "name": f"{a['name']}:{mode}:{len(values)}", "profile": "MoveDirectionChange",
                "mode": "Linear", "duration": 0.25, "curve_a": 0.0, "curve_b": 1.0,
                "from": first, "to": second, "deltas": [0.02, 0.04, 0.06],
                "pose_mode": mode, "weighted_inputs": [
                    {"weight": weight, "transform": value} for weight, value in zip(weights, values)
                ],
            })
    (HERE / "operators.request.json").write_text(json.dumps({"samples": controls}, separators=(",", ":")) + "\n")


def request_transform(value):
    rotation, translation, scale = components(value)
    return {"rotation": rotation, "translation_cm": translation, "scale": scale}


def input_transform(value):
    return tuple(tuple(value[field]) for field in ("rotation", "translation_cm", "scale"))


def accumulate(poses, weights, normalize, passes):
    rotation, translation, scale = (tuple(x * weights[0] for x in field) for field in poses[0])
    for (next_rotation, next_translation, next_scale), weight in zip(poses[1:], weights[1:]):
        weighted = tuple(x * weight for x in next_rotation)
        dot = FMA(rotation[2], weighted[2], rotation[0] * weighted[0]) + FMA(
            rotation[3], weighted[3], rotation[1] * weighted[1])
        rotation = tuple(a - b if dot < 0 else a + b for a, b in zip(rotation, weighted))
        translation = tuple(FMA(b, weight, a) for a, b in zip(translation, next_translation))
        scale = tuple(FMA(b, weight, a) for a, b in zip(scale, next_scale))
    for _ in range(passes):
        rotation = normalize(rotation)
    return rotation, translation, scale


def replay(work):
    normalize, _ = source_rotations(work)
    counts = Counter()
    witness = {}

    def check(key, poses, weights, passes, expected, direct=False):
        value = poses[0] if direct else accumulate(poses, weights, normalize, passes)
        assert value == expected, (key, poses, weights, value, expected)
        counts[key] += 1
        early = [early_projection(pose) for pose in poses]
        early = early[0] if direct else accumulate(early, weights, normalize, passes)
        if project(early) != project(value):
            counts[f"{key}_early_projection"] += 1
            witness.setdefault(key, {"source": project(value), "early": project(early), "weights": weights})

    for stem in ("two", "multi"):
        request = json.loads((HERE / f"{stem}.request.json").read_text())
        native = read_capture(HERE / f"{stem}.native.json.gz")["blend_list_probe"]
        for sample, result in zip(request["samples"], native["samples"], strict=True):
            assert sample["name"] == result["name"]
            expected_inputs = [[input_transform(b) for b in pose["bone_transforms"]] for pose in sample["poses"]]
            for frame in result["frames"]:
                inputs = [[components(bone["local_transform"]) for bone in pose["bones"]] for pose in frame["input_poses"]]
                assert inputs == expected_inputs
                active = [(i, weight) for i, weight in enumerate(frame["weights"]) if weight > f32(1e-5)]
                indices, weights = map(list, zip(*active))
                direct = len(weights) == 1 and weights[0] >= f32(1.0 - f32(1e-5))
                if len(weights) == 2 and abs(f32(f32(weights[0] + weights[1]) - 1.0)) <= f32(1e-8):
                    weights[1] = f32(1.0 - weights[0])
                for bone, row in enumerate(frame["pose"]["bones"]):
                    check(f"blend_list_{len(weights)}", [inputs[i][bone] for i in indices], weights,
                          int(len(weights) > 1), components(row["local_transform"]), direct)

    request = json.loads((HERE / "operators.request.json").read_text())
    native = read_capture(HERE / "operators.native.json.gz")["blend_profile_probe"]
    for sample, result in zip(request["samples"], native["samples"], strict=True):
        assert sample["name"] == result["name"]
        weighted = sample["weighted_inputs"]
        poses = [input_transform(row["transform"]) for row in weighted]
        weights = [row["weight"] for row in weighted]
        transitions = [input_transform(sample["from"]), input_transform(sample["to"])]
        for frame in result["frames"]:
            passes = 1 + int(len(weights) > 1) if sample["pose_mode"] == "multi" else 1
            check(sample["pose_mode"], poses, weights, passes, components(frame["weighted_pose"]))
            for bone, values in frame["bones"].items():
                bone_weights = [values["source_weight"], values["target_weight"]]
                check("fsm_raw", transitions, bone_weights, 0, components(values["raw_transform"]))
                check("fsm_final", transitions, bone_weights, 1, components(values["transform"]))
                assert normalize(components(values["raw_transform"])[0]) == components(values["transform"])[0]
                counts["fsm_removed_final_normalization"] += values["raw_transform"] != values["transform"]

    cache = read_capture(HERE / "cache.native.json.gz")["blend_list_probe"]
    archived_cache = read_capture(HERE.parent / "d221/weights.native.json.gz")["blend_list_probe"]
    assert cache == archived_cache
    for sample in cache["samples"]:
        for frame in sample["frames"]:
            assert frame["cached_updates"] == [max(frame["cached_requests"])]
            counts["cache_frames"] += 1
            counts["cache_shared_frames"] += len(frame["cached_requests"]) > 1

    assert all(counts[key] for key in (
        "blend_list_1", "blend_list_2_early_projection", "blend_list_3_early_projection",
        "multi_early_projection", "two_early_projection", "fsm_raw_early_projection",
        "fsm_final_early_projection", "fsm_removed_final_normalization",
        "cache_shared_frames",
    ))
    return {"result": "PASS_SOURCE_DOUBLE_POSE_CHAIN", **counts, "witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--requests", action="store_true")
    parser.add_argument("--work", type=Path, default=ROOT / "target/d232b")
    args = parser.parse_args()
    if args.requests:
        write_requests()
    else:
        print(json.dumps(replay(args.work), indent=2))
