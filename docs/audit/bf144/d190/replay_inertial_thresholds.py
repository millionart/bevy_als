import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "inertia.request.json").read_bytes())
with gzip.open(directory / "inertia.native.json.gz") as stream:
    native = json.load(stream)["inertialization_probe"]
assert native["node_property"] == request["node_property"]
assert native["enabled"] and not native["ignore_velocity"]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


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
    return a * time ** 5 + b * time ** 4 + c * time ** 3 + acceleration * time ** 2 / 2 + velocity * time + value


def output(steps, channel, distance_gate, time_gate):
    factor = 0.01 if channel == "translation_cm" else 1.0
    previous_previous, previous, target = [f32(step[channel][0] * factor) for step in steps]
    magnitude = f32(previous - target)
    if magnitude <= distance_gate:
        return target
    dt = f32(steps[1]["delta_seconds"])
    speed = f32((magnitude - f32(previous_previous - target)) / dt) if dt > time_gate else 0.0
    return target + decay(magnitude, speed, steps[2]["delta_seconds"], steps[2]["request_duration"])


cases = []
ablation_failures = {"translation_distance": 0, "scale_distance": 0, "history_delta": 0}
for sample, observed in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == observed["name"]
    pose = observed["frames"][-1]["transform"]
    expected = [pose["translation"]["x"] / 100, pose["scale"]["x"]]
    results = []
    for channel, threshold, source, tolerance in zip(
        ["translation_cm", "scale"], [1e-6, 1e-4], expected, [2e-9, 2e-7], strict=True
    ):
        fixed = output(sample["steps"], channel, f32(threshold), f32(1e-4))
        old = output(sample["steps"], channel, 2 ** -23, 2 ** -23)
        assert abs(fixed - source) < tolerance, (sample["name"], channel, fixed, source)
        distance_key = "translation_distance" if channel == "translation_cm" else "scale_distance"
        ablation_failures[distance_key] += abs(output(sample["steps"], channel, 2 ** -23, f32(1e-4)) - source) >= tolerance
        ablation_failures["history_delta"] += abs(output(sample["steps"], channel, f32(threshold), 2 ** -23) - source) >= tolerance
        results.append(abs(old - source) >= tolerance)
    cases.append({"name": sample["name"], "translation_m": expected[0], "scale": expected[1],
                  "legacy_failed": any(results)})
assert sum(case["legacy_failed"] for case in cases) == 5
assert ablation_failures == {"translation_distance": 2, "scale_distance": 1, "history_delta": 2}
print(json.dumps({"legacy_failures": 5, "ablation_failures": ablation_failures, "cases": cases}, indent=2))
