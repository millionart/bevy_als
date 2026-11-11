import gzip
import json
import math
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "inertia.request.json").read_bytes())
with gzip.open(directory / "inertia.native.json.gz") as stream:
    native = json.load(stream)["inertialization_probe"]
assert native["node_property"] == request["node_property"]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def runtime(vector):
    return [f32(vector[index] / 100) for index in (0, 2, 1)]


def decay(value, velocity, time, duration):
    velocity = min(velocity, 0.0)
    if velocity < -1e-4:
        duration = min(duration, -5 * value / velocity)
    if time >= duration - 1e-7:
        return 0.0
    acceleration = max((-8 * duration * velocity - 20 * value) / duration ** 2, 0.0)
    a = -(acceleration * duration ** 2 + 6 * duration * velocity + 12 * value) / (2 * duration ** 5)
    b = (3 * acceleration * duration ** 2 + 16 * duration * velocity + 30 * value) / (2 * duration ** 4)
    c = -(3 * acceleration * duration ** 2 + 12 * duration * velocity + 20 * value) / (2 * duration ** 3)
    return f32(((((a * time + b) * time + c) * time + acceleration / 2) * time + velocity) * time + value)


cases = []
for sample, result in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == result["name"]
    steps = sample["steps"]
    older, previous, target = [step["translation_cm"] for step in steps]
    difference = [prev - dst for prev, dst in zip(previous, target, strict=True)]
    magnitude = f32(math.sqrt(sum(value * value for value in difference)))
    direction = [value * (1.0 / magnitude) for value in difference]
    previous_magnitude = f32(sum((old - dst) * axis for old, dst, axis in zip(older, target, direction, strict=True)))
    history_delta = f32(steps[1]["delta_seconds"])
    speed = f32(f32(magnitude - previous_magnitude) / history_delta) if history_delta > f32(1e-4) else 0.0
    elapsed = f32(steps[2]["delta_seconds"])
    duration = f32(steps[2]["request_duration"])
    offset = decay(magnitude, speed, elapsed, duration)
    projected = runtime([dst + f32(axis) * offset for dst, axis in zip(target, direction, strict=True)])
    native_translation = result["frames"][-1]["transform"]["translation"]
    expected = runtime([native_translation[axis] for axis in "xyz"])
    assert all(abs(actual - source) < 8e-10 for actual, source in zip(projected, expected)), sample["name"]
    zero_offset = decay(magnitude, 0.0, elapsed, duration)
    without_speed = runtime([dst + f32(axis) * zero_offset for dst, axis in zip(target, direction, strict=True)])
    cases.append({"name": sample["name"], "older": runtime(older), "previous": runtime(previous),
                  "target": runtime(target), "history_delta": history_delta, "elapsed": elapsed, "duration": duration,
                  "magnitude_cm": magnitude, "speed_cm_s": speed, "native_m": expected,
                  "without_speed_m": without_speed})
assert all(case["speed_cm_s"] < 0 and abs(case["native_m"][0] - case["without_speed_m"][0]) > 1e-8 for case in cases[:2])
assert cases[2]["speed_cm_s"] == 0.0
print(json.dumps({"cases": cases}, indent=2))
