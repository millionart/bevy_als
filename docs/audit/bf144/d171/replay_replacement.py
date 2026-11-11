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


def cubic(elapsed, duration):
    alpha = min(1.0, f32(elapsed / duration))
    return f32(f32(alpha * alpha) * f32(3.0 - f32(2.0 * alpha)))


def weights(steps, always_restart):
    elapsed = 0.0
    duration = f32(0.2)
    stopped = False
    start = 0.0
    current = 0.0
    result = []
    for step in steps:
        requested = step.get("stop_seconds")
        if requested is not None:
            requested = f32(requested)
            if not stopped or requested < duration or always_restart:
                stopped = True
                start = current
                elapsed = 0.0
                duration = requested
        elapsed = f32(elapsed + f32(step["delta_seconds"]))
        eased = cubic(elapsed, duration)
        current = f32(start * f32(1.0 - eased)) if stopped else eased
        result.append(current)
    return result


directory = Path(__file__).resolve().parent
skeleton = load(directory / "groups.native.json")
groups = next(value["exported_text"] for value in skeleton["asset_properties"] if value["name"] == "SlotGroups")
grounded = re.search(r'GroupName="Grounded",SlotNames=\(([^)]+)\)', groups)
assert grounded and set(re.findall(r'"([^"]+)"', grounded[1])) == {"Transition", "TurnInPlaceStanding", "TurnInPlaceCrouching"}

cases = []
original_mismatches = 0
for stance in ("standing", "crouching"):
    request = load(directory / f"{stance}.request.json")
    native = load(directory / f"{stance}.native.json")["montage_clock_probe"]["samples"]
    for input_case, source_case in zip(request["samples"], native, strict=True):
        name = input_case["name"]
        assert name == source_case["name"]
        expected = [frame["weight"] for frame in source_case["frames"]]
        steps = input_case["steps"]
        assert weights(steps, always_restart=False) == expected
        original = weights(steps, always_restart=True)
        original_mismatches += sum(left != right for left, right in zip(original, expected, strict=True))
        if name == "equal":
            assert original[3:] == [0.1318359375, 0.078125, 0.0244140625]
            assert expected[3:] == [0.0, 0.0, 0.0]
        if name == "shorter":
            assert expected[3] == 0.078125
        cases.append({"name": name, "crouching": stance == "crouching",
                      "frames": [{**step, "weight": value} for step, value in zip(steps, expected, strict=True)]})

assert len(cases) == 8 and original_mismatches == 12
print(json.dumps({"cases": cases, "old_restart_mismatches": original_mismatches}, indent=2))
