import json
import math
from pathlib import Path
import struct


def quat(bone):
    q = bone["local_transform"]["rotation"]
    return tuple(q[axis] for axis in ("x", "y", "z", "w"))


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def scale(q, weight):
    return tuple(x * weight for x in q)


def add_shortest(a, b):
    sign = -1 if dot(a, b) < 0 else 1
    return tuple(x + y * sign for x, y in zip(a, b))


def normalize(q):
    return scale(q, 1.0 / math.sqrt(dot(q, q)))


def distance(a, b):
    sign = -1 if dot(a, b) < 0 else 1
    return math.sqrt(sum((x - y * sign) ** 2 for x, y in zip(a, b)))


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def normalize_f32(q):
    squares = [f32(value * value) for value in q]
    length = f32(math.sqrt(f32(f32(squares[0] + squares[1]) + f32(squares[2] + squares[3]))))
    scale = f32(1.0 / length)
    return tuple(f32(value * scale) for value in q)


def verify(path):
    native = json.loads(path.read_text())
    samples = native["blend_list_probe"]["samples"]
    result = {"bones":0, "triple_bones":0, "early_normalization_failures":0,
              "f32_second_normalization_changes":0, "max_native_orientation_error":0.0,
              "max_native_norm_error":0.0, "max_early_error":0.0}
    for sample in samples:
        for frame in sample["frames"]:
            weights = list(frame["weights"])
            triple = all(weight > 0 for weight in weights)
            active = [index for index, weight in enumerate(weights) if weight > 0]
            if len(active) == 2:
                weights[active[1]] = f32(1.0 - weights[active[0]])
            for index, bone in enumerate(frame["pose"]["bones"]):
                weighted = [scale(quat(pose["bones"][index]), weight)
                            for pose, weight in zip(frame["input_poses"], weights)]
                native_quat = quat(bone)
                native_direction = normalize(native_quat)
                raw_pair = add_shortest(weighted[0], weighted[1])
                reference = normalize(add_shortest(raw_pair, weighted[2]))
                result["max_native_orientation_error"] = max(result["max_native_orientation_error"], distance(native_direction, reference))
                result["max_native_norm_error"] = max(result["max_native_norm_error"], abs(dot(native_quat, native_quat) - 1.0))
                result["bones"] += 1
                if triple:
                    result["triple_bones"] += 1
                    early_pair = scale(normalize(raw_pair), weights[0] + weights[1])
                    error = distance(native_direction, normalize(add_shortest(early_pair, weighted[2])))
                    result["max_early_error"] = max(result["max_early_error"], error)
                    result["early_normalization_failures"] += error > 1e-6
                    projected = normalize_f32(tuple(f32(value) for value in reference))
                    result["f32_second_normalization_changes"] += normalize_f32(projected) != projected
    assert result["max_native_orientation_error"] < 1e-12, result
    assert result["triple_bones"] > 0 and result["early_normalization_failures"] == result["triple_bones"]
    assert result["f32_second_normalization_changes"] > 0
    return result


if __name__ == "__main__":
    print(json.dumps(verify(Path(__file__).with_name("blend.native.json")), indent=2))
