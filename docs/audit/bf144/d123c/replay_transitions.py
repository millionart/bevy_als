import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / "d123a"))
from replay_blend import add_shortest, distance, dot, normalize, scale


def rotation(transform):
    return tuple(transform["rotation"][axis] for axis in "xyzw")


def read(name):
    return json.loads((ROOT / name).read_text())


def verify_layer(request, native):
    source = request["samples"]
    results = native["blend_profile_probe"]["samples"]
    assert len(source) == len(results)
    observed = {}
    for sample, native in zip(source, results, strict=True):
        assert sample["name"] == native["name"]
        frame, = native["frames"]
        for name, bone in frame["bones"].items():
            weighted_a = scale(sample["from"]["rotation"], bone["source_weight"])
            weighted_b = scale(sample["to"]["rotation"], bone["target_weight"])
            raw = add_shortest(weighted_a, weighted_b)
            assert distance(raw, rotation(bone["raw_transform"])) < 1e-13, (sample["name"], name)
            final = normalize(rotation(bone["transform"]))
            assert distance(normalize(raw), final) < 1e-13, (sample["name"], name)
            if name == "default":
                observed[sample["name"]] = {"raw":raw, "final":final,
                                             "raw_norm_squared":dot(raw, raw), "bones":len(frame["bones"])}
    return observed


if __name__ == "__main__":
    first = verify_layer(read("first.request.json"), read("first.native.json"))["first_layer"]
    second = verify_layer(read("second.request.json"), read("second.native.json"))
    mixed_error = distance(second["raw_transform_mixed"]["final"], second["transform_mixed"]["final"])
    zero_error = distance(second["raw_transform_zero"]["final"], second["transform_zero"]["final"])
    assert first["raw_norm_squared"] < 1.0 and mixed_error > 1e-4 and zero_error < 1e-13
    print(json.dumps({"first_layer":first, "second_layer":second,
                      "early_normalization_error":mixed_error, "zero_weight_control":zero_error}, indent=2))
