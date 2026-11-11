import gzip
import json
import math
import struct
import sys
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def basis(q):
    x, y, z, w = q
    return -x, -z, -y, w


def vector_basis(v):
    return v[0], v[2], v[1]


def scale(v, factor):
    return tuple(value * factor for value in v)


def normalize(v):
    return scale(v, 1.0 / math.sqrt(sum(value * value for value in v)))


def product(left, right):
    x, y, z, w = left
    a, b, c, d = right
    return (w * a + x * d + y * c - z * b, w * b - x * c + y * d + z * a,
            w * c + x * b - y * a + z * d, w * d - x * a - y * b - z * c)


def partial(delta, weight):
    x, y, z, w = scale(delta, weight)
    return normalize((x, y, z, w + math.copysign(1.0 - weight, delta[3])))


directory = Path(__file__).resolve().parent
threshold = f32(1.0 - f32(1e-5))
if sys.argv[1:] == ["--request"]:
    recording = json.loads((directory.parent / "451b/replay-inputs.json").read_text())
    frame = recording["frames"][90]
    assert frame["source_frame"] == 90
    head_base, head_delta = (next(bone["rotation"] for bone in frame[pose]["bones"] if bone["name"] == "neck_01")
                             for pose in ("current_base", "current_additive"))
    below = struct.unpack("<f", struct.pack("<I", struct.unpack("<I", struct.pack("<f", threshold))[0] - 1))[0]
    samples = []
    for name, base, delta in (("large_rotation", (0.0, 0.0, 0.0, 1.0), (math.sin(1.2), 0.0, 0.0, math.cos(1.2))),
                              ("head_neck", head_base, head_delta)):
        for label, weight in (("partial", 0.25), ("below", below), ("threshold", threshold),
                              ("near_full", 0.999995), ("full", 1.0)):
            samples.append({"name": f"{name}/{label}", "rotation_xyzw": basis(base), "additive_accumulation": {
                "translation_cm": (10.0, 15.0, 20.0), "scale": (1.0, 1.5, 0.75),
                "layers": [{"rotation_xyzw": basis(delta), "translation_cm": (8.0, -16.0, 4.0),
                            "scale_delta": (0.25, -0.125, 0.5), "weight": f32(weight)}],
            }})
    print(json.dumps({"samples": samples}, indent=2))
    sys.exit()

requests = json.loads((directory / "additive.request.json").read_text())["samples"]
path = directory / "additive.native.json"
native = json.loads(path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes()))
cases = []
old_direction_mismatches = []
maximum_norm_residual = 0.0
for request, row in zip(requests, native["geometry_query_probe"]["samples"], strict=True):
    assert row["input"]["name"] == request["name"]
    name = request["name"]
    inputs = request["additive_accumulation"]
    base, layer = request["rotation_xyzw"], inputs["layers"][0]
    delta, weight = layer["rotation_xyzw"], layer["weight"]
    is_full = weight >= threshold
    weighted = scale(delta, weight) if is_full else partial(delta, weight)
    expected = normalize(product(weighted, base))
    result = row["additive_accumulation"]
    actual = tuple(result["rotation"][axis] for axis in "xyzw")
    actual_direction = normalize(actual)
    maximum_norm_residual = max(maximum_norm_residual, abs(math.sqrt(sum(value * value for value in actual)) - 1.0))
    assert math.dist(actual_direction, expected) < 1e-14, name
    translation = tuple(result["translation"][axis] for axis in "xyz")
    final_scale = tuple(result["scale"][axis] for axis in "xyz")
    assert translation == tuple(a + b * weight for a, b in zip(inputs["translation_cm"], layer["translation_cm"], strict=True))
    assert final_scale == tuple(a * (1.0 + b * weight) for a, b in zip(inputs["scale"], layer["scale_delta"], strict=True))
    old = normalize(product(partial(delta, weight), base))
    if math.dist(actual_direction, old) > 1e-7:
        old_direction_mismatches.append(name)
    cases.append({"name": name, "base": basis(base), "delta": basis(delta), "weight": weight, "full": is_full,
                  "base_translation": vector_basis(scale(inputs["translation_cm"], 0.01)),
                  "delta_translation": vector_basis(scale(layer["translation_cm"], 0.01)),
                  "base_scale": vector_basis(inputs["scale"]), "scale_delta": vector_basis(layer["scale_delta"]),
                  "expected_rotation": basis(actual), "expected_translation": vector_basis(scale(translation, 0.01)),
                  "expected_scale": vector_basis(final_scale)})

assert len(cases) == 10 and len(old_direction_mismatches) == 4
print(json.dumps({"full_weight_threshold": threshold, "old_direction_mismatches": old_direction_mismatches,
                  "maximum_native_norm_residual": maximum_norm_residual,
                  "cases": cases}, indent=2))
