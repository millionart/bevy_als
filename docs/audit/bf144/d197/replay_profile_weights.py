import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
request = json.loads((directory / "profiles.request.json").read_bytes())
with gzip.open(directory / "profiles.native.json.gz") as stream:
    native = json.load(stream)["blend_profile_probe"]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def profile_weights(alpha, scale):
    threshold = f32(1e-5)
    source = max(f32(f32(1.0 - alpha) / scale), threshold)
    target = max(f32(alpha * scale), threshold)
    total = f32(source + target)
    if abs(f32(total - 1.0)) > threshold:
        source, target = f32(source / total), f32(target / total)
    return [source, target]


def runtime_pose(position, rotation, scale):
    return {"translation": [position[index] / 100 for index in (0, 2, 1)],
            "rotation": [-rotation[0], -rotation[2], -rotation[1], rotation[3]],
            "scale": [scale[index] for index in (0, 2, 1)]}


def runtime_output(pose):
    return runtime_pose([pose["translation"][axis] for axis in "xyz"],
                        [pose["rotation"][axis] for axis in "xyzw"], [pose["scale"][axis] for axis in "xyz"])


rows = []
legacy_mismatches = 0
scales = {"MoveDirectionChange": 2.0, "QuickFeetBlend": 5.0}
for sample, raw in zip(request["samples"], native["samples"], strict=True):
    assert sample["name"] == raw["name"]
    frame = raw["frames"][0]
    alpha = frame["alpha"]
    scale = scales[sample["profile"]]
    source = runtime_pose(sample["from"]["translation_cm"], sample["from"]["rotation"], sample["from"]["scale"])
    target = runtime_pose(sample["to"]["translation_cm"], sample["to"]["rotation"], sample["to"]["scale"])
    for name, bone in frame["bones"].items():
        weights = [f32(1 - alpha), alpha] if name == "default" else profile_weights(alpha, scale)
        assert weights == [bone["source_weight"], bone["target_weight"]], (sample["name"], name, weights)
        actual = runtime_output(bone["transform"])
        legacy_source = f32(1.0 - weights[1])
        mismatched = False
        for channel in ["translation", "scale"]:
            for first, second, observed in zip(source[channel], target[channel], actual[channel], strict=True):
                assert abs(first * weights[0] + second * weights[1] - observed) < 2e-12
                mismatched |= abs(first * legacy_source + second * weights[1] - observed) > 1e-6
        legacy_mismatches += mismatched
        if name in {"foot_l", "default"}:
            rows.append({"name": sample["name"], "bone": name, "scale": scale, "factor": frame["factor"],
                         "weights": weights, "source": source, "target": target, "expected": actual,
                         "raw_rotation": runtime_output(bone["raw_transform"])["rotation"]})
assert legacy_mismatches > 0 and len(rows) == 20
print(json.dumps({"legacy_mismatches": legacy_mismatches, "cases": rows}, indent=2))
