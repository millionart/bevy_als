import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def sum_f32(values):
    result = 0.0
    for value in values:
        result = f32(result + value)
    return result


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
threshold = f32(1.0 + f32(1e-5))
cases = {}
original_mismatches = []
for name in ("transition", "standing-turn", "crouching-turn"):
    source = load(directory / f"{name}.native.json")["slot_probe"]
    request = load(directory / f"{name}.request.json")
    full = source["frames"][0]
    curve_name = "AllowTransitions" if name == "transition" else "RotationYawSpeed"
    base_curve = f32(request["steps"][0]["curves"][curve_name])
    additive = name == "transition"
    layer = f32(full["curves"][curve_name] - base_curve) if additive else full["curves"][curve_name]
    assert layer != 0.0
    assert not additive or layer == -1.0
    output = []
    for step, row in zip(request["steps"], source["frames"], strict=True):
        assert row["evaluation_weights"] == step["evaluation_weights"]
        weights = list(map(f32, row["evaluation_weights"]))
        total = sum_f32(weights)
        assert row["total_weight"] == total and row["slot_weight"] == min(1.0, total)
        expected_source_weight = 1.0 if additive else f32(1.0 - min(1.0, total))
        assert row["source_weight"] == expected_source_weight
        divisor = total if total > threshold else 1.0
        effective = [f32(weight / divisor) for weight in weights]
        if additive:
            value = sum_f32([base_curve, *(f32(layer * weight) for weight in effective)])
        else:
            value = sum_f32(f32(layer * weight) for weight in effective)
            root_scale = row["bones"][0]["local_transform"]["scale"]["x"]
            full_scale = full["bones"][0]["local_transform"]["scale"]["x"]
            base_scale = step["bone_transforms"][0]["scale"][0]
            expected_scale = full_scale * sum(effective) + base_scale * expected_source_weight
            assert abs(root_scale - expected_scale) < 1e-12
        assert value == row["curves"][curve_name], (name, total, value, row["curves"][curve_name])
        original = [f32(weight / max(total, 1.0)) for weight in weights]
        if original != effective:
            assert 1.0 < total <= threshold
            original_mismatches.append([name, row["frame"]])
        output.append({"weights": weights, "effective": effective, "slot_weight": row["slot_weight"],
                       "source_curve": value, "base_curve": base_curve, "layer_curve": layer})
    assert len(output) == 6
    cases[name] = output

assert original_mismatches == [[name, index] for name in cases for index in (2, 3)]
default = load(directory / "default-slot.native.json")["slot_probe"]
previous = load(directory.parent / "d168/transition.native.json")["slot_probe"]
assert default == previous
print(json.dumps({"threshold": threshold, "original_mismatches": original_mismatches,
                  "default_slot_probe_matches": True, "cases": cases}, indent=2))
