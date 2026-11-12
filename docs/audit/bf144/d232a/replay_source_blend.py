import argparse
import ctypes
import json
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d230"))
from replay_animation_relative import PACK, clip_data, components, f32, pose_at, project, read_capture, relative, source_rotations


FMA = ctypes.CDLL(None).fma
FMA.argtypes = [ctypes.c_double] * 3
FMA.restype = ctypes.c_double


def source_pose(sequence, metadata, keys, time, normalize):
    values = pose_at(sequence, metadata, keys, time)
    if not sequence["additive_type"]:
        for index in metadata["animation_relative_bones"]:
            values[index] = relative(values[index], metadata["bones"][index], normalize)
    if sequence["forced_root_lock"] == "RefPose":
        values[0] = components(metadata["bones"][0]["mesh_pose"])
    return values


def blend(inputs, weights, normalize):
    poses = zip(*inputs)
    output = []
    for joints in poses:
        rotation, translation, scale = (tuple(x * weights[0] for x in field) for field in joints[0])
        for (child_rotation, child_translation, child_scale), weight in zip(joints[1:], weights[1:]):
            next_rotation = tuple(value * weight for value in child_rotation)
            dot = FMA(rotation[2], next_rotation[2], rotation[0] * next_rotation[0]) + FMA(
                rotation[3], next_rotation[3], rotation[1] * next_rotation[1])
            sign = -1 if dot < 0 else 1
            rotation = tuple(a + sign * b for a, b in zip(rotation, next_rotation))
            translation = tuple(FMA(b, weight, a) for a, b in zip(translation, child_translation))
            scale = tuple(FMA(b, weight, a) for a, b in zip(scale, child_scale))
        if len(inputs) > 1:
            rotation = normalize(rotation)
        output.append((normalize(rotation), translation, scale))
    return output


def early_projection(value):
    rotation, translation, scale = value
    return tuple(map(f32, rotation)), tuple(f32(component * 0.01) * 100.0 for component in translation), tuple(map(f32, scale))


def replay(work):
    metadata, keys = clip_data(ROOT / PACK)
    by_asset = {row["asset"]: row for row in metadata["sequences"]}
    normalize, _ = source_rotations(work)
    native = read_capture(HERE.parent / "d231/blend.native.json.gz")["sync_group_clock_probe"]
    counts = Counter()
    witness = None
    for case in native["samples"]:
        for frame in case["frames"]:
            for player in frame["players"]:
                if player["pose"] is None:
                    continue
                inputs, weights, identities = [], [], []
                for sample in player["samples"]:
                    sequence = by_asset[sample["animation"]]
                    time = min(sample["time"], sequence["duration"])
                    values = source_pose(sequence, metadata, keys, time, normalize)
                    expected = [components(bone["local_transform"]) for bone in sample["pose"]["bones"]]
                    assert values == expected, (case["name"], sample["animation"], time)
                    counts["source_child_bones"] += len(values)
                    inputs.append(values)
                    weights.append(sample["weight"])
                    identities.append({"gltf": sequence["gltf"], "animation": sequence["animation"],
                                       "time": time, "weight": sample["weight"]})
                restored = blend(inputs, weights, normalize)
                projected_early = blend([[early_projection(value) for value in pose] for pose in inputs], weights, normalize)
                for index, (actual, early, bone) in enumerate(zip(restored, projected_early, player["pose"]["bones"])):
                    expected = components(bone["local_transform"])
                    assert actual == expected, (case["name"], bone["name"], actual, expected)
                    late_projection = project(actual)
                    early_projection_value = project(early)
                    counts["source_blend_bones"] += 1
                    counts["early_projection_translation"] += late_projection[1] != early_projection_value[1]
                    counts["early_projection_rotation"] += late_projection[0] != early_projection_value[0]
                    counts["early_projection_scale"] += late_projection[2] != early_projection_value[2]
                    if witness is None and late_projection[1] != early_projection_value[1]:
                        witness = {
                            "path": "/".join(metadata["bone_paths"][index]),
                            "inputs": identities,
                            "late_translation": late_projection[1],
                            "early_translation": early_projection_value[1],
                        }
    assert witness and counts["early_projection_rotation"] > 0
    return {"result": "PASS_BLENDSPACE_SOURCE_DOUBLE", **counts, "witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d232a")
    print(json.dumps(replay(parser.parse_args().work), indent=2))
