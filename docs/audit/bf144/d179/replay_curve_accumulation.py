import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def add(values):
    result = 0.0
    for value in values:
        result = f32(result + value)
    return result


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
cutoff = f32(1e-5)
cases = {}
original_failures = []
for stance in ("standing", "crouching"):
    source = load(directory / f"{stance}-turn.native.json")["slot_probe"]
    references = {row["evaluation_positions"][0]: row["curves"]["RotationYawSpeed"] for row in source["frames"][:3]}
    fixture = []
    for row in source["frames"][3:]:
        positions = row["evaluation_positions"]
        weights = list(map(f32, row["evaluation_weights"]))
        total = add(weights)
        assert row["total_weight"] == total and row["source_weight"] == 0.0
        divisor = total if total > f32(1.0 + cutoff) else 1.0
        effective = [f32(weight / divisor) for weight in weights]
        assert all(weight > cutoff for weight in weights)
        values = [f32(references[position] * weight) for position, weight in zip(positions, effective, strict=True)]
        source_curve = row["curves"]["RotationYawSpeed"]
        expected = add(value for index, (value, weight) in enumerate(zip(values, effective, strict=True))
                       if index == 0 or weight > cutoff)
        assert expected == source_curve, (stance, row["frame"], expected, source_curve)
        if add(values) != expected:
            original_failures.append([stance, row["frame"]])
        if row["frame"] == 4:
            assert effective[0] <= cutoff and values[0] != 0.0
            assert add(values[1:]) != source_curve
        fixture.append({"weights": weights, "positions": positions, "effective": effective, "source_curve": source_curve})
    cases[stance] = fixture

assert original_failures == [["standing", 3], ["crouching", 3]]
print(json.dumps({"cases": cases, "old_curve_failures": original_failures}, indent=2))
