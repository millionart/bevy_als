import gzip
import json
import math
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def wrap(value, length):
    value = f32(math.fmod(value, length))
    return f32(value + length) if value < 0.0 else value


cases = []
early_end = []
for asset in ["land", "idle"]:
    request = json.loads((directory / f"{asset}.request.json").read_bytes())
    with gzip.open(directory / f"{asset}.native.json.gz") as stream:
        report = json.load(stream)["sequence_clock_probe"]
    duration = report["duration"]
    for sample, result in zip(request["samples"], report["samples"], strict=True):
        assert sample["name"] == result["name"]
        time = legacy = f32(sample["start"])
        rate = f32(f32(sample["rate"]) * report["rate_scale"])
        frames = []
        failures = 0
        for index, (delta, row) in enumerate(zip(sample["deltas"], result["frames"], strict=True)):
            delta = f32(delta)
            movement = f32(delta * rate)
            previous = time
            time = f32(time + movement)
            if time < 0.0 or time > duration:
                time = wrap(time, duration) if sample["looping"] else max(0.0, min(time, duration))
            assert time == row["position"] and previous == row["previous"], (asset, sample["name"], index)
            assert f32(time / duration) == row["ratio"]
            legacy_phase = f32(f32(legacy + movement) / duration)
            legacy_phase = wrap(legacy_phase, 1.0) if sample["looping"] else max(0.0, min(legacy_phase, 1.0))
            legacy = f32(legacy_phase * duration)
            failures += legacy != time
            if not sample["looping"] and legacy == duration and time < duration:
                early_end.append([asset, sample["name"], index, time, duration])
            frames.append([delta, row["previous"], row["position"], row["ratio"]])
        cases.append({"asset": asset, "name": sample["name"], "duration": duration, "rate": rate,
                      "looped": sample["looping"], "frames": frames, "legacy_mismatches": failures})
assert early_end and sum(case["legacy_mismatches"] for case in cases) > 100
print(json.dumps({"cases": cases, "legacy_early_end": early_end}, separators=(",", ":")))
