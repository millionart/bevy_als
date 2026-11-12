import bisect
import json
import math
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / "tools/import"))
from audit_animation_transform_conversion import accessor_values, load_gltf
from capture_als_sequence_compression import read_capture, sequence_inventory
from compile_als_event_tracks import f32
from compile_als_raw_rotation_timelines import compile_timelines
from coordinate_basis import source_rotation_to_runtime


def normalized(values):
    length = math.sqrt(sum(value * value for value in values))
    return tuple(value / length for value in values)


def nlerp(start, end, alpha):
    if alpha <= 1e-5:
        return tuple(start)
    if abs(f32(alpha - 1.0)) <= 1e-5:
        return tuple(end)
    bias = -1 if sum(a * b for a, b in zip(start, end)) < 0 else 1
    return normalized([a * bias * (1.0 - alpha) + b * alpha for a, b in zip(start, end)])


def slerp(start, end, alpha):
    dot = sum(a * b for a, b in zip(start, end))
    if dot < 0:
        end = tuple(-value for value in end)
        dot = -dot
    if dot > 0.9999:
        return normalized([a * (1.0 - alpha) + b * alpha for a, b in zip(start, end)])
    angle = math.acos(dot)
    first = math.sin((1.0 - alpha) * angle) / math.sin(angle)
    second = math.sin(alpha * angle) / math.sin(angle)
    return tuple(a * first + b * second for a, b in zip(start, end))


def difference(a, b):
    sign = -1 if sum(x * y for x, y in zip(a, b)) < 0 else 1
    return max(abs(x - sign * y) for x, y in zip(a, b))


def replay():
    assets = ROOT / "assets"
    timelines = compile_timelines(assets)
    installed = json.loads((assets / "ALSRuntime/Animations/als_raw_rotation_timelines.json").read_bytes())
    assert installed == json.loads(json.dumps(timelines))
    assert len(sequence_inventory(assets)) == 128 and len(timelines) == 109
    report = read_capture(HERE / "sprint-raw.native.json.gz")
    source = report["asset_path"].rsplit(".", 1)[-1]
    path, animation_name, timeline = next(row for row in timelines if row[1] == source + "_0")
    document, buffer = load_gltf(assets / path)
    animation = next(row for row in document["animations"] if row["name"] == animation_name)
    channel = next(row for row in animation["channels"] if row["target"]["path"] == "rotation"
                   and document["nodes"][row["target"]["node"]]["name"] == "ik_hand_l")
    sampler = animation["samplers"][channel["sampler"]]
    rotations = accessor_values(document, buffer, sampler["output"])
    rows = []
    for frame in report["sequence_pose_probe"]["frames"]:
        assert frame["pose_evaluation_mode"] == "raw_model"
        time = f32(frame["time"])
        end = min(bisect.bisect_right(timeline, time), len(timeline) - 1)
        start = end - 1
        alpha = f32(f32(time - timeline[start]) / f32(timeline[end] - timeline[start]))
        bones = next(bone for bone in frame["bones"] if bone["name"] == "ik_hand_l")
        rotation = bones["local_transform"]["rotation"]
        expected = source_rotation_to_runtime([rotation[axis] for axis in "xyzw"])
        nlerped = tuple(f32(value) for value in nlerp(rotations[start], rotations[end], alpha))
        slerped = slerp(rotations[start], rotations[end], alpha)
        rows.append({"time": time, "alpha": alpha, "nlerp_error": difference(nlerped, expected),
                     "slerp_error": difference(slerped, expected)})
    assert max(row["nlerp_error"] for row in rows) < 2e-7
    witness = next(row for row in rows if row["time"] == f32(0.3595596254))
    assert witness["slerp_error"] > 0.001
    a = (0.0, 0.0, 0.0, 1.0)
    b = (0.0, 0.0, 0.0, -1.0)
    assert nlerp(a, b, 0.4) == b
    assert nlerp(a, b, 0.00001) == a
    assert nlerp(a, b, 1.0) == b
    return {"result": "PASS_RAW_SEQUENCE_NLERP", "linear_timelines": len(timelines), "step_sequences": 19,
            "bone": "ik_hand_l", "raw_native_comparison": rows}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
