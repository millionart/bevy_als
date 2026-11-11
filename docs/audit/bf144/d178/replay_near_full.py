import gzip
import json
import math
import sys
from pathlib import Path


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
sys.path.insert(0, str(directory.parent / "d123a"))
from replay_blend import add_shortest, distance, f32, normalize, quat, scale


single_checks = 0
multi_checks = 0
old_weight_mismatches = 0
maximum_curve_delta = 0.0
rows = []
for name in ("standing-turn", "crouching-turn"):
    report = load(directory / f"{name}.native.json")["slot_probe"]
    references = {0.25: report["frames"][0], 0.4: report["frames"][4]}
    for frame in report["frames"]:
        weights = list(map(f32, frame["evaluation_weights"]))
        positions = frame["evaluation_positions"]
        total = f32(sum(weights))
        remaining = f32(1.0 - total)
        base_is_relevant = remaining > f32(1e-5)
        assert frame["slot_weight"] == total and frame["source_weight"] == remaining
        assert frame["source_update_weight"] == (max(remaining, f32(2e-5)) if base_is_relevant else 0.0)
        curve = frame["curves"]["RotationYawSpeed"]
        if len(weights) == 1:
            reference = references[positions[0]]
            layer_curve = reference["curves"]["RotationYawSpeed"]
            assert curve == f32(layer_curve * weights[0])
            if not base_is_relevant:
                for source, actual in zip(reference["bones"], frame["bones"], strict=True):
                    assert source["name"] == actual["name"]
                    for field in ("translation", "scale", "rotation"):
                        value = source["local_transform"][field]
                        assert actual["local_transform"][field] == {key: component * total for key, component in value.items()}
                    single_checks += 1
                if total < 1.0:
                    assert curve != layer_curve
                    old_weight_mismatches += 1
                    maximum_curve_delta = max(maximum_curve_delta, abs(curve - layer_curve))
        else:
            assert not base_is_relevant
            first, second = (references[position]["bones"] for position in positions)
            for a, b, result in zip(first, second, frame["bones"], strict=True):
                raw = add_shortest(scale(quat(a), weights[0]), scale(quat(b), weights[1]))
                assert abs(math.sqrt(sum(value * value for value in quat(result))) - 1.0) < 1e-9
                assert distance(normalize(raw), normalize(quat(result))) < 1e-12
                multi_checks += 1
        rows.append({"slot": report["slot_name"], "positions": positions, "weights": weights,
                     "base_is_relevant": base_is_relevant, "curve": curve})

assert len(rows) == 18 and single_checks == 632 and multi_checks == 158 and old_weight_mismatches == 4
print(json.dumps({"source_samples": rows, "single_bones": single_checks, "multiple_bones": multi_checks,
                  "old_weight_mismatches": old_weight_mismatches, "maximum_old_curve_delta": maximum_curve_delta}, indent=2))
