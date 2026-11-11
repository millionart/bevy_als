import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "inertia.request.json").read_bytes())
with gzip.open(directory / "inertia.native.json.gz") as stream:
    native = json.load(stream)["inertialization_probe"]
assert native["node_property"] == request["node_property"]


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


def decay(value, speed, time, duration):
    speed = min(speed, 0.0)
    if speed < -f32(1e-4):
        duration = min(duration, f32(product(-5.0, value) / speed))
    if time >= f32(duration - f32(1e-7)):
        return 0.0
    t2 = product(duration, duration)
    t3 = product(duration, t2)
    t4 = product(duration, t3)
    t5 = product(duration, t4)
    a0 = max(f32(total(product(-8.0, duration, speed), -product(20.0, value)) / t2), 0.0)
    a = f32(product(-0.5, total(product(a0, t2), product(6.0, duration, speed), product(12.0, value))) / t5)
    b = f32(product(0.5, total(product(3.0, a0, t2), product(16.0, duration, speed), product(30.0, value))) / t4)
    c = f32(product(-0.5, total(product(3.0, a0, t2), product(12.0, duration, speed), product(20.0, value))) / t3)
    result = a
    for coefficient in [b, c, product(0.5, a0), speed, value]:
        result = total(product(result, time), coefficient)
    return result


cases = []
for sample, observed in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == observed["name"]
    steps = sample["steps"]
    previous, magnitude = [f32(step["translation_cm"][0] / 100) for step in steps[:2]]
    speed = f32(f32(magnitude - previous) / f32(steps[1]["delta_seconds"]))
    duration = f32(steps[2]["request_duration"])
    elapsed = 0.0
    rows = []
    for step, frame in zip(steps[2:], observed["frames"][2:], strict=True):
        elapsed = total(elapsed, f32(step["delta_seconds"]))
        expected = f32(frame["transform"]["translation"]["x"] / 100)
        fixed = f32(decay(product(magnitude, 100), product(speed, 100), elapsed, duration) / 100)
        old = decay(magnitude, speed, elapsed, duration)
        tolerance = 1e-11 if sample["name"] == "centimeter_stop_window" else 1e-9
        assert abs(fixed - expected) <= tolerance, (sample["name"], elapsed, fixed, expected)
        rows.append({"elapsed_seconds": elapsed, "translation_m": expected, "legacy_m": old})
    cases.append({"name": sample["name"], "magnitude_m": magnitude, "speed_mps": speed,
                  "duration_seconds": duration, "frames": rows})

stop, meter = cases
assert stop["frames"][0]["translation_m"] > 0 and stop["frames"][0]["legacy_m"] < 0
rounding_failures = sum(frame["translation_m"] != frame["legacy_m"] for frame in meter["frames"])
assert rounding_failures > 0
print(json.dumps({"cases": cases, "meter_rounding_failures": rounding_failures}, indent=2))
