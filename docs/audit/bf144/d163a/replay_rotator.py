import json
import math
from pathlib import Path


def axis(value):
    value = math.fmod(value, 360.0)
    if value < 0.0:
        value += 360.0
    return value - 360.0 if value > 180.0 else value


def source_rotator(q):
    x, y, z, w = q
    singularity = z * x - w * y
    if abs(singularity) > 0.4999995:
        sign = math.copysign(1.0, singularity)
        return 0.0, -axis(math.degrees(sign * 2.0 * math.atan2(x, w))), sign * 90.0
    return (
        math.degrees(math.atan2(-2.0 * (w * x + y * z), 1.0 - 2.0 * (x * x + y * y))),
        -math.degrees(math.atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z))),
        math.degrees(math.asin(2.0 * singularity)),
    )


def runtime_algebra(q):
    x, y, z, w = q
    singularity = x * y + w * z
    if abs(singularity) > 0.4999995:
        sign = math.copysign(1.0, singularity)
        return 0.0, -axis(math.degrees(-sign * 2.0 * math.atan2(x, w))), sign * 90.0
    return (
        math.degrees(math.atan2(2.0 * (w * x - y * z), 1.0 - 2.0 * (x * x + z * z))),
        -math.degrees(math.atan2(2.0 * (x * z - w * y), 1.0 - 2.0 * (z * z + y * y))),
        math.degrees(math.asin(2.0 * singularity)),
    )


directory = Path(__file__).resolve().parent
requests = json.loads((directory / "rotator.request.json").read_text())["samples"]
samples = json.loads((directory / "rotator.native.json").read_text())["geometry_query_probe"]["samples"]
rows = []
for request, sample in zip(requests, samples, strict=True):
    assert request["name"] == sample["input"]["name"]
    native = sample["rotator_degrees"]
    expected = native["roll"], -native["yaw"], native["pitch"]
    x, y, z, w = map(float, request["rotation_xyzw"])
    runtime = (-x, -z, -y, w)
    assert math.dist(source_rotator((x, y, z, w)), expected) < 1e-12
    original = runtime_algebra(runtime)
    rows.append({"case": request["name"], "runtime_quaternion": runtime, "native_euler": expected,
                 "runtime_algebra_euler": original})
failures = [row["case"] for row in rows if math.dist(row["native_euler"], row["runtime_algebra_euler"]) > 1e-6]
assert failures and all(name.startswith("half_turn") for name in failures)
first = next(row for row in rows if row["case"] == "half_turn_0_-1_1")
assert first["native_euler"][0] == -180.0 and first["runtime_algebra_euler"][0] == 180.0
target = -177.4759
limit = lambda angle: min(max(target, min(0.0, angle)), max(0.0, angle))
assert limit(first["native_euler"][0]) == target and limit(first["runtime_algebra_euler"][0]) == 0.0
print(json.dumps({"cases": rows, "runtime_algebra_failures": failures}, indent=2))
