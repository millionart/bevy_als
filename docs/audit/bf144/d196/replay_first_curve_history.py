import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "inertia.request.json").read_bytes())
with gzip.open(directory / "inertia.native.json.gz") as stream:
    native = json.load(stream)["inertialization_probe"]
assert native["node_property"] == request["node_property"]
curve = "PoseGrounded"


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def product(*values):
    result = 1.0
    for value in values:
        result = f32(result * value)
    return result


def total(*values):
    result = 0.0
    for value in values:
        result = f32(result + value)
    return result


def blend(target, difference, velocity, elapsed, duration):
    if duration == 0.0:
        return target
    velocity = min(velocity, 0.0)
    if velocity < -f32(1e-4):
        duration = min(duration, f32(product(-5.0, difference) / velocity))
    if elapsed >= f32(duration - f32(1e-7)):
        return target
    t2 = product(duration, duration)
    t3 = product(duration, t2)
    t4 = product(duration, t3)
    t5 = product(duration, t4)
    acceleration = max(f32(total(product(-8, duration, velocity), -product(20, difference)) / t2), 0.0)
    a = f32(product(-0.5, total(product(acceleration, t2), product(6, duration, velocity), product(12, difference))) / t5)
    b = f32(product(0.5, total(product(3, acceleration, t2), product(16, duration, velocity), product(30, difference))) / t4)
    c = f32(product(-0.5, total(product(3, acceleration, t2), product(12, duration, velocity), product(20, difference))) / t3)
    result = a
    for coefficient in [b, c, product(0.5, acceleration), velocity, difference]:
        result = total(product(result, elapsed), coefficient)
    return total(target, result)


cases = []
failures = 0
for sample, observed in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == observed["name"]
    steps = sample["steps"]
    frames = []
    difference = speed = old_speed = duration = elapsed = 0.0
    for index, (step, raw) in enumerate(zip(steps, observed["frames"], strict=True)):
        value = step["curves"].get(curve)
        delta = f32(step["delta_seconds"])
        durations = [f32(step["request_duration"])] if "request_duration" in step else []
        if durations and index > 0:
            previous = steps[index - 1]["curves"].get(curve, 0.0)
            older = steps[index - 2]["curves"].get(curve, 0.0) if index > 1 else previous
            old_older = older if index > 1 else 0.0
            history_delta = f32(steps[index - 1]["delta_seconds"])
            duration = durations[0]
            difference = previous - value
            if history_delta > f32(1e-4):
                speed = f32((difference - value - older) / history_delta)
                old_speed = f32((difference - value - old_older) / history_delta)
        if duration > 0.0:
            elapsed = f32(elapsed + delta)
        output = raw["curves"].get(curve)
        if output is not None:
            actual = blend(value, difference, speed, elapsed, duration)
            old = blend(value, difference, old_speed, elapsed, duration)
            assert output == actual, (sample["name"], index, actual, output)
            failures += abs(output - old) >= 3e-7
        frames.append({"delta_seconds": delta, "request_durations": durations,
                       "input_m": 0.0, "native_m": 0.0, "input_curve": value, "native_curve": output,
                       "expected_duration_seconds": duration, "expected_deficit_seconds": 0.0})
    cases.append({"name": sample["name"], "frames": frames})
assert failures == 4
print(json.dumps({"curve_name": curve, "legacy_mismatches": failures, "cases": cases}, indent=2))
