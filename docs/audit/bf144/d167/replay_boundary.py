import gzip
import json
import re
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


def frames(duration, steps, rate):
    position = 0.0
    stop_elapsed = None
    playing = True
    result = []
    for step in steps:
        delta = f32(step["delta_seconds"])
        weight = 1.0
        if stop_elapsed is not None:
            stop_elapsed = f32(stop_elapsed + delta)
            alpha = min(1.0, f32(stop_elapsed / f32(0.2)))
            weight = 1.0 - alpha * alpha * (3.0 - 2.0 * alpha)
        if playing:
            remaining = f32(duration - position)
            movement = f32(delta * rate)
            reached = movement >= remaining
            position = f32(position + min(movement, remaining))
            if stop_elapsed is None and f32(f32(duration - position) / rate) <= f32(1e-4):
                stop_elapsed = 0.0
            if reached:
                position = f32(duration - f32(0.00005))
                playing = False
        result.append({"delta_seconds": delta, "position": position, "weight": weight,
                       "playing": playing, "stopped": stop_elapsed is not None})
    return result


directory = Path(__file__).resolve().parent
root = directory.parents[3]
request = load(directory / "boundary.request.json")["samples"]
montages = (root / "assets/ALS/Data/AnimationInstance/Grounded/als_standing_montages.ron").read_text()
cases = []
failures = 0
maximum_weight_error = 0.0
for stance in ("stand", "crouch"):
    for side in ("left", "right"):
        sequence = f"A_Als_{stance.title()}_DynamicTransition_{side.title()}"
        native = load(directory / f"{stance}-{side}.native.json")
        assert native["asset_path"].endswith(f"/{sequence}.{sequence}")
        probe = native["montage_clock_probe"]
        length = next(value["numeric_value"] for value in native["asset_properties"] if value["name"] == "SequenceLength")
        assert length == probe["montage_length"] == 1.5333333015441895
        match = re.search(rf'^"{sequence}": \(clip: "[^"\n]+", duration: ([\d.]+),', montages, re.M)
        assert match is not None and f32(float(match[1])) == length
        samples = []
        for sample, native_sample in zip(request, probe["samples"], strict=True):
            assert sample["name"] == native_sample["name"]
            expected = [{key: value for key, value in frame.items() if key != "valid"} for frame in native_sample["frames"]]
            actual = frames(length, sample["steps"], sample["play_rate"])
            assert actual == expected, (sequence, sample["name"], actual, expected)
            original = frames(f32(1.533333), sample["steps"], sample["play_rate"])
            if sample["name"] == "rounding_boundary":
                assert original[1]["stopped"] and not expected[1]["stopped"]
                failures += 1
                maximum_weight_error = max(maximum_weight_error, max(
                    abs(a["weight"] - b["weight"]) for a, b in zip(original, expected, strict=True)))
            else:
                assert [frame["stopped"] for frame in original] == [frame["stopped"] for frame in expected]
            samples.append({"name": sample["name"], "frames": expected})
        cases.append({"sequence": sequence, "crouching": stance == "crouch", "right": side == "right",
                      "length": length, "samples": samples})

assert failures == 4 and 0.059 < maximum_weight_error < 0.060
print(json.dumps({"cases": cases, "truncated_duration_ablations_failed": failures,
                  "maximum_weight_error": maximum_weight_error}, indent=2))
