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


cases = []
legacy_mismatches = 0
for sample, raw in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == raw["name"]
    elapsed = duration = 0.0
    frames = []
    for index, (step, frame) in enumerate(zip(sample["steps"], raw["frames"], strict=True)):
        delta = f32(step["delta_seconds"])
        requests = []
        if "request_duration" in step:
            duration = f32(step["request_duration"])
            requests = [duration]
            elapsed = 0.0
        elapsed = f32(elapsed + delta)
        native_translation = f32(frame["transform"]["translation"]["x"] / 100)
        native_curve = frame["curves"]["InertialProbeCurve"]
        if index >= 2:
            expected = duration > 0.0 and elapsed < f32(duration - f32(1e-7))
            assert (native_translation > 0.0) == (native_curve > 0.0) == expected
            legacy = duration > 2 ** -23 and elapsed < f32(duration - f32(1e-7))
            legacy_mismatches += expected != legacy
        frames.append({"delta_seconds": delta, "request_durations": requests,
                       "input_m": f32(step["translation_cm"][0] / 100), "native_m": native_translation,
                       "input_curve": step["curves"]["InertialProbeCurve"], "native_curve": native_curve,
                       "expected_duration_seconds": duration, "expected_deficit_seconds": 0.0})
    cases.append({"name": sample["name"], "frames": frames})
assert len(cases) == 18 and legacy_mismatches == 13
print(json.dumps({"cases": cases, "legacy_mismatches": legacy_mismatches}, indent=2))
