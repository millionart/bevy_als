import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
cutoff = f32(1e-5)
rows = []
for name, slot in (("transition", "Transition"), ("standing-turn", "TurnInPlaceStanding"),
                   ("crouching-turn", "TurnInPlaceCrouching")):
    request = load(directory / f"{name}.request.json")
    source = load(directory / f"{name}.native.json")["slot_probe"]
    assert source["slot_name"] == slot and source["node_guid"] == request["node_guid"]
    base = source["frames"][0]
    for step, row in zip(request["steps"], source["frames"], strict=True):
        weight = row["montage_weight"]
        eligible = weight > cutoff
        assert row["slot_weight"] == row["total_weight"] == (weight if eligible else 0.0)
        allow = f32(step["curves"]["AllowTransitions"])
        if not eligible:
            assert row["bones"] == base["bones"]
            assert row["source_weight"] == row["source_update_weight"] == 1.0
            assert row["curves"]["AllowTransitions"] == allow
            assert row["curves"]["RotationYawSpeed"] == 0.0
        else:
            expected = f32(allow - weight) if name == "transition" else f32(allow * f32(1.0 - weight))
            assert row["curves"]["AllowTransitions"] == expected
        rows.append({"slot": slot, "frame": row["frame"], "montage_weight": weight,
                     "submitted": eligible, "slot_weight": row["slot_weight"],
                     "allow_transitions": row["curves"]["AllowTransitions"],
                     "rotation_yaw_speed": row["curves"]["RotationYawSpeed"]})

assert len(rows) == 15 and sum(row["submitted"] for row in rows) == 6
threshold = f32(1.0 - cutoff)
base_curve = f32(0.999992)
assert base_curve >= threshold and f32(base_curve - f32(3e-6)) < threshold
cases = []
for raw in ((3e-6,), (1e-5,), (9e-6, 9e-6), (1.1e-5,), (9e-6, 0.25)):
    weights = list(map(f32, raw))
    eligible = [weight for weight in weights if weight > cutoff]
    cases.append({"weights": weights, "eligible": len(eligible), "total": f32(sum(eligible))})
assert cases[2]["total"] == 0.0 and f32(sum(cases[2]["weights"])) > cutoff
print(json.dumps({"native_frames": rows, "cases": cases}, indent=2))
