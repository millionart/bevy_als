import gzip
import json
import math
import struct
import sys
from pathlib import Path


def subtract(a, b):
    return tuple(x - y for x, y in zip(a, b, strict=True))


def scale(vector, factor):
    return tuple(value * factor for value in vector)


def basis(vector):
    x, y, z = vector
    return x, z, y


def length(vector):
    x, y, z = vector
    return math.sqrt(x * x + y * y + z * z)


def direction(delta, distance, minimum):
    return (1.0, 0.0, 0.0) if distance < minimum else scale(delta, 1.0 / distance)


def sample(name, root, effector, pole):
    return {"name": name, "rotation_xyzw": (0, 0, 0, 1),
            "runtime_metres": {"root": root, "effector": effector, "pole": pole},
            "two_bone_ik": {"root_cm": scale(root, 100.0), "effector_cm": scale(effector, 100.0),
                            "pole_cm": scale(pole, 100.0), "lengths_cm": (50, 75),
                            "stretch_ratios": (0.75, 1.25), "stretch": False}}


directory = Path(__file__).resolve().parent
if sys.argv[1:] == ["--request"]:
    data = json.loads((directory.parent / "478/inputs.json").read_text())["rows"]
    rows = {row["row"]: row for row in data if row["domain"] == "current_m"}
    samples = []
    for index, axis in ((1, 1), (157, 2)):
        row = rows[index]
        root = row["root"]
        for label, distance in (("coincident", 0.0), ("below", 0.999e-6), ("boundary", 1e-6), ("above", 1.001e-6)):
            effector = root.copy()
            effector[axis] += distance
            if index == 157 and label == "boundary":
                effector[axis] = math.nextafter(effector[axis], -math.inf)
            samples.append(sample(f"{index}/{label}", root, effector, row["pole"]))
    origin_target = (0.0, math.nextafter(6e-7, -math.inf), math.nextafter(8e-7, math.inf))
    samples.append(sample("origin/at_threshold", (0.0, 0.0, 0.0), origin_target, (0.0, 1.0, 0.0)))
    row = rows[157]
    root = row["root"][:2] + [row["root"][2] + 2621.0]
    pole = row["pole"][:2] + [row["pole"][2] + 2621.0]
    effector = root[:2] + [root[2] + 1e-6]
    samples.append(sample("157/translated_normalization", root, effector, pole))
    print(json.dumps({"samples": samples}, indent=2))
    sys.exit()

requests = json.loads((directory / "goal.request.json").read_text())["samples"]
report = directory / "goal.native.json"
report_data = report.read_bytes() if report.is_file() else gzip.decompress(report.with_suffix(".json.gz").read_bytes())
native = json.loads(report_data)["geometry_query_probe"]["samples"]
cases = []
original_failures = []
projected_failures = []
runtime_normalization_failures = []
threshold_m = 1e-4 * 0.01
assert math.nextafter(1e-4, -math.inf) * 0.01 < threshold_m
normalize_squared = struct.unpack("<f", struct.pack("<f", 1e-8))[0] * 0.0001
for request, result in zip(requests, native, strict=True):
    name = request["name"]
    assert name == result["input"]["name"]
    source, runtime = request["two_bone_ik"], request["runtime_metres"]
    source_delta = subtract(source["effector_cm"], source["root_cm"])
    delta = subtract(runtime["effector"], runtime["root"])
    source_distance = length(source_delta)
    distance = length(delta)
    if name.endswith(("/below", "/coincident")):
        assert source_distance < 1e-4
    elif name.endswith("/above"):
        assert source_distance > 1e-4
    assert (source_distance < 1e-4) == (source_distance * 0.01 < threshold_m)
    old_direction = direction(delta, distance, threshold_m)
    source_direction = direction(source_delta, source_distance, 1e-4)
    source_joint = subtract(source["root_cm"], scale(source_direction, source["lengths_cm"][0]))
    readout = result["two_bone_ik"]
    native_joint = tuple(readout["joint_cm"][axis] for axis in "xyz")
    native_effector = tuple(readout["effector_cm"][axis] for axis in "xyz")
    assert math.dist(source_joint, native_joint) < 1e-12, name
    assert tuple(source["effector_cm"]) == native_effector
    original_joint = subtract(runtime["root"], scale(old_direction, 0.5))
    error = math.dist(original_joint, scale(native_joint, 0.01))
    corrected_direction = source_direction if source_distance < 1e-4 or distance < threshold_m else old_direction
    corrected_joint = subtract(runtime["root"], scale(corrected_direction, 0.5))
    assert math.dist(corrected_joint, scale(native_joint, 0.01)) < 1e-12, name
    if error > 1e-8:
        original_failures.append(name)
    if (length(scale(delta, 100.0)) < 1e-4) != (source_distance < 1e-4):
        projected_failures.append(name)
    if source_distance >= 1e-4 and distance * distance < normalize_squared:
        runtime_normalization_failures.append(name)
        assert math.dist(runtime["root"], scale(native_joint, 0.01)) > 0.49
    cases.append({"name": name, **{key: basis(value) for key, value in runtime.items()},
                  "joint": basis(scale(native_joint, 0.01)), "end": basis(scale(native_effector, 0.01)),
                  "source_distance_cm": source_distance, "old_position_error_m": error})

assert all(name in original_failures and name in projected_failures for name in ("1/boundary", "157/boundary"))
assert next(row["source_distance_cm"] for row in cases if row["name"] == "origin/at_threshold") == 1e-4
assert runtime_normalization_failures == ["157/translated_normalization"]
print(json.dumps({"cases": cases, "original_failures": original_failures,
                  "converting_delta_after_subtraction_failures": projected_failures,
                  "retaining_meter_normalization_failures": runtime_normalization_failures}, indent=2))
