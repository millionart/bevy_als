import argparse
from bisect import bisect_right
import json
from pathlib import Path
import sys
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[4]
sys.path[:0] = [str(ROOT / "tools/import"), str(ROOT / "tools/animgraph_authoring")]
import gen_in_air_settings
import gen_standing_curves
from ue_animation_asset_resolver import asset_property, curve_float_contract, native_float_curve, rich_curve_key, RICH_CURVE_KEY_NUMBERS
from ue_animation_timing import f32
from ue_exported_text import field, parse_list, parse_struct


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def lerp(left, right, alpha):
    return f32(left + f32(f32(right - left) * alpha))


def evaluate(keys, time):
    if time < keys[0]["t"]:
        return keys[0]["v"]
    if time > keys[-1]["t"]:
        return keys[-1]["v"]
    index = min(bisect_right([key["t"] for key in keys], time), len(keys) - 1)
    left, right = keys[index - 1:index + 1]
    width = f32(right["t"] - left["t"])
    alpha = f32(f32(time - left["t"]) / width)
    start, end = left["v"], right["v"]
    if left["interpolation"] == "Constant":
        return start
    if left["interpolation"] == "Linear":
        return lerp(start, end, alpha)
    assert left["interpolation"] == "Cubic"
    assert left["tangent_weight_mode"] == right["tangent_weight_mode"] == "None"
    third = f32(1.0 / 3.0)
    control_a = f32(start + f32(f32(left["leave_tangent"] * width) * third))
    control_b = f32(end - f32(f32(right["arrive_tangent"] * width) * third))
    a = lerp(start, control_a, alpha)
    b = lerp(control_a, control_b, alpha)
    c = lerp(control_b, end, alpha)
    return lerp(lerp(a, b, alpha), lerp(b, c, alpha), alpha)


def verify_native(evidence):
    results = []
    for sample in read(evidence / "curve-values.native.json")["blend_profile_probe"]["samples"]:
        source = read(evidence / (sample["name"] + ".native.json"))
        keys, pre, post = native_float_curve(source)
        assert pre == post == "RCCE_Constant"
        fields = {"time":"t", "value":"v", "arrive_tangent":"arrive_tangent", "leave_tangent":"leave_tangent"}
        assert [{name:key[field] for name,field in fields.items()} for key in keys] == sample["custom_curve_keys"]
        rounded = [rich_curve_key(key) for key in parse_list(field(parse_struct(asset_property(source, "FloatCurve")), "Keys"))]
        for key in rounded:
            for name in RICH_CURVE_KEY_NUMBERS:
                key[name] = f32(key[name])
        frames = sample["custom_curve_time_samples"]
        exact_failures = sum(evaluate(keys, row["time"]) != row["value"] for row in frames)
        rounded_failures = sum(evaluate(rounded, row["time"]) != row["value"] for row in frames)
        assert exact_failures == 0 and rounded_failures > 0, (sample["name"], exact_failures, rounded_failures)
        results.append({"name":sample["name"], "keys":len(keys), "samples":len(frames),
                        "exact_failures":exact_failures, "rounded_failures":rounded_failures})
    return results


def regenerate(evidence, output, content_root, cpp):
    settings = read(evidence / "animation-settings.native.json")
    curves = {}
    for path in evidence.glob("*.native.json"):
        document = read(path)
        if document["asset_class"] == "/Script/Engine.CurveFloat":
            curves[document["asset_path"]] = document
    uasset = content_root / "ALS/Data/AnimationInstance/AIS_Als_Default.uasset"
    assert uasset.is_file()
    for module, stem, extra in [
        (gen_standing_curves, "als_standing_curves", ["--content-root", str(content_root)]),
        (gen_in_air_settings, "als_in_air_settings", ["--animation-source", str(cpp)]),
    ]:
        args = [module.__file__, "--uasset", str(uasset), "--output", str(output / (stem + ".json")),
                "--report", str(output / (stem + ".report.json")), *extra]
        with patch.object(module, "read_uasset_source", return_value=(settings, {})), \
             patch.object(module, "native_source_asset", side_effect=lambda name, _: curves[name]), \
             patch.object(sys, "argv", args):
            module.main()
    stance = curve_float_contract(read(evidence / "stance.native.json"))
    (output / "stance.contract.json").write_text(json.dumps(stance, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--content-root", required=True, type=Path)
    parser.add_argument("--source-cpp", required=True, type=Path)
    args = parser.parse_args()
    evidence = Path(__file__).resolve().parent
    args.output.mkdir(parents=True, exist_ok=True)
    result = verify_native(evidence)
    (args.output / "curve-sample-comparison.json").write_text(json.dumps(result, indent=2) + "\n")
    regenerate(evidence, args.output, args.content_root, args.source_cpp)
    print(json.dumps(result), flush=True)
