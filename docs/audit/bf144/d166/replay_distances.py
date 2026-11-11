import gzip
import json
import math
import struct
import sys
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def cm(point):
    return [f32(point[index]) * 100.0 for index in (0, 2, 1)]


def squared(delta):
    x, y, z = delta
    return x * x + y * y + z * z


def select(distances, threshold, amounts):
    left, right = [weight > f32(1e-5) and distance > threshold
                   for weight, distance in zip(amounts, distances, strict=True)]
    if not (left or right):
        return None
    return "right" if not left or (right and distances[0] < distances[1]) else "left"


directory = Path(__file__).resolve().parent
if sys.argv[1:] == ["--request"]:
    left = [0.1, 0.0, 0.0]
    right = [0.1, 0.0, 1e-6]
    zero = [0.0, 0.0, 0.0]
    samples = []
    for name, stance, targets, amounts, scale in (
        ("standing_rank", "standing", [left, right], [1.0, 1.0], 1.0),
        ("crouching_rank", "crouching", [left, right], [1.0, 1.0], 1.0),
        ("equal", "standing", [left, left], [1.0, 1.0], 1.0),
        ("30_degrees_below", "standing", [[0.06928203254938126, 0.0, 0.04], zero], [1.0, 0.0], 1.0),
        ("10_degrees_above", "crouching", [zero, [0.07878462225198746, 0.0, 0.01389185432344675]], [0.0, 1.0], 1.0),
        ("scale_blocks", "standing", [left, right], [1.0, 1.0], 1.5),
        ("source_weight_boundary", "crouching", [left, right], [1.0, 1e-5], 1.0),
    ):
        for index, target in enumerate(targets):
            samples.append({"name": name, "foot": index, "rotation_xyzw": [0, 0, 0, 1],
                            "stance": stance, "scale_world_space": f32(scale), "lock_amount": f32(amounts[index]),
                            "target_m": list(map(f32, target)), "lock_m": zero,
                            "scaled_distance": {"a": cm(target), "b": cm(zero), "scale": [1, 1, 1]}})
    print(json.dumps({"samples": samples}, indent=2))
    sys.exit()

requests = json.loads((directory / "distance.request.json").read_text())["samples"]
report = directory / "distance.native.json"
data = report.read_bytes() if report.is_file() else gzip.decompress(report.with_suffix(".json.gz").read_bytes())
native = json.loads(data)
settings = next(value for value in native["asset_properties"] if value["name"] == "DynamicTransitions")
properties = {field["name"]: field for field in settings["fields"]}
threshold_cm = properties["FootLockDistanceThreshold"]["numeric_value"]
assert threshold_cm == 8.0
blend = properties["BlendDuration"]["numeric_value"]
rate = properties["PlayRate"]["numeric_value"]
assert f32(blend) == f32(0.2) and rate == 1.5
for name, stance in (("Standing", "Stand"), ("Crouching", "Crouch")):
    for side in ("Left", "Right"):
        assert f"A_Als_{stance}_DynamicTransition_{side}" in properties[f"{name}{side}Sequence"]["exported_text"]

cases = []
old_failures = []
native_samples = native["geometry_query_probe"]["samples"]
for start in range(0, len(requests), 2):
    pair = requests[start:start + 2]
    readouts = native_samples[start:start + 2]
    distances = []
    old_distances = []
    for request, readout in zip(pair, readouts, strict=True):
        assert request == readout["input"]
        inputs = request["scaled_distance"]
        delta = [a - b for a, b in zip(inputs["a"], inputs["b"], strict=True)]
        distance = squared(delta)
        assert math.sqrt(distance) == readout["scaled_distance"]
        distances.append(distance)
        dx, dy, dz = [f32(a - b) for a, b in zip(request["target_m"], request["lock_m"], strict=True)]
        old_distances.append(f32(f32(f32(dx * dx) + f32(dy * dy)) + f32(dz * dz)))
    scale = pair[0]["scale_world_space"]
    amounts = [value["lock_amount"] for value in pair]
    limit_cm = f32(f32(threshold_cm) * scale)
    limit_m = f32(f32(threshold_cm * 0.01) * scale)
    expected = select(distances, f32(limit_cm * limit_cm), amounts)
    old = select(old_distances, f32(limit_m * limit_m), amounts)
    name = pair[0]["name"]
    if expected != old:
        old_failures.append(name)
    cases.append({"name": name, "stance": pair[0]["stance"], "scale": scale, "amounts": amounts,
                  "targets": [value["target_m"] for value in pair], "locks": [value["lock_m"] for value in pair],
                  "source_distances_squared_cm": distances, "expected": expected, "original": old})

assert old_failures == ["standing_rank", "crouching_rank", "30_degrees_below"]
assert select([64.0, 64.0], 64.0, [1.0, 1.0]) is None
print(json.dumps({"cases": cases, "original_failures": old_failures}, indent=2))
