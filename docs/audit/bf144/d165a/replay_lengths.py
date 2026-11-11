import gzip
import json
import math
import struct
import sys
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def basis(value):
    return value[0], value[2], value[1]


def cm(value):
    return tuple(component * 100.0 for component in basis(value))


def scaled_length(a, b, scale):
    x, y, z = ((x - y) * factor for x, y, factor in zip(a, b, scale, strict=True))
    return math.sqrt(x * x + y * y + z * z)


def joint_offset(distance, lengths):
    upper, lower = lengths
    if distance >= upper + lower:
        return upper, 0.0
    cosine = (upper * upper + distance * distance - lower * lower) / (2.0 * upper * distance)
    perpendicular = upper * math.sin(math.acos(max(-1.0, min(1.0, cosine))))
    return math.sqrt(max(0.0, upper * upper - perpendicular * perpendicular)), perpendicular


directory = Path(__file__).resolve().parent
if sys.argv[1:] == ["--request"]:
    data = json.loads((directory.parent / "476/inputs.json").read_text())
    bones = data["definition"]["bones"]
    samples = []
    for row_index, authored_scale in ((1122, None), (1049, None), (1122, (1.25, 1.125, 0.875))):
        row = data["current"][row_index]
        root = row["bones_before"][0][1]["translation"]
        scale = authored_scale or row["bones_before"][0][1]["scale"]
        initials = [bones[index]["initial_global"] for index, _ in row["bones_before"]]
        name = f"{row_index}/{'nonuniform' if authored_scale else 'actual_scale'}"
        lengths = []
        old_lengths = []
        for bone, (parent, child) in enumerate(zip(initials, initials[1:])):
            ratio = tuple(x / y for x, y in zip(scale, parent["scale"], strict=True))
            a, b, ratio_source = cm(parent["translation"]), cm(child["translation"]), basis(ratio)
            lengths.append(f32(scaled_length(a, b, ratio_source)))
            old_lengths.append(f32(scaled_length(parent["translation"], child["translation"], ratio)))
            samples.append({"name": f"{name}/bone{bone}", "rotation_xyzw": (0, 0, 0, 1),
                            "scaled_distance": {"a": a, "b": b, "scale": ratio_source}})
        correct = sum(lengths) * 0.01
        old = sum(old_lengths)
        assert correct != old
        pole = (root[0], root[1] + 1.0, root[2])
        for label, distance in (("bend", correct * 0.999), ("rounding_boundary", (correct + old) * 0.5),
                                ("extend", correct * 1.001)):
            effector = (root[0] + distance, root[1], root[2])
            samples.append({"name": f"{name}/{label}", "rotation_xyzw": (0, 0, 0, 1),
                            "two_bone_ik": {"root_cm": cm(root), "effector_cm": cm(effector), "pole_cm": cm(pole),
                                            "lengths_cm": lengths, "stretch_ratios": (0.75, 1.25), "stretch": False},
                            "runtime": {"root": root, "effector": effector, "pole": pole, "scale": scale,
                                        "initials": [item["translation"] for item in initials],
                                        "initial_scales": [item["scale"] for item in initials], "old_lengths": old_lengths}})
    print(json.dumps({"samples": samples}, indent=2))
    sys.exit()

requests = json.loads((directory / "lengths.request.json").read_text())["samples"]
report = directory / "lengths.native.json"
data = report.read_bytes() if report.is_file() else gzip.decompress(report.with_suffix(".json.gz").read_bytes())
native = json.loads(data)["geometry_query_probe"]["samples"]
observed_lengths = {}
cases = []
old_failures = []
for request, row in zip(requests, native, strict=True):
    name = request["name"]
    assert name == row["input"]["name"]
    group = name.rsplit("/", 1)[0]
    if "scaled_distance" in request:
        inputs = request["scaled_distance"]
        distance = scaled_length(inputs["a"], inputs["b"], inputs["scale"])
        assert distance == row["scaled_distance"]
        assert f32(distance) == row["scaled_distance_float"]
        observed_lengths.setdefault(group, []).append(row["scaled_distance_float"])
        continue
    assert request["two_bone_ik"]["lengths_cm"] == observed_lengths[group]
    sample = row["two_bone_ik"]
    joint = tuple(sample["joint_cm"][axis] * 0.01 for axis in "xzy")
    end = tuple(sample["effector_cm"][axis] * 0.01 for axis in "xzy")
    runtime = request["runtime"]
    root, effector = runtime["root"], runtime["effector"]
    distance = effector[0] - root[0]
    def solve(lengths):
        along, perpendicular = joint_offset(distance, lengths)
        return root[0] + along, root[1] + perpendicular, root[2]
    new_error = math.dist(joint, solve([value * 0.01 for value in observed_lengths[group]]))
    assert new_error < 1e-8, (name, new_error)
    old_error = math.dist(joint, solve(runtime["old_lengths"]))
    if old_error >= 1e-5:
        old_failures.append(name)
    cases.append({"name": name, **runtime, "joint": joint, "end": end, "old_joint_error_m": old_error})

assert len(cases) == 9
assert all(name.endswith("/rounding_boundary") for name in old_failures) and len(old_failures) == 3
print(json.dumps({"cases": cases, "old_joint_failures": old_failures}, indent=2))
