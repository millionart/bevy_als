import bisect
import json
import math
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d225"))
from replay_raw_rotation import accessor_values, compile_timelines, difference, f32, load_gltf, nlerp, read_capture
from coordinate_basis import source_rotation_to_runtime


def roundtrip(rotation):
    x, y, z, w = source_rotation_to_runtime(rotation)
    singularity = z * x - w * y
    if abs(singularity) > 0.4999995:
        sign = -1.0 if singularity < 0.0 else 1.0
        pitch = sign * 90.0
        yaw = math.degrees(sign * 2.0 * math.atan2(x, w)) % 360.0
        if yaw > 180.0:
            yaw -= 360.0
        roll = 0.0
    else:
        pitch = math.degrees(math.asin(2.0 * singularity))
        yaw = math.degrees(math.atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z)))
        roll = math.degrees(math.atan2(-2.0 * (w * x + y * z), 1.0 - 2.0 * (x * x + y * y)))
    p, y, r = (math.radians(f32(angle)) * 0.5 for angle in (pitch, yaw, roll))
    sp, cp = math.sin(p), math.cos(p)
    sy, cy = math.sin(y), math.cos(y)
    sr, cr = math.sin(r), math.cos(r)
    quaternion = (cr * (sp * sy) - sr * (cp * cy), -cr * (sp * cy) - sr * (cp * sy),
                  cr * (cp * sy) - sr * (sp * cy), cr * (cp * cy) + sr * (sp * sy))
    return source_rotation_to_runtime(quaternion), singularity


def model_inventory():
    models = Counter()
    for group, previous in (("character", "d224d"), ("bow", "d224e")):
        old = read_capture(HERE.parent / previous / f"{group}.native.json.gz")
        source = read_capture(HERE / f"{group}.native.json.gz")
        for row in source["sequences"]:
            models[row.pop("data_model_class")] += 1
        assert old == source
    return dict(models)


def replay():
    assets = ROOT / "assets"
    raw = read_capture(HERE / "raw.native.json.gz")
    prepared = read_capture(HERE / "prepared.native.json.gz")
    assert raw["asset_path"] == prepared["asset_path"]
    clip = raw["asset_path"].rsplit(".", 1)[-1]
    path, animation_name, timeline = next(row for row in compile_timelines(assets) if row[1] == clip + "_0")
    document, buffer = load_gltf(assets / path)
    animation = next(row for row in document["animations"] if row["name"] == animation_name)
    channel = next(row for row in animation["channels"] if row["target"]["path"] == "rotation"
                   and document["nodes"][row["target"]["node"]]["name"] == "ik_foot_r")
    keys = accessor_values(document, buffer, animation["samplers"][channel["sampler"]]["output"])
    rows = []
    for frame, compressed in zip(raw["sequence_pose_probe"]["frames"], prepared["sequence_pose_probe"]["frames"]):
        assert frame["pose_evaluation_mode"] == "raw_model" and compressed["pose_evaluation_mode"] == "compressed"
        assert frame["time"] == compressed["time"]
        time = f32(frame["time"])
        second = min(bisect.bisect_right(timeline, time), len(timeline) - 1)
        first = second - 1
        alpha = f32(f32(time - timeline[first]) / f32(timeline[second] - timeline[first]))
        local = next(bone["local_transform"]["rotation"] for bone in frame["bones"] if bone["name"] == "ik_foot_r")
        native = source_rotation_to_runtime([local[axis] for axis in "xyzw"])
        before = nlerp(keys[first], keys[second], alpha)
        after, singularity = roundtrip(before)
        if alpha <= 1e-8:
            after = before
        rows.append({"time": time, "alpha": alpha, "singularity_test": singularity,
                     "before_error": difference(before, native),
                     "after_error": difference(tuple(f32(value) for value in after), native)})
    witness = next(row for row in rows if row["time"] == f32(0.7756271362304688))
    assert witness["singularity_test"] < -0.4999995
    assert witness["before_error"] > 0.0009 and witness["after_error"] < 2e-7
    assert max(row["after_error"] for row in rows) < 2e-7, rows
    return {"result": "PASS_RAW_SEQUENCER_EULER_ROUNDTRIP", "active_models": model_inventory(),
            "bone": "ik_foot_r", "native_comparison": rows}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
