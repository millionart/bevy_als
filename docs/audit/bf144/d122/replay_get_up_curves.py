import argparse
from bisect import bisect_right
import json
from pathlib import Path
import sys
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[4]
sys.path[:0] = [
    str(ROOT / "tools/import"),
    str(ROOT / "tools/animgraph_authoring"),
    str(ROOT / "docs/audit/bf144/d121c"),
]
import gen_als_ragdoll_settings
import ue_animgraph_converter
from replay_curve_products import evaluate, lerp
from ue_animation_asset_resolver import native_rich_curve, rich_curve_key, RICH_CURVE_KEY_NUMBERS
from ue_animation_timing import f32
from ue_exported_text import field, parse_list, parse_struct


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def field_named(entry, name):
    field, = [field for field in entry["fields"] if field["name"] == name]
    return field


def find_sequence_curves(value):
    if isinstance(value, dict):
        if "ALS_float_curves" in value:
            yield value["ALS_float_curves"]
        for child in value.values():
            yield from find_sequence_curves(child)
    elif isinstance(value, list):
        for child in value:
            yield from find_sequence_curves(child)


def sample_runtime(curve, time):
    times, values = curve["times"], curve["values"]
    if time <= times[0]:
        return values[0]
    if time >= times[-1]:
        return values[-1]
    right = bisect_right(times, time)
    left = right - 1
    width = f32(times[right] - times[left])
    alpha = f32(f32(time - times[left]) / width)
    start, end = values[left], values[right]
    mode = curve["interps"][left]
    if mode == "Constant":
        return start
    if mode == "Linear":
        return lerp(start, end, alpha)
    assert mode == "Cubic" and curve["weight_modes"][left] == curve["weight_modes"][right] == 0
    squared = f32(alpha * alpha)
    cubed = f32(squared * alpha)
    a = f32(f32(f32(f32(2.0 * cubed) - f32(3.0 * squared)) + 1.0) * start)
    b = f32(f32(f32(f32(cubed - f32(2.0 * squared)) + alpha) * width) * curve["leave_tangents"][left])
    c = f32(f32(f32(-2.0 * cubed) + f32(3.0 * squared)) * end)
    d = f32(f32(f32(cubed - squared) * width) * curve["arrive_tangents"][right])
    return f32(f32(f32(a + b) + c) + d)


def verify_curve_inputs(source, settings):
    raw, = [row for row in source["asset_properties"] if row["name"] == "RawCurveData"]
    elements = field_named(raw, "FloatCurves")["elements"]
    exact = {}
    rounded = {}
    for element in elements:
        name = field_named(element, "CurveName")["exported_text"]
        curve = field_named(element, "FloatCurve")
        keys, pre, post = native_rich_curve(curve)
        assert pre == post == "RCCE_Constant"
        exact[name] = keys
        generated = settings["montage_curves"][name]
        assert generated["times"] == [k["t"] for k in keys]
        assert generated["values"] == [k["v"] for k in keys]
        assert generated["interps"] == [k["interpolation"] for k in keys]
        for field_name in ("arrive_tangent", "leave_tangent", "arrive_weight", "leave_weight"):
            assert generated[field_name + "s"] == [k[field_name] for k in keys]
        rounded[name] = [rich_curve_key(key) for key in parse_list(field(parse_struct(curve["exported_text"]), "Keys"))]
        for key in rounded[name]:
            for field_name in RICH_CURVE_KEY_NUMBERS:
                key[field_name] = f32(key[field_name])
    assert len(exact) == 11 and exact.keys() == settings["montage_curves"].keys()
    position_samples = value_samples = rounded_failures = 0
    max_runtime_error = 0.0
    for sample in source["montage_clock_probe"]["samples"]:
        frame, = sample["frames"]
        time = frame["position"]
        assert frame["curves"].keys() == exact.keys()
        position_samples += 1
        for name, value in frame["curves"].items():
            assert evaluate(exact[name], time) == value, (name, time, evaluate(exact[name], time), value)
            rounded_failures += evaluate(rounded[name], time) != value
            max_runtime_error = max(max_runtime_error, abs(sample_runtime(settings["montage_curves"][name], time) - value))
            value_samples += 1
    assert rounded_failures > 0 and max_runtime_error < 1e-6
    gltf = settings["curves"].split("#", 1)[0]
    sequence, = list(find_sequence_curves(read(ROOT / "assets" / gltf)))
    if isinstance(sequence, str):
        sequence = json.loads(sequence)
    sequence_names = {curve["name"] for curve in sequence["curves"]}
    assert sequence_names == {"PoseCrouching", "PoseGrounded"}
    assert not sequence_names.intersection(exact)
    return {"source":source["asset_path"], "positions":position_samples, "values":value_samples,
            "rounded_failures":rounded_failures, "max_runtime_sampling_error":max_runtime_error,
            "sequence_only_missing_values":value_samples, "montage_curves":sorted(exact),
            "sequence_curves":sorted(sequence_names)}


def regenerate(evidence, output, content_root, cpp):
    names = ["front", "back", "settings"]
    documents = {name:read(evidence / (name + ".native.json")) for name in names}
    by_asset = {document["asset_path"].split(".")[-1]:document for document in documents.values()}
    target = output / "als_ragdoll_settings.json"
    cli = ["gen_als_ragdoll_settings.py", "--uasset", str(content_root / "ALS/Data/Character/CS_Als_Default.uasset"),
           "--content-root", str(content_root), "--source-cpp", str(cpp), "--output", str(target),
           "--report", str(output / "ragdoll-settings.report.json")]
    def resolve(path, *_args, **_kwargs):
        return by_asset[path.stem], {}
    with patch.object(ue_animgraph_converter, "read_uasset_source", side_effect=resolve), patch.object(sys, "argv", cli):
        gen_als_ragdoll_settings.main()
    generated = read(target)
    report = [verify_curve_inputs(documents[face], generated[face]) for face in ("front", "back")]
    (output / "curve-comparison.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--content-root", required=True, type=Path)
    parser.add_argument("--source-cpp", required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    regenerate(Path(__file__).resolve().parent, args.output, args.content_root, args.source_cpp)
