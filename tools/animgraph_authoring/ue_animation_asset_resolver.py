import functools
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

IMPORT_ROOT = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(IMPORT_ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "uasset_curve_export"))

from uasset_base import UAsset
from uasset_reader import PropReader
from uasset import UAsset as CurveUAsset, ParseError as CurveParseError
from extract_curves import CurveKey, FloatCurve, extract_float_curves, is_anim_sequence
from export_curve_gltf import FLT_MAX_MARKER, curve_to_json
from ue_exported_text import (
    ExportedTextError,
    field as struct_field,
    parse_bool,
    parse_list,
    parse_number,
    parse_quoted,
    parse_struct,
    reject_unknown_fields,
)


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE_READER = ROOT / "tools" / "run_ue_source_reader.ps1"
RICH_CURVE_INTERPOLATIONS = {
    "RCIM_Constant": "Constant",
    "RCIM_Linear": "Linear",
    "RCIM_Cubic": "Cubic",
}
RICH_CURVE_TANGENT_MODES = {
    "RCTM_Auto": "Auto",
    "RCTM_User": "User",
    "RCTM_Break": "Break",
    "RCTM_None": "None",
    "RCTM_SmartAuto": "SmartAuto",
}
RICH_CURVE_TANGENT_WEIGHT_MODES = {
    "RCTWM_WeightedNone": "None",
    "RCTWM_WeightedArrive": "Arrive",
    "RCTWM_WeightedLeave": "Leave",
    "RCTWM_WeightedBoth": "Both",
}


class AssetResolutionError(ValueError):
    pass


def source_object_path(value):
    matches = re.findall(r"'(/[^']+)'", value)
    if len(matches) != 1:
        raise AssetResolutionError("source object reference is missing or ambiguous")
    return matches[0]


def source_asset_name(source_path):
    if not isinstance(source_path, str) or "." not in source_path:
        raise AssetResolutionError("source asset path is invalid")
    return source_path.rsplit(".", 1)[-1]


def source_package_path(source_path):
    if not isinstance(source_path, str) or "." not in source_path:
        raise AssetResolutionError("source asset path is invalid")
    return source_path.rsplit(".", 1)[0]


def source_uasset_path(content_root, mount, source_path):
    root = Path(content_root).resolve()
    if not root.is_dir() or not isinstance(mount, str) or not mount.startswith("/"):
        raise AssetResolutionError("source content root or mount is invalid")
    package = source_package_path(source_path)
    prefix = mount.rstrip("/")
    if not prefix or not package.startswith(prefix + "/"):
        raise AssetResolutionError("source asset is outside mount")
    path = (root / (package[len(prefix) + 1 :] + ".uasset")).resolve()
    try:
        path.relative_to(root)
    except ValueError as error:
        raise AssetResolutionError("source asset is outside content root") from error
    if not path.is_file():
        raise AssetResolutionError("source animation asset is unavailable")
    return path


def source_export_class(asset, export):
    if export.class_index >= 0:
        return None
    imported = asset.imports[-export.class_index - 1]
    return imported.object_name if imported.class_name == "Class" else imported.class_name


def native_source_asset(source_path, source_reader=None):
    reader = Path(source_reader or DEFAULT_SOURCE_READER).resolve()
    return native_source_asset_cached(source_path, str(reader))


@functools.lru_cache(maxsize=None)
def native_source_asset_cached(source_path, reader_path):
    reader = Path(reader_path)
    if not reader.is_file():
        raise AssetResolutionError("native source reader is unavailable")
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as directory:
        output = Path(directory) / "source.ue.json"
        try:
            process = subprocess.run(
                [
                    "pwsh",
                    "-NoProfile",
                    "-File",
                    str(reader),
                    "-Asset",
                    source_path,
                    "-Output",
                    str(output),
                    "-TimeoutSeconds",
                    "120",
                    "-NoProgressSeconds",
                    "60",
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=150,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise AssetResolutionError("native source reader failed") from error
        if process.returncode != 0 or not output.is_file():
            raise AssetResolutionError("native source reader failed")
        try:
            value = json.loads(output.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise AssetResolutionError("native source reader output is invalid") from error
    if value.get("format") != "ue_asset_source_v1" or value.get("asset_path") != source_path:
        raise AssetResolutionError("native source reader returned an unexpected asset")
    return value


def asset_properties(source, name):
    values = [
        value.get("exported_text")
        for value in source.get("asset_properties", [])
        if value.get("name") == name and isinstance(value.get("exported_text"), str)
    ]
    if not values:
        raise AssetResolutionError(f"source asset property is missing: {name}")
    return values


def asset_property(source, name):
    values = asset_properties(source, name)
    if len(values) != 1:
        raise AssetResolutionError(f"source asset property is missing or ambiguous: {name}")
    return values[0]


def object_property(value, name):
    values = [
        property_value.get("exported_text")
        for property_value in value.get("properties", [])
        if property_value.get("name") == name
        and isinstance(property_value.get("exported_text"), str)
    ]
    if len(values) != 1:
        raise AssetResolutionError(f"source object property is missing or ambiguous: {name}")
    return values[0]


def rich_curve_enum(fields, name, default, values):
    value = struct_field(fields, name, default)
    target = values.get(value)
    if target is None:
        raise AssetResolutionError(f"source rich curve {name} is unsupported")
    return target


def rich_curve_key(value):
    try:
        fields = parse_struct(value)
        reject_unknown_fields(
            fields,
            {
                "InterpMode",
                "TangentMode",
                "TangentWeightMode",
                "Time",
                "Value",
                "ArriveTangent",
                "ArriveTangentWeight",
                "LeaveTangent",
                "LeaveTangentWeight",
            },
        )
        return {
            "interpolation": rich_curve_enum(
                fields,
                "InterpMode",
                "RCIM_Linear",
                RICH_CURVE_INTERPOLATIONS,
            ),
            "tangent_mode": rich_curve_enum(
                fields,
                "TangentMode",
                "RCTM_Auto",
                RICH_CURVE_TANGENT_MODES,
            ),
            "tangent_weight_mode": rich_curve_enum(
                fields,
                "TangentWeightMode",
                "RCTWM_WeightedNone",
                RICH_CURVE_TANGENT_WEIGHT_MODES,
            ),
            "t": parse_number(struct_field(fields, "Time", "0.0")),
            "v": parse_number(struct_field(fields, "Value", "0.0")),
            "arrive_tangent": parse_number(struct_field(fields, "ArriveTangent", "0.0")),
            "arrive_weight": parse_number(struct_field(fields, "ArriveTangentWeight", "0.0")),
            "leave_tangent": parse_number(struct_field(fields, "LeaveTangent", "0.0")),
            "leave_weight": parse_number(struct_field(fields, "LeaveTangentWeight", "0.0")),
        }
    except ExportedTextError as error:
        raise AssetResolutionError("source rich curve key is invalid") from error


def curve_float_contract(source):
    if (
        not isinstance(source, dict)
        or source.get("format") != "ue_asset_source_v1"
        or source.get("asset_class") != "/Script/Engine.CurveFloat"
        or not isinstance(source.get("asset_path"), str)
    ):
        raise AssetResolutionError("source CurveFloat is invalid")
    try:
        fields = parse_struct(asset_property(source, "FloatCurve"))
        reject_unknown_fields(
            fields,
            {"Keys", "DefaultValue", "PreInfinityExtrap", "PostInfinityExtrap"},
        )
        parse_number(struct_field(fields, "DefaultValue"))
        if struct_field(fields, "PreInfinityExtrap", "RCCE_Constant") != "RCCE_Constant":
            raise AssetResolutionError("source CurveFloat pre-infinity extrapolation is unsupported")
        if struct_field(fields, "PostInfinityExtrap", "RCCE_Constant") != "RCCE_Constant":
            raise AssetResolutionError("source CurveFloat post-infinity extrapolation is unsupported")
        keys = [rich_curve_key(value) for value in parse_list(struct_field(fields, "Keys"))]
    except ExportedTextError as error:
        raise AssetResolutionError("source CurveFloat properties are invalid") from error
    if len(keys) < 2 or any(right["t"] <= left["t"] for left, right in zip(keys, keys[1:])):
        raise AssetResolutionError("source CurveFloat key times are invalid")
    return {"source_curve": source["asset_path"], "keys": keys}


def source_curve_float(content_root, mount, source_path, source_reader=None):
    source_uasset_path(content_root, mount, source_path)
    return curve_float_contract(native_source_asset(source_path, source_reader))


def sampling_frame_rate(value):
    if isinstance(value, str):
        fields = parse_struct(value)
    elif isinstance(value, dict):
        fields = value.get("__tagged__", value)
    else:
        raise AssetResolutionError("source sampling frame rate is unavailable")
    numerator = parse_number(str(fields.get("Numerator")))
    denominator = parse_number(str(fields.get("Denominator", 1)))
    if not numerator.is_integer() or not denominator.is_integer() or numerator <= 0 or denominator <= 0:
        raise AssetResolutionError("source sampling frame rate is invalid")
    return numerator / denominator


def sequence_root_lock(forced, mode):
    forced = parse_bool(forced) if isinstance(forced, str) else forced
    if not isinstance(forced, bool):
        raise AssetResolutionError("source force root lock is invalid")
    if not forced:
        return None
    if isinstance(mode, dict):
        mode = mode.get("value", mode.get("enum_name"))
    mode = mode.rsplit("::", 1)[-1] if isinstance(mode, str) else mode
    if mode not in {"RefPose", "AnimFirstFrame", "Zero"}:
        raise AssetResolutionError("source root lock mode is unsupported")
    return mode


def sequence_timing_contract(source):
    if (
        not isinstance(source, dict)
        or source.get("format") != "ue_asset_source_v1"
        or source.get("asset_class") != "/Script/Engine.AnimSequence"
    ):
        raise AssetResolutionError("source animation sequence is invalid")
    try:
        duration = parse_number(asset_property(source, "SequenceLength"))
        frames_per_second = sampling_frame_rate(asset_property(source, "SamplingFrameRate"))
        properties = {row["name"]: row["exported_text"] for row in source["asset_properties"]}
        root_lock = sequence_root_lock(properties.get("bForceRootLock", "False"), properties.get("RootMotionRootLock", "RefPose"))
        raw_markers = asset_property(source, "AuthoredSyncMarkers")
        markers = []
        for raw_marker in parse_list(raw_markers) if raw_markers else []:
            fields = parse_struct(raw_marker)
            reject_unknown_fields(fields, {"MarkerName", "Time", "TrackIndex", "Guid"})
            markers.append(
                {
                    "name": parse_quoted(struct_field(fields, "MarkerName")),
                    "time": parse_number(struct_field(fields, "Time")),
                }
            )
    except ExportedTextError as error:
        raise AssetResolutionError("source animation sequence properties are unreadable") from error
    if duration <= 0.0 or any(not marker["name"] for marker in markers):
        raise AssetResolutionError("source animation sequence duration or markers are invalid")
    return {"duration": duration, "markers": markers, "frames_per_second": frames_per_second, "root_lock": root_lock}


def source_sequence_timing(content_root, mount, source_path, source_reader=None):
    path = source_uasset_path(content_root, mount, source_path)
    try:
        asset = UAsset(str(path))
        reader = PropReader(asset)
    except Exception:
        return sequence_timing_contract(native_source_asset(source_path, source_reader))
    name = source_asset_name(source_path)
    matches = [
        index
        for index, export in enumerate(asset.exports)
        if source_export_class(asset, export) == "AnimSequence" and export.object_name == name
    ]
    if len(matches) != 1:
        raise AssetResolutionError("source animation sequence export is missing or ambiguous")
    properties, ok, _ = reader.read_export(matches[0])
    if not ok:
        return sequence_timing_contract(native_source_asset(source_path, source_reader))
    duration = properties.get("SequenceLength")
    if not isinstance(duration, (int, float)) or isinstance(duration, bool) or duration <= 0.0:
        raise AssetResolutionError("source animation sequence duration is invalid")
    markers = []
    for marker in properties.get("AuthoredSyncMarkers", []):
        if not isinstance(marker, dict):
            raise AssetResolutionError("source animation sync marker is invalid")
        marker_name = marker.get("MarkerName")
        time = marker.get("Time")
        if not isinstance(marker_name, str) or not isinstance(time, (int, float)) or isinstance(time, bool):
            raise AssetResolutionError("source animation sync marker fields are invalid")
        markers.append({"name": marker_name, "time": float(time)})
    try:
        frames_per_second = sampling_frame_rate(properties.get("SamplingFrameRate"))
        root_lock = sequence_root_lock(properties.get("bForceRootLock", False), properties.get("RootMotionRootLock", "RefPose"))
    except (AssetResolutionError, ExportedTextError):
        return sequence_timing_contract(native_source_asset(source_path, source_reader))
    return {"duration": float(duration), "markers": markers, "frames_per_second": frames_per_second, "root_lock": root_lock}


def sequence_curves_contract(source):
    if not isinstance(source, dict) or source.get("format") != "ue_asset_source_v1" or source.get("asset_class") != "/Script/Engine.AnimSequence":
        raise AssetResolutionError("source animation sequence curves are invalid")
    try:
        raw = parse_struct(asset_property(source, "RawCurveData"))
        reject_unknown_fields(raw, {"FloatCurves", "TransformCurves"})
        curves = []
        for value in parse_list(struct_field(raw, "FloatCurves", "()")):
            fields = parse_struct(value)
            curve = FloatCurve()
            curve.name = parse_quoted(struct_field(fields, "CurveName"))
            flags = parse_number(struct_field(fields, "CurveTypeFlags", "4"))
            if not flags.is_integer():
                raise AssetResolutionError("source animation curve flags are invalid")
            curve.flags = int(flags)
            rich = parse_struct(struct_field(fields, "FloatCurve", "()"))
            reject_unknown_fields(rich, {"DefaultValue", "PreInfinityExtrap", "PostInfinityExtrap", "Keys"})
            curve.default_value = parse_number(rich["DefaultValue"]) if "DefaultValue" in rich else FLT_MAX_MARKER
            for name, attribute in (("PreInfinityExtrap", "pre_infinity"), ("PostInfinityExtrap", "post_infinity")):
                mode = struct_field(rich, name, "RCCE_Constant")
                if mode not in {"RCCE_Cycle", "RCCE_CycleWithOffset", "RCCE_Oscillate", "RCCE_Linear", "RCCE_Constant", "RCCE_None"}:
                    raise AssetResolutionError("source animation curve extrapolation is unsupported")
                setattr(curve, attribute, mode)
            for raw_key in parse_list(struct_field(rich, "Keys", "()")):
                value = rich_curve_key(raw_key)
                key = CurveKey()
                key.interp = ("Linear", "Constant", "Cubic").index(value["interpolation"])
                key.tangent_mode = ("Auto", "User", "Break", "None", "SmartAuto").index(value["tangent_mode"])
                key.weight_mode = ("None", "Arrive", "Leave", "Both").index(value["tangent_weight_mode"])
                for attribute, field in (("time", "t"), ("value", "v"), ("arrive_tangent", "arrive_tangent"),
                    ("leave_tangent", "leave_tangent"), ("arrive_weight", "arrive_weight"), ("leave_weight", "leave_weight")):
                    setattr(key, attribute, value[field])
                curve.keys.append(key)
            curves.append(curve_to_json(curve))
    except ExportedTextError as error:
        raise AssetResolutionError("source animation sequence curve properties are unreadable") from error
    names = [curve["name"] for curve in curves]
    if any(not name for name in names) or len(names) != len(set(names)):
        raise AssetResolutionError("source animation curve names are invalid")
    return curves


def source_sequence_curves(content_root, mount, source_path, source_reader=None):
    path = source_uasset_path(content_root, mount, source_path)
    try:
        asset = CurveUAsset(str(path))
        indices = [
            index for index, export in enumerate(asset.exports)
            if is_anim_sequence(asset, index) and export.object_name == source_asset_name(source_path)
        ]
        if len(indices) != 1:
            raise AssetResolutionError("source animation curve export is missing or ambiguous")
        curves = [curve_to_json(curve) for curve in extract_float_curves(asset, indices[0])]
    except CurveParseError:
        return sequence_curves_contract(native_source_asset(source_path, source_reader))
    names = [curve["name"] for curve in curves]
    if any(not name for name in names) or len(names) != len(set(names)):
        raise AssetResolutionError("source animation curve names are invalid")
    return curves


def blend_profile_contract(source, source_path):
    package_object, separator, profile_name = source_path.rpartition(":")
    if not separator or not package_object or not profile_name:
        raise AssetResolutionError("source blend profile path is invalid")
    if (
        not isinstance(source, dict)
        or source.get("format") != "ue_asset_source_v1"
        or source.get("asset_path") != package_object
    ):
        raise AssetResolutionError("source blend profile asset is invalid")
    matches = [
        value
        for value in source.get("asset_objects", [])
        if value.get("class_path") == "/Script/Engine.BlendProfile"
        and value.get("object_path") == source_path
    ]
    if len(matches) != 1:
        raise AssetResolutionError("source blend profile export is missing or ambiguous")
    try:
        entries = parse_list(object_property(matches[0], "ProfileEntries"))
        scales = []
        for raw_entry in entries:
            fields = parse_struct(raw_entry)
            reject_unknown_fields(fields, {"BoneReference", "BlendScale"})
            reference = parse_struct(struct_field(fields, "BoneReference"))
            reject_unknown_fields(reference, {"BoneName"})
            bone = parse_quoted(struct_field(reference, "BoneName"))
            scale = parse_number(struct_field(fields, "BlendScale"))
            if not bone:
                raise AssetResolutionError("source blend profile entry fields are invalid")
            scales.append({"bone": bone, "scale": scale})
    except ExportedTextError as error:
        raise AssetResolutionError("source blend profile properties are unreadable") from error
    return {"source_profile": source_path, "bone_scales": scales}


def source_blend_profile(content_root, mount, source_path, source_reader=None):
    package_object, separator, profile_name = source_path.rpartition(":")
    if not separator or not package_object or not profile_name:
        raise AssetResolutionError("source blend profile path is invalid")
    path = source_uasset_path(content_root, mount, package_object)
    try:
        asset = UAsset(str(path))
        reader = PropReader(asset)
    except Exception:
        return blend_profile_contract(
            native_source_asset(package_object, source_reader), source_path
        )
    matches = [
        index
        for index, export in enumerate(asset.exports)
        if source_export_class(asset, export) == "BlendProfile"
        and export.object_name == profile_name
    ]
    if len(matches) != 1:
        raise AssetResolutionError("source blend profile export is missing or ambiguous")
    properties, ok, _ = reader.read_export(matches[0])
    if not ok:
        raise AssetResolutionError("source blend profile properties are unreadable")
    scales = []
    for entry in properties.get("ProfileEntries", []):
        if not isinstance(entry, dict):
            raise AssetResolutionError("source blend profile entry is invalid")
        reference = entry.get("BoneReference")
        tagged = reference.get("__tagged__") if isinstance(reference, dict) else None
        bone = tagged.get("BoneName") if isinstance(tagged, dict) else None
        scale = entry.get("BlendScale")
        if (
            not isinstance(bone, str)
            or not bone
            or not isinstance(scale, (int, float))
            or isinstance(scale, bool)
        ):
            raise AssetResolutionError("source blend profile entry fields are invalid")
        scales.append({"bone": bone, "scale": float(scale)})
    return {"source_profile": source_path, "bone_scales": scales}


def source_blend_profile_wrapper(content_root, mount, value, source_reader=None):
    try:
        fields = parse_struct(value)
        unknown = set(fields) - {"bIsSkeletonBlendProfile", "BlendProfile"}
        if unknown or not parse_bool(struct_field(fields, "bIsSkeletonBlendProfile")):
            raise ExportedTextError("blend profile wrapper is unsupported")
    except ExportedTextError as error:
        raise AssetResolutionError("source blend profile wrapper is invalid") from error
    reference = fields.get("BlendProfile")
    if reference is None:
        return {"source_profile": None, "bone_scales": []}
    return source_blend_profile(
        content_root, mount, source_object_path(reference), source_reader
    )


def relative_asset_path(asset_root, path):
    try:
        return path.resolve().relative_to(asset_root.resolve()).as_posix()
    except ValueError as error:
        raise AssetResolutionError("target asset is outside asset root") from error


def unique_candidate(candidates, label):
    values = sorted({path.resolve() for path in candidates})
    if len(values) != 1:
        raise AssetResolutionError(f"{label} target asset count is {len(values)}")
    return values[0]


def resolve_animation_clip(asset_root, source_path):
    name = source_asset_name(source_path)
    target = unique_candidate(asset_root.rglob(name + ".anim.ron"), "animation clip")
    return relative_asset_path(asset_root, target)


def resolve_reference_skeleton(asset_root, source_path):
    candidates = []
    for path in asset_root.rglob("*.skn.ron"):
        text = path.read_text(encoding="utf-8")
        identities = re.findall(r'\bsource_skeleton\s*:\s*("(?:\\.|[^"\\])*")', text)
        if len(identities) == 1 and json.loads(identities[0]) == source_path:
            candidates.append(path)
    target = unique_candidate(candidates, "source reference skeleton")
    sources = re.findall(r'\bsource\s*:\s*Gltf\s*\(\s*source\s*:\s*("(?:\\.|[^"\\])*")', target.read_text(encoding="utf-8"))
    if len(sources) != 1:
        raise AssetResolutionError("reference skeleton mesh source is invalid")
    mesh = asset_root / json.loads(sources[0])
    relative_asset_path(asset_root, mesh)
    if not mesh.is_file():
        raise AssetResolutionError("reference skeleton mesh is unavailable")
    return relative_asset_path(asset_root, target)


def target_clip_skeleton(asset_root, target):
    values = re.findall(r'\bskeleton\s*:\s*("(?:\\.|[^"\\])*")', (asset_root / target).read_text(encoding="utf-8"))
    if not values:
        return None
    if len(values) != 1:
        raise AssetResolutionError("animation clip reference skeleton is ambiguous")
    path = asset_root / json.loads(values[0])
    relative = relative_asset_path(asset_root, path)
    if not path.is_file():
        raise AssetResolutionError("animation clip reference skeleton is unavailable")
    return relative


def blendspace_metadata_path(asset_root, source_path):
    package_path = source_package_path(source_path)
    candidates = []
    for path in asset_root.rglob("*.json"):
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(value, dict) and value.get("asset") == package_path:
            candidates.append(path)
    return unique_candidate(candidates, "blendspace")


def numeric(value, label):
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise AssetResolutionError(f"{label} is not numeric")
    return float(value)


def source_pin_number(value, label):
    if isinstance(value, str) and re.fullmatch(r"-?[0-9]+(?:\.[0-9]+)?", value):
        return float(value)
    return numeric(value, label)


def resolve_blendspace_contract(asset_root, source_path):
    path = blendspace_metadata_path(asset_root, source_path)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise AssetResolutionError("blendspace metadata cannot be read") from error
    axes = value.get("axes") if isinstance(value, dict) else None
    samples = value.get("samples") if isinstance(value, dict) else None
    if not isinstance(axes, list) or len(axes) < 2:
        raise AssetResolutionError("blendspace axes are unavailable")
    if not isinstance(samples, list) or not samples:
        raise AssetResolutionError("blendspace samples are unavailable")
    normalized_axes = []
    for index, axis in enumerate(axes):
        if not isinstance(axis, dict) or not isinstance(axis.get("name"), str):
            raise AssetResolutionError(f"blendspace axis {index} is invalid")
        normalized_axes.append(
            {
                "name": axis["name"],
                "min": numeric(axis.get("min"), f"blendspace axis {index} min"),
                "max": numeric(axis.get("max"), f"blendspace axis {index} max"),
                "grid_num": numeric(axis.get("grid_num"), f"blendspace axis {index} grid"),
            }
        )
    normalized_samples = []
    for index, sample in enumerate(samples):
        if not isinstance(sample, dict):
            raise AssetResolutionError(f"blendspace sample {index} is invalid")
        source_animation = sample.get("animation")
        coordinates = sample.get("value")
        if not isinstance(source_animation, str) or not isinstance(coordinates, list) or len(coordinates) < 2:
            raise AssetResolutionError(f"blendspace sample {index} is incomplete")
        normalized_samples.append(
            {
                "source_animation": source_animation,
                "target_clip": resolve_animation_clip(asset_root, source_animation),
                "value": [numeric(component, f"blendspace sample {index} value") for component in coordinates],
                "rate_scale": numeric(sample.get("rate_scale"), f"blendspace sample {index} rate scale"),
            }
        )
    return {
        "source_asset": source_path,
        "target_asset": relative_asset_path(asset_root, path),
        "axes": normalized_axes,
        "samples": normalized_samples,
    }


def source_name(value):
    return parse_quoted(value) if value.startswith('"') else value


def blendspace_vector(value):
    fields = parse_struct(value)
    reject_unknown_fields(fields, {"X", "Y", "Z"})
    return [
        parse_number(struct_field(fields, axis, "0.0"))
        for axis in ("X", "Y", "Z")
    ]


def blendspace_source_sample(value):
    fields = parse_struct(value)
    reject_unknown_fields(
        fields,
        {
            "Animation",
            "SampleValue",
            "RateScale",
            "bMirror",
            "bUseSingleFrameForBlending",
            "FrameIndexToSample",
            "bIncludeInAnalyseAll",
        },
    )
    if parse_bool(struct_field(fields, "bMirror", "False")):
        raise AssetResolutionError("source blendspace mirrored sample is unsupported")
    if parse_bool(struct_field(fields, "bUseSingleFrameForBlending", "False")):
        raise AssetResolutionError("source blendspace single-frame sample is unsupported")
    if parse_number(struct_field(fields, "FrameIndexToSample", "0")) != 0.0:
        raise AssetResolutionError("source blendspace sample frame is unsupported")
    if not parse_bool(struct_field(fields, "bIncludeInAnalyseAll", "True")):
        raise AssetResolutionError("source blendspace excluded sample is unsupported")
    return {
        "source_animation": source_object_path(struct_field(fields, "Animation")),
        "value": blendspace_vector(struct_field(fields, "SampleValue", "()")),
        "rate_scale": parse_number(struct_field(fields, "RateScale", "1.0")),
    }


def blendspace_triangulation(source, axes, samples):
    minimum = [axis["min"] for axis in axes[:2]]
    maximum = [axis["max"] for axis in axes[:2]]
    if any(high <= low for low, high in zip(minimum, maximum)):
        raise AssetResolutionError("source BlendSpace axis range is invalid")
    fields = parse_struct(asset_property(source, "BlendSpaceData"))
    reject_unknown_fields(fields, {"Triangles", "Segments"})
    segments = []
    for row in parse_list(struct_field(fields, "Segments", "()")):
        segment = parse_struct(row)
        reject_unknown_fields(segment, {f"{name}[{index}]" for name in ("SampleIndices", "Vertices") for index in range(2)})
        indices = [parse_number(struct_field(segment, f"SampleIndices[{index}]")) for index in range(2)]
        if any(index != int(index) or not 0 <= index < len(samples) for index in indices):
            raise AssetResolutionError("source BlendSpace segment sample index is invalid")
        indices = [int(index) for index in indices]
        for corner, index in enumerate(indices):
            vertex = parse_number(struct_field(segment, f"Vertices[{corner}]", "0.0"))
            expected = (samples[index]["value"][0] - minimum[0]) / (maximum[0] - minimum[0])
            if abs(vertex - expected) > 1e-6:
                raise AssetResolutionError("source BlendSpace segment vertex differs from its sample")
        segments.append(indices)
    triangles = []
    for row in parse_list(struct_field(fields, "Triangles", "()")):
        fields = parse_struct(row)
        reject_unknown_fields(fields, {f"{name}[{index}]" for name in ("SampleIndices", "Vertices", "EdgeInfo") for index in range(3)})
        indices = [parse_number(struct_field(fields, f"SampleIndices[{index}]")) for index in range(3)]
        if any(index != int(index) or not 0 <= index < len(samples) for index in indices):
            raise AssetResolutionError("source BlendSpace triangle sample index is invalid")
        indices = [int(index) for index in indices]
        for corner, index in enumerate(indices):
            vertex = blendspace_vector(struct_field(fields, f"Vertices[{corner}]", "()"))[:2]
            expected = [(samples[index]["value"][axis] - minimum[axis]) / (maximum[axis] - minimum[axis]) for axis in range(2)]
            if any(abs(actual - expected) > 1e-6 for actual, expected in zip(vertex, expected)):
                raise AssetResolutionError("source BlendSpace normalized triangle vertex differs from its sample")
        triangles.append(indices)
    if bool(triangles) == bool(segments):
        raise AssetResolutionError("source BlendSpace topology is missing or mixed")
    if segments and segments != [[segments[0][0], segments[0][0]]]:
        if any(samples[a]["value"][0] >= samples[b]["value"][0] for a, b in segments) or any(left[1] != right[0] for left, right in zip(segments, segments[1:])):
            raise AssetResolutionError("source BlendSpace segments are unordered or disconnected")
    return {"axis_min": minimum, "axis_max": maximum, "triangles": triangles, "segments": segments}


def blendspace_asset_contract(source, blendspace, evaluator=False):
    if (
        not isinstance(source, dict)
        or source.get("format") != "ue_asset_source_v1"
        or source.get("asset_class") not in ({"/Script/Engine.BlendSpace", "/Script/Engine.BlendSpace1D"} if evaluator else {"/Script/Engine.BlendSpace"})
        or source.get("asset_path") != blendspace.get("source_asset")
    ):
        raise AssetResolutionError("source BlendSpace is invalid")
    try:
        parameter_rows = asset_properties(source, "BlendParameters")
        if len(parameter_rows) != 3:
            raise AssetResolutionError("source BlendSpace axis count is unsupported")
        axes = []
        for row in parameter_rows:
            fields = parse_struct(row)
            reject_unknown_fields(fields, {"DisplayName", "Min", "Max", "GridNum"})
            axes.append(
                {
                    "name": parse_quoted(struct_field(fields, "DisplayName")),
                    "min": parse_number(struct_field(fields, "Min", "0.0")),
                    "max": parse_number(struct_field(fields, "Max")),
                    "grid_num": parse_number(struct_field(fields, "GridNum")),
                }
            )
        interpolation_rows = asset_properties(source, "InterpolationParam")
        if len(interpolation_rows) != 3:
            raise AssetResolutionError("source BlendSpace interpolation axis count is unsupported")
        filters = []
        for index, row in enumerate(interpolation_rows):
            fields = parse_struct(row)
            reject_unknown_fields(fields, {"InterpolationTime", "DampingRatio", "InterpolationType"})
            if parse_number(struct_field(fields, "DampingRatio", "1.0")) != 1.0:
                raise AssetResolutionError("source BlendSpace damping ratio is unsupported")
            interpolation = struct_field(fields, "InterpolationType")
            if evaluator:
                if parse_number(struct_field(fields, "InterpolationTime", "0.0")) != 0.0:
                    raise AssetResolutionError("source BlendSpace evaluator axis interpolation is unsupported")
                filters.append(0.0)
            elif index < 2:
                if interpolation != "BSIT_Cubic":
                    raise AssetResolutionError("source BlendSpace axis interpolation is unsupported")
                filters.append(parse_number(struct_field(fields, "InterpolationTime")))
            elif interpolation != "BSIT_SpringDamper":
                raise AssetResolutionError("source BlendSpace unused axis interpolation is unsupported")
        source_samples = [
            blendspace_source_sample(row)
            for row in parse_list(asset_property(source, "SampleData"))
        ]
        dimensions = [parse_number(value) for value in parse_list(asset_property(source, "DimensionIndices"))]
        if dimensions not in ([[0.0], [0.0, 1.0]] if evaluator else [[0.0, 1.0]]):
            raise AssetResolutionError("source BlendSpace dimensions are unsupported")
        if not evaluator and not parse_bool(asset_property(source, "bLoop")):
            raise AssetResolutionError("source BlendSpace non-looping asset is unsupported")
        if not parse_bool(asset_property(source, "bAllowMarkerBasedSync")):
            raise AssetResolutionError("source BlendSpace marker sync is unsupported")
        if parse_bool(asset_property(source, "bShouldMatchSyncPhases")):
            raise AssetResolutionError("source BlendSpace phase matching is unsupported")
        if parse_bool(asset_property(source, "bUseLegacySamplePointAnimationLengthCalculations")):
            raise AssetResolutionError("source BlendSpace legacy sample timing is unsupported")
        if parse_bool(asset_property(source, "bAllowMeshSpaceBlending")):
            raise AssetResolutionError("source BlendSpace mesh-space blending is unsupported")
        if parse_number(asset_property(source, "TargetWeightInterpolationSpeedPerSec")) != 0.0:
            raise AssetResolutionError("source BlendSpace target-weight interpolation is unsupported")
        if evaluator and (asset_property(source, "ManualPerBoneOverrides") not in ("", "()") or asset_property(source, "PerBoneBlendProfile") != "()"):
            raise AssetResolutionError("source BlendSpace evaluator per-bone interpolation is unsupported")
        if evaluator and parse_number(asset_property(source, "SampleIndexWithMarkers")) != -1.0:
            raise AssetResolutionError("source BlendSpace evaluator marker synchronization is unsupported")
        if parse_bool(asset_property(source, "bInterpolateUsingGrid")):
            raise AssetResolutionError("source BlendSpace grid interpolation is unsupported")
        if asset_property(source, "PreferredTriangulationDirection") != "Tangential":
            raise AssetResolutionError("source BlendSpace triangulation is unsupported")
        if asset_property(source, "NotifyTriggerMode") != "HighestWeightedAnimation":
            raise AssetResolutionError("source BlendSpace notify mode is unsupported")
        triangulation = blendspace_triangulation(source, axes, source_samples)
        if bool(triangulation["segments"]) != (dimensions == [0.0]):
            raise AssetResolutionError("source BlendSpace topology dimensions differ")
    except ExportedTextError as error:
        raise AssetResolutionError("source BlendSpace properties are invalid") from error
    target_axes = blendspace.get("axes")
    target_samples = blendspace.get("samples")
    if not isinstance(target_axes, list) or not isinstance(target_samples, list):
        raise AssetResolutionError("target BlendSpace metadata is invalid")
    if len(axes) != len(target_axes) or len(source_samples) != len(target_samples):
        raise AssetResolutionError("target BlendSpace metadata differs from source")
    for source_axis, target_axis in zip(axes, target_axes):
        if (
            not isinstance(target_axis, dict)
            or source_axis["name"] != target_axis.get("name")
            or any(
                abs(source_axis[field] - numeric(target_axis.get(field), "target BlendSpace axis")) > 1.0e-6
                for field in ("min", "max", "grid_num")
            )
        ):
            raise AssetResolutionError("target BlendSpace axis metadata differs from source")
    for source_sample, target_sample in zip(source_samples, target_samples):
        target_value = target_sample.get("value") if isinstance(target_sample, dict) else None
        if (
            not isinstance(target_value, list)
            or source_sample["source_animation"] != target_sample.get("source_animation")
            or len(target_value) != len(source_sample["value"])
            or any(
                abs(left - numeric(right, "target BlendSpace sample")) > 1.0e-6
                for left, right in zip(source_sample["value"], target_value)
            )
            or abs(
                source_sample["rate_scale"]
                - numeric(target_sample.get("rate_scale"), "target BlendSpace sample rate")
            )
            > 1.0e-6
        ):
            raise AssetResolutionError("target BlendSpace sample metadata differs from source")
    return {
        "axis_filter": {"x_window_seconds": filters[0], "y_window_seconds": filters[1]},
        "triangulation": triangulation,
    }


def node_property(node, name):
    values = [
        value.get("exported_text")
        for value in node.get("properties", [])
        if value.get("name") == name and isinstance(value.get("exported_text"), str)
    ]
    if len(values) != 1:
        raise AssetResolutionError(f"source node property is missing or ambiguous: {name}")
    return values[0]


def named_object_reference(value, field):
    match = re.search(rf"{re.escape(field)}=\"[^']*'(/[^']+)'\"", value)
    if match is None:
        raise AssetResolutionError(f"source node object reference is missing: {field}")
    return match.group(1)


def visible_input_pins(node):
    return [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == "Input" and not pin.get("hidden")
    ]


def visible_output_pins(node):
    return [
        pin
        for pin in node.get("pins", [])
        if pin.get("direction") == "Output" and not pin.get("hidden")
    ]


def blendspace_player_contract(node, blendspace, asset_contract):
    source_node = node_property(node, "Node")
    try:
        fields = parse_struct(source_node)
        reject_unknown_fields(
            fields,
            {
                "GroupName",
                "GroupRole",
                "Method",
                "BlendSpace",
                "X",
                "Y",
                "PlayRate",
                "bLoop",
                "bResetPlayTimeWhenBlendSpaceChanges",
                "StartPosition",
                "bIgnoreForRelevancyTest",
                "bOverridePositionWhenJoiningSyncGroupAsLeader",
                "InitialUpdateFunction",
                "BecomeRelevantFunction",
                "UpdateFunction",
            },
        )
        if named_object_reference(source_node, "BlendSpace") != blendspace.get("source_asset"):
            raise AssetResolutionError("source BlendSpace player asset does not match metadata")
        for name in ("InitialUpdateFunction", "BecomeRelevantFunction", "UpdateFunction"):
            if struct_field(fields, name, "()") != "()":
                raise AssetResolutionError("source BlendSpace player lifecycle is unsupported")
        group = source_name(struct_field(fields, "GroupName", "None"))
        role = source_name(struct_field(fields, "GroupRole", "CanBeLeader"))
        method = source_name(struct_field(fields, "Method", "DoNotSync"))
        if group == "None" or method != "SyncGroup" or role != "CanBeLeader":
            raise AssetResolutionError("source BlendSpace player sync contract is unsupported")
        if parse_bool(struct_field(fields, "bIgnoreForRelevancyTest", "False")):
            raise AssetResolutionError("source BlendSpace player relevancy override is unsupported")
        if parse_bool(struct_field(fields, "bOverridePositionWhenJoiningSyncGroupAsLeader", "False")):
            raise AssetResolutionError("source BlendSpace player join override is unsupported")
        loop = parse_bool(struct_field(fields, "bLoop", "True"))
        if not loop:
            raise AssetResolutionError("source BlendSpace player non-looping playback is unsupported")
        reset_play_time = parse_bool(
            struct_field(fields, "bResetPlayTimeWhenBlendSpaceChanges", "True")
        )
        if not reset_play_time:
            raise AssetResolutionError("source BlendSpace player reset mode is unsupported")
        start_position = parse_number(struct_field(fields, "StartPosition", "0.0"))
        if not 0.0 <= start_position <= 1.0:
            raise AssetResolutionError("source BlendSpace player start position is invalid")
        configured_play_rate = parse_number(struct_field(fields, "PlayRate", "1.0"))
    except ExportedTextError as error:
        raise AssetResolutionError("source BlendSpace player properties are invalid") from error
    inputs = visible_input_pins(node)
    if [pin.get("name") for pin in inputs] != ["X", "Y", "PlayRate"]:
        raise AssetResolutionError("source BlendSpace player input schema is unsupported")
    if [pin.get("name") for pin in visible_output_pins(node)] != ["Pose"]:
        raise AssetResolutionError("source BlendSpace player output schema is unsupported")
    axis_names = [axis.get("name") for axis in blendspace.get("axes", [])]
    expected_labels = [*axis_names[:2], "Play Rate"]
    if len(axis_names) != 3 or axis_names[2] != "None" or [
        pin.get("friendly_name") for pin in inputs
    ] != expected_labels:
        raise AssetResolutionError("source BlendSpace player pin labels are unsupported")
    if inputs[2].get("linked_to"):
        raise AssetResolutionError("source BlendSpace player linked play rate is unsupported")
    for pin in inputs:
        source_pin_number(pin.get("default_value"), f"source BlendSpace player input {pin['name']}")
    samples = blendspace.get("samples")
    if not isinstance(samples, list) or not samples:
        raise AssetResolutionError("source BlendSpace player samples are unavailable")
    for index, sample in enumerate(samples):
        timing = sample.get("source_timing") if isinstance(sample, dict) else None
        markers = timing.get("markers") if isinstance(timing, dict) else None
        if (
            not isinstance(timing, dict)
            or not isinstance(timing.get("duration"), (int, float))
            or timing["duration"] <= 0.0
            or not isinstance(markers, list)
            or len(markers) < 2
            or any(
                not isinstance(marker, dict)
                or not isinstance(marker.get("name"), str)
                or not marker["name"]
                or not isinstance(marker.get("time"), (int, float))
                for marker in markers
            )
        ):
            raise AssetResolutionError(f"source BlendSpace player sample timing is unsupported: {index}")
    if not isinstance(asset_contract, dict) or not isinstance(asset_contract.get("axis_filter"), dict):
        raise AssetResolutionError("source BlendSpace player asset contract is unavailable")
    return {
        "input_pins": [
            {
                "name": pin["name"],
                "axis": axis_names[index] if index < 2 else None,
                "default_value": source_pin_number(
                    pin.get("default_value"), f"source BlendSpace player input {pin['name']}"
                ),
            }
            for index, pin in enumerate(inputs)
        ],
        "sync_group": group,
        "sync_group_role": role,
        "sync_method": method,
        "loop": loop,
        "reset_play_time_when_asset_changes": reset_play_time,
        "start_position": start_position,
        "configured_play_rate": configured_play_rate,
        "axis_filter": asset_contract["axis_filter"],
        "triangulation": asset_contract["triangulation"],
    }


def blendspace_evaluator_contract(node, blendspace):
    source_node = node_property(node, "Node")
    try:
        fields = parse_struct(source_node)
        teleport = parse_bool(struct_field(fields, "bTeleportToNormalizedTime", "True"))
        loop = parse_bool(struct_field(fields, "bLoop", "True"))
        reset_play_time = parse_bool(
            struct_field(fields, "bResetPlayTimeWhenBlendSpaceChanges", "True")
        )
        configured_play_rate = parse_number(struct_field(fields, "PlayRate", "1.0"))
        normalized_time = parse_number(struct_field(fields, "NormalizedTime", "0.0"))
    except ExportedTextError as error:
        raise AssetResolutionError(f"blendspace evaluator source properties are invalid: {error}") from error
    input_pins = visible_input_pins(node)
    input_names = [pin.get("name") for pin in input_pins]
    axis_names = [axis["name"] for axis in blendspace["axes"]]
    axis_count = 1 if len(axis_names) >= 2 and axis_names[1] == "None" else 2
    if (
        len(axis_names) < axis_count
        or any(name != "None" for name in axis_names[axis_count:])
    ):
        raise AssetResolutionError("blendspace evaluator dimensionality is unsupported")
    axis_pins = ["X"] if axis_count == 1 else ["X", "Y"]
    dynamic_normalized_time = input_names == [*axis_pins, "NormalizedTime"]
    if input_names not in (axis_pins, [*axis_pins, "NormalizedTime"]):
        raise AssetResolutionError("blendspace evaluator input schema is unsupported")
    for index, pin in enumerate(input_pins[:axis_count]):
        if pin.get("friendly_name") != axis_names[index]:
            raise AssetResolutionError("blendspace evaluator axis pin does not match metadata")
        source_pin_number(pin.get("default_value"), f"blendspace evaluator input {pin['name']} default")
    if dynamic_normalized_time:
        normalized_time_pin = input_pins[-1]
        if normalized_time_pin.get("friendly_name") != "Normalized Time":
            raise AssetResolutionError("blendspace evaluator normalized time label is unsupported")
        source_pin_number(
            normalized_time_pin.get("default_value"),
            "blendspace evaluator normalized time default",
        )
    for index, sample in enumerate(blendspace["samples"]):
        if any(abs(value) > 1.0e-8 for value in sample["value"][2:]):
            raise AssetResolutionError(f"blendspace evaluator sample {index} uses unsupported axis")
    if not teleport:
        raise AssetResolutionError("blendspace evaluator without teleport time is unsupported")
    return {
        "input_pins": [
            {
                "name": pin["name"],
                "axis": axis_names[index],
                "default_value": source_pin_number(pin.get("default_value"), f"blendspace evaluator input {pin['name']} default"),
            }
            for index, pin in enumerate(input_pins[:axis_count])
        ]
        + (
            [
                {
                    "name": "NormalizedTime",
                    "axis": None,
                    "default_value": source_pin_number(
                        input_pins[-1].get("default_value"),
                        "blendspace evaluator normalized time default",
                    ),
                }
            ]
            if dynamic_normalized_time
            else []
        ),
        "normalized_time": normalized_time,
        "teleport_to_normalized_time": teleport,
        "loop": loop,
        "reset_play_time_when_asset_changes": reset_play_time,
        "configured_play_rate": configured_play_rate,
    }


def resolve_graph_animation_assets(
    asset_root,
    source,
    source_content_root=None,
    source_mount=None,
    source_reader=None,
    reachable_nodes=None,
):
    rows = []
    timings = {}
    curves = {}
    native_blendspaces = {}
    for graph in source.get("graphs", []):
        for node in graph.get("nodes", []):
            if reachable_nodes is not None and node.get("object_path") not in reachable_nodes:
                continue
            class_path = node.get("class_path")
            if class_path in {
                "/Script/AnimGraph.AnimGraphNode_SequencePlayer",
                "/Script/AnimGraph.AnimGraphNode_SequenceEvaluator",
            }:
                source_path = named_object_reference(node_property(node, "Node"), "Sequence")
                target = resolve_animation_clip(asset_root, source_path)
                category = "sequence"
            elif class_path == "/Script/AnimGraph.AnimGraphNode_LocalRefPose":
                source_path = source_object_path(asset_property(source, "TargetSkeleton"))
                target = resolve_reference_skeleton(asset_root, source_path)
                category = "skeleton"
            elif class_path == "/Script/AnimGraph.AnimGraphNode_BlendSpaceEvaluator":
                source_path = named_object_reference(node_property(node, "Node"), "BlendSpace")
                blendspace = resolve_blendspace_contract(asset_root, source_path)
                evaluator = blendspace_evaluator_contract(node, blendspace)
                if source_content_root is not None:
                    native = native_blendspaces.get(source_path)
                    if native is None:
                        native = blendspace_asset_contract(native_source_asset(source_path, source_reader), blendspace, evaluator=True)
                        native_blendspaces[source_path] = native
                    evaluator.update(native)
                target = blendspace["target_asset"]
                category = "blendspace"
            elif class_path == "/Script/AnimGraph.AnimGraphNode_BlendSpacePlayer":
                if source_content_root is None or source_mount is None:
                    raise AssetResolutionError("source BlendSpace player timing root is unavailable")
                source_path = named_object_reference(node_property(node, "Node"), "BlendSpace")
                blendspace = resolve_blendspace_contract(asset_root, source_path)
                native = native_blendspaces.get(source_path)
                if native is None:
                    native = blendspace_asset_contract(
                        native_source_asset(source_path, source_reader), blendspace
                    )
                    native_blendspaces[source_path] = native
                for sample in blendspace["samples"]:
                    animation = sample["source_animation"]
                    timing = timings.get(animation)
                    if timing is None:
                        timing = source_sequence_timing(
                            source_content_root, source_mount, animation, source_reader
                        )
                        timings[animation] = timing
                    sample["source_timing"] = timing
                player = blendspace_player_contract(node, blendspace, native)
                target = blendspace["target_asset"]
                category = "blendspace"
            else:
                continue
            row = {
                "source_graph": graph.get("object_path"),
                "source_node": node.get("object_path"),
                "source_class": class_path,
                "category": category,
                "source_asset": source_path,
                "target_asset": target,
            }
            if category == "sequence":
                row["target_skeleton"] = target_clip_skeleton(asset_root, target)
            if source_content_root is not None and category in {"sequence", "blendspace"}:
                samples = blendspace["samples"] if category == "blendspace" else [row]
                for sample in samples:
                    animation = sample.get("source_animation", source_path)
                    if animation not in curves:
                        curves[animation] = source_sequence_curves(source_content_root, source_mount, animation, source_reader)
                    sample["source_curves"] = curves[animation]
            if class_path == "/Script/AnimGraph.AnimGraphNode_BlendSpaceEvaluator":
                row["blendspace"] = blendspace
                row["evaluator"] = evaluator
            if class_path == "/Script/AnimGraph.AnimGraphNode_BlendSpacePlayer":
                row["blendspace"] = blendspace
                row["player"] = player
            if category == "sequence" and source_content_root is not None:
                timing = timings.get(source_path)
                if timing is None:
                    timing = source_sequence_timing(
                        source_content_root, source_mount, source_path, source_reader
                    )
                    timings[source_path] = timing
                row["source_timing"] = timing
            rows.append(row)
    return rows


def run_self_test():
    from unittest.mock import patch
    from types import SimpleNamespace
    with tempfile.TemporaryDirectory() as directory:
        content = Path(directory)
        (content / "Sequence.uasset").write_bytes(b"fixture")
        legacy_source = {"format":"ue_asset_source_v1", "asset_path":"/ALS/Sequence.Sequence",
            "asset_class":"/Script/Engine.AnimSequence", "asset_properties":[
                {"name":"RawCurveData", "exported_text":"()"},
                {"name":"SequenceLength", "exported_text":"1.0"},
                {"name":"SamplingFrameRate", "exported_text":"(Numerator=30,Denominator=1)"},
                {"name":"AuthoredSyncMarkers", "exported_text":""}]}
        with patch(__name__ + ".CurveUAsset", side_effect=CurveParseError("legacy property format")), \
             patch(__name__ + ".native_source_asset", return_value=legacy_source) as native:
            assert source_sequence_curves(content, "/ALS", legacy_source["asset_path"], "fixture_reader") == []
            native.assert_called_once_with(legacy_source["asset_path"], "fixture_reader")
        with patch(__name__ + ".CurveUAsset", side_effect=CurveParseError("legacy property format")), \
             patch(__name__ + ".native_source_asset", side_effect=AssetResolutionError("native failed")):
            try:
                source_sequence_curves(content, "/ALS", legacy_source["asset_path"])
            except AssetResolutionError:
                pass
            else:
                raise AssertionError("A failed native curve read was accepted as an empty curve list")
        weighted = '(CurveName="Weighted",CurveTypeFlags=20,FloatCurve=(DefaultValue=0.25,PreInfinityExtrap=RCCE_Linear,PostInfinityExtrap=RCCE_Cycle,Keys=((Time=0.5,Value=0.75,InterpMode=RCIM_Cubic,TangentMode=RCTM_User,TangentWeightMode=RCTWM_WeightedBoth,ArriveTangent=2.0,LeaveTangent=3.0,ArriveTangentWeight=0.1,LeaveTangentWeight=0.2))))'
        weighted_source = {**legacy_source, "asset_properties":[{"name":"RawCurveData", "exported_text":"(FloatCurves=(" + weighted + "))"}]}
        curve = sequence_curves_contract(weighted_source)[0]
        assert curve == {"name":"Weighted", "flags":20, "default":0.25, "pre_infinity":"RCCE_Linear", "post_infinity":"RCCE_Cycle",
            "interps":["CUBIC"], "times":[0.5], "values":[0.75], "tangent_modes":[1], "weight_modes":[3],
            "arrive_tangents":[2.0], "leave_tangents":[3.0], "arrive_weights":[0.1], "leave_weights":[0.2]}
        for invalid in ({**weighted_source, "asset_class":"/Script/Engine.Skeleton"},
            {**weighted_source, "asset_properties":[]},
            {**weighted_source, "asset_properties":[{"name":"RawCurveData", "exported_text":"(FloatCurves=(" + weighted + "," + weighted + "))"}]},
            {**weighted_source, "asset_properties":[{"name":"RawCurveData", "exported_text":"(FloatCurves=(" + weighted.replace('RCIM_Cubic', 'RCIM_Invalid') + "))"}]}):
            try:
                sequence_curves_contract(invalid)
            except AssetResolutionError:
                pass
            else:
                raise AssertionError("Invalid native sequence curves were accepted")
        asset = SimpleNamespace(exports=[SimpleNamespace(class_index=-1, object_name="Sequence")],
            imports=[SimpleNamespace(class_name="Class", object_name="AnimSequence")])
        with patch(__name__ + ".UAsset", return_value=asset), \
             patch(__name__ + ".PropReader") as reader, \
             patch(__name__ + ".native_source_asset", return_value=legacy_source):
            reader.return_value.read_export.return_value = ({}, False, "legacy tag")
            assert source_sequence_timing(content, "/ALS", legacy_source["asset_path"])["duration"] == 1.0
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        skeleton = root / "Reference.static.skn.ron"
        mesh = root / "Reference.gltf"
        mesh.write_text("{}", encoding="utf-8")
        skeleton.write_text('(source: Gltf(source: "Reference.gltf", label: "Scene0"), root: "root", source_skeleton: "/Source/SK_Test.SK_Test")', encoding="utf-8")
        reference_source = {"asset_properties": [{"name": "TargetSkeleton", "exported_text": "/Script/Engine.Skeleton'/Source/SK_Test.SK_Test'"}],
            "graphs": [{"object_path": "/Source:Graph", "nodes": [{"object_path": "/Source:Graph.Reference", "class_path": "/Script/AnimGraph.AnimGraphNode_LocalRefPose"}]}]}
        rows = resolve_graph_animation_assets(root, reference_source)
        assert len(rows) == 1 and rows[0]["category"] == "skeleton"
        assert rows[0]["target_asset"] == "Reference.static.skn.ron"
        reference_source["graphs"][0]["nodes"][0]["object_path"] = "/Other:Renamed.Node"
        renamed = resolve_graph_animation_assets(root, reference_source)
        assert renamed[0] == rows[0] | {"source_node": "/Other:Renamed.Node"}
        duplicate = root / "Duplicate.static.skn.ron"
        duplicate.write_text(skeleton.read_text(), encoding="utf-8")
        try:
            resolve_graph_animation_assets(root, reference_source)
        except AssetResolutionError:
            pass
        else:
            raise AssertionError("Ambiguous source skeleton was accepted")
        duplicate.unlink()
        mesh.unlink()
        assert resolve_graph_animation_assets(root, reference_source, reachable_nodes=set()) == []
        try:
            resolve_graph_animation_assets(root, reference_source)
        except AssetResolutionError:
            pass
        else:
            raise AssertionError("Missing skeleton mesh was accepted")
    for mode in ("RefPose", "AnimFirstFrame", "Zero"):
        assert sequence_root_lock("True", mode) == mode
        assert sequence_root_lock(True, {"value": "ERootMotionRootLock::" + mode}) == mode
        assert sequence_root_lock(False, mode) is None
    for value in ("(Numerator=30000,Denominator=1001)", {"Numerator": 30000, "Denominator": 1001}, {"__tagged__": {"Numerator": 30000, "Denominator": 1001}}):
        assert sampling_frame_rate(value) == 30000.0 / 1001.0
    assert sampling_frame_rate({"Numerator": 60}) == 60.0
    for value in (None, {}, {"Numerator": 30, "Denominator": 0}, {"Numerator": 0}, {"Numerator": True}):
        try:
            sampling_frame_rate(value)
        except (AssetResolutionError, ExportedTextError):
            pass
        else:
            raise AssertionError("invalid source sampling frame rate was accepted")
    fixture_root = Path(__file__).parent / "fixtures"
    for fixture in ("ue_standing_walk_forward_curves", "ue_standing_rotate_left_curves"):
        curves = json.loads((fixture_root / f"{fixture}.json").read_text())["curves"]
        native_text = (fixture_root / f"{fixture}.ue.txt").read_text().strip()
        native = parse_struct(native_text)
        converted = sequence_curves_contract({"format":"ue_asset_source_v1", "asset_class":"/Script/Engine.AnimSequence",
            "asset_properties":[{"name":"RawCurveData", "exported_text":native_text}]})
        assert len(converted) == len(curves)
        for actual, expected in zip(converted, curves, strict=True):
            for field in ("name", "flags", "default", "pre_infinity", "post_infinity", "interps"):
                assert actual[field] == expected[field], (fixture, field, actual[field], expected[field])
            for field in ("times", "values"):
                assert len(actual[field]) == len(expected[field])
                assert all(abs(a-b) < 5e-7 for a,b in zip(actual[field], expected[field], strict=True))
        for raw_curve, curve in zip(parse_list(struct_field(native, "FloatCurves")), curves, strict=True):
            fields = parse_struct(raw_curve)
            assert parse_quoted(struct_field(fields, "CurveName")) == curve["name"]
            rich = parse_struct(struct_field(fields, "FloatCurve"))
            keys = [rich_curve_key(key) for key in parse_list(struct_field(rich, "Keys"))]
            assert len(keys) == len(curve["times"])
            for index, key in enumerate(keys):
                assert key["interpolation"].upper() == curve["interps"][index]
                fields = [("t", "times"), ("v", "values")]
                if key["interpolation"] == "Cubic":
                    fields.extend([("leave_tangent", "leave_tangents"), ("leave_weight", "leave_weights")])
                if index > 0 and keys[index - 1]["interpolation"] == "Cubic":
                    fields.extend([("arrive_tangent", "arrive_tangents"), ("arrive_weight", "arrive_weights")])
                for source_field, target_field in fields:
                    assert abs(key[source_field] - curve.get(target_field, [0.0] * len(keys))[index]) < 5e-7
    native_source_asset_cached.cache_clear()
    reader_calls = []
    original_run = subprocess.run
    try:
        def fake_reader(command, **_kwargs):
            source = command[command.index("-Asset") + 1]
            output = Path(command[command.index("-Output") + 1])
            output.write_text(
                json.dumps(
                    {
                        "format": "ue_asset_source_v1",
                        "asset_path": source,
                    }
                ),
                encoding="utf-8",
            )
            reader_calls.append((source, command[command.index("-File") + 1]))
            return type("ReaderResult", (), {"returncode": 0})()

        subprocess.run = fake_reader
        first = native_source_asset("/ALS/Test.A_Test")
        second = native_source_asset("/ALS/Test.A_Test", DEFAULT_SOURCE_READER)
        assert first == second and len(reader_calls) == 1
    finally:
        subprocess.run = original_run
        native_source_asset_cached.cache_clear()
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "assets"
        clip = root / "animations" / "Air" / "A_Test.anim.ron"
        clip.parent.mkdir(parents=True)
        clip.write_text("()\n", encoding="utf-8")
        blendspace = root / "animations" / "blendspaces" / "BS_Test.json"
        blendspace.parent.mkdir(parents=True)
        blendspace.write_text(
            json.dumps(
                {
                    "asset": "/ALS/Animations/BS_Test",
                    "axes": [
                        {"name": "X", "min": -1.0, "max": 1.0, "grid_num": 2},
                        {"name": "Y", "min": -1.0, "max": 1.0, "grid_num": 2},
                    ],
                    "samples": [
                        {
                            "animation": "/ALS/Animations/A_Test.A_Test",
                            "value": [0.0, 0.0, 0.0],
                            "rate_scale": 1.0,
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        (root / "metadata.json").write_text("[]", encoding="utf-8")
        assert source_object_path("Sequence=\"/Script/Engine.AnimSequence'/ALS/Animations/A_Test.A_Test'\"") == "/ALS/Animations/A_Test.A_Test"
        assert resolve_animation_clip(root, "/ALS/Animations/A_Test.A_Test") == "animations/Air/A_Test.anim.ron"
        contract = resolve_blendspace_contract(root, "/ALS/Animations/BS_Test.BS_Test")
        assert contract["target_asset"] == "animations/blendspaces/BS_Test.json"
        assert contract["axes"][0]["name"] == "X"
        assert contract["samples"][0]["target_clip"] == "animations/Air/A_Test.anim.ron"
        duplicate = root / "other" / "A_Test.anim.ron"
        duplicate.parent.mkdir(parents=True)
        duplicate.write_text("()\n", encoding="utf-8")
        try:
            resolve_animation_clip(root, "/ALS/Animations/A_Test.A_Test")
        except AssetResolutionError:
            pass
        else:
            raise AssertionError("ambiguous animation target was accepted")
    with tempfile.TemporaryDirectory() as directory:
        content = Path(directory) / "Content"
        source = content / "ALS" / "Animations" / "A_Test.uasset"
        source.parent.mkdir(parents=True)
        source.write_bytes(b"fixture")
        assert source_uasset_path(
            content,
            "/ALS",
            "/ALS/ALS/Animations/A_Test.A_Test",
        ) == source.resolve()
    source = {
        "graphs": [
            {
                "object_path": "/Asset:AnimGraph",
                "nodes": [
                    {
                        "object_path": "/Asset:AnimGraph.Player",
                        "class_path": "/Script/AnimGraph.AnimGraphNode_SequencePlayer",
                        "properties": [
                            {
                                "name": "Node",
                                "exported_text": "(Sequence=\"/Script/Engine.AnimSequence'/ALS/Animations/A_Test.A_Test'\")",
                            }
                        ],
                    }
                ],
            }
        ]
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "assets"
        clip = root / "animations" / "A_Test.anim.ron"
        clip.parent.mkdir(parents=True)
        clip.write_text("()\n", encoding="utf-8")
        rows = resolve_graph_animation_assets(root, source)
        assert rows[0]["target_asset"] == "animations/A_Test.anim.ron"
        assert rows[0]["target_skeleton"] is None
        skeleton = root / "skeletons" / "Test.skn.ron"
        skeleton.parent.mkdir()
        skeleton.write_text("()\n", encoding="utf-8")
        clip.write_text('(skeleton: "skeletons/Test.skn.ron")\n', encoding="utf-8")
        assert target_clip_skeleton(root, "animations/A_Test.anim.ron") == "skeletons/Test.skn.ron"
        for text in ['(skeleton: "../outside.skn.ron")', '(skeleton: "missing.skn.ron")', '(skeleton: "a", skeleton: "b")']:
            clip.write_text(text, encoding="utf-8")
            try:
                target_clip_skeleton(root, "animations/A_Test.anim.ron")
                raise AssertionError("invalid reference skeleton was accepted")
            except AssetResolutionError:
                pass
    blendspace_source = {
        "graphs": [
            {
                "object_path": "/Asset:AnimGraph",
                "nodes": [
                    {
                        "object_path": "/Asset:AnimGraph.BlendSpace",
                        "class_path": "/Script/AnimGraph.AnimGraphNode_BlendSpaceEvaluator",
                        "properties": [
                            {
                                "name": "Node",
                                "exported_text": "(bTeleportToNormalizedTime=True,PlayRate=1.000000,bLoop=True,bResetPlayTimeWhenBlendSpaceChanges=True,BlendSpace=\"/Script/Engine.BlendSpace'/ALS/Animations/BS_Test.BS_Test'\")",
                            }
                        ],
                        "pins": [
                            {
                                "name": "X",
                                "direction": "Input",
                                "hidden": False,
                                "friendly_name": "X",
                                "default_value": "0.000000",
                            },
                            {
                                "name": "Y",
                                "direction": "Input",
                                "hidden": False,
                                "friendly_name": "Y",
                                "default_value": "0.000000",
                            },
                        ],
                    }
                ],
            }
        ]
    }
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "assets"
        clip = root / "animations" / "A_Test.anim.ron"
        clip.parent.mkdir(parents=True)
        clip.write_text("()\n", encoding="utf-8")
        metadata = root / "blendspaces" / "BS_Test.json"
        metadata.parent.mkdir(parents=True)
        metadata.write_text(
            json.dumps(
                {
                    "asset": "/ALS/Animations/BS_Test",
                    "axes": [
                        {"name": "X", "min": -1.0, "max": 1.0, "grid_num": 2},
                        {"name": "Y", "min": -1.0, "max": 1.0, "grid_num": 2},
                    ],
                    "samples": [
                        {
                            "animation": "/ALS/Animations/A_Test.A_Test",
                            "value": [0.0, 0.0, 0.0],
                            "rate_scale": 1.0,
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        rows = resolve_graph_animation_assets(root, blendspace_source)
        assert rows[0]["evaluator"]["teleport_to_normalized_time"]
        assert rows[0]["evaluator"]["input_pins"][1]["axis"] == "Y"
        one_dimensional = blendspace_evaluator_contract(
            {
                "properties": [
                    {
                        "name": "Node",
                        "exported_text": "(bTeleportToNormalizedTime=True,bLoop=True,PlayRate=1.000000)",
                    }
                ],
                "pins": [
                    {
                        "name": "X",
                        "direction": "Input",
                        "hidden": False,
                        "friendly_name": "Pitch Angle",
                        "default_value": "0.000000",
                    },
                    {
                        "name": "NormalizedTime",
                        "direction": "Input",
                        "hidden": False,
                        "friendly_name": "Normalized Time",
                        "default_value": "0.000000",
                    },
                    {"name": "Pose", "direction": "Output", "hidden": False},
                ],
            },
            {
                "axes": [
                    {"name": "Pitch Angle"},
                    {"name": "None"},
                    {"name": "None"},
                ],
                "samples": [{"value": [0.0, 0.0, 0.0]}],
            },
        )
        assert [pin["name"] for pin in one_dimensional["input_pins"]] == [
            "X",
            "NormalizedTime",
        ]
        blendspace_source["graphs"][0]["nodes"][0]["pins"].append(
            {
                "name": "Z",
                "direction": "Input",
                "hidden": False,
                "friendly_name": "Z",
                "default_value": "0.000000",
            }
        )
        try:
            resolve_graph_animation_assets(root, blendspace_source)
        except AssetResolutionError:
            pass
        else:
            raise AssertionError("unsupported blendspace evaluator input was accepted")
    player_blendspace = {
        "source_asset": "/ALS/Animations/BS_Test.BS_Test",
        "axes": [
            {"name": "Stride", "min": 0.0, "max": 1.0, "grid_num": 1.0},
            {"name": "WalkRun", "min": 0.0, "max": 1.0, "grid_num": 1.0},
            {"name": "None", "min": 0.0, "max": 100.0, "grid_num": 4.0},
        ],
        "samples": [
            {
                "source_animation": "/ALS/Animations/A_Test.A_Test",
                "target_clip": "animations/A_Test.anim.ron",
                "value": [0.0, 0.0, 0.0],
                "rate_scale": 1.0,
                "source_timing": {
                    "duration": 1.0,
                    "markers": [{"name": "Left", "time": 0.0}, {"name": "Right", "time": 0.5}],
                },
            }
        ],
    }
    player_asset_properties = [
        {"name": "BlendParameters", "exported_text": '(DisplayName="Stride",Max=1.000000,GridNum=1)'},
        {"name": "BlendParameters", "exported_text": '(DisplayName="WalkRun",Max=1.000000,GridNum=1)'},
        {"name": "BlendParameters", "exported_text": '(DisplayName="None",Max=100.000000,GridNum=4)'},
        {"name": "InterpolationParam", "exported_text": '(InterpolationTime=0.200000,DampingRatio=1.000000,InterpolationType=BSIT_Cubic)'},
        {"name": "InterpolationParam", "exported_text": '(InterpolationTime=0.400000,DampingRatio=1.000000,InterpolationType=BSIT_Cubic)'},
        {"name": "InterpolationParam", "exported_text": '(DampingRatio=1.000000,InterpolationType=BSIT_SpringDamper)'},
        {"name": "SampleData", "exported_text": '((Animation="/Script/Engine.AnimSequence\'/ALS/Animations/A_Test.A_Test\'"))'},
        {"name": "DimensionIndices", "exported_text": '(0,1)'},
        {"name": "bLoop", "exported_text": "True"},
        {"name": "bAllowMarkerBasedSync", "exported_text": "True"},
        {"name": "bShouldMatchSyncPhases", "exported_text": "False"},
        {"name": "bUseLegacySamplePointAnimationLengthCalculations", "exported_text": "False"},
        {"name": "bAllowMeshSpaceBlending", "exported_text": "False"},
        {"name": "TargetWeightInterpolationSpeedPerSec", "exported_text": "0.000000"},
        {"name": "bInterpolateUsingGrid", "exported_text": "False"},
        {"name": "PreferredTriangulationDirection", "exported_text": "Tangential"},
        {"name": "NotifyTriggerMode", "exported_text": "HighestWeightedAnimation"},
        {"name": "BlendSpaceData", "exported_text": "(Triangles=((SampleIndices[0]=0,SampleIndices[1]=0,SampleIndices[2]=0)))"},
    ]
    player_source = {
        "format": "ue_asset_source_v1",
        "asset_path": "/ALS/Animations/BS_Test.BS_Test",
        "asset_class": "/Script/Engine.BlendSpace",
        "asset_properties": player_asset_properties,
    }
    player_asset = blendspace_asset_contract(player_source, player_blendspace)
    assert player_asset["axis_filter"] == {"x_window_seconds": 0.2, "y_window_seconds": 0.4}
    assert player_asset["triangulation"]["triangles"] == [[0, 0, 0]]
    evaluator_source = json.loads(json.dumps(player_source))
    evaluator_source["asset_class"] = "/Script/Engine.BlendSpace1D"
    evaluator_blendspace = json.loads(json.dumps(player_blendspace))
    evaluator_blendspace["axes"] = [
        {"name": "Pitch", "min": -90.0, "max": 90.0, "grid_num": 4.0},
        {"name": "None", "min": 0.0, "max": 100.0, "grid_num": 4.0},
        {"name": "None", "min": 0.0, "max": 100.0, "grid_num": 4.0},
    ]
    evaluator_blendspace["samples"] = [dict(player_blendspace["samples"][0], value=[pitch, 0.0, 0.0]) for pitch in [0.0, -90.0, 90.0]]
    evaluator_properties = evaluator_source["asset_properties"]
    for index in range(3):
        axis = evaluator_blendspace["axes"][index]
        evaluator_properties[index]["exported_text"] = f'(DisplayName="{axis["name"]}",Min={axis["min"]},Max={axis["max"]},GridNum=4)'
        evaluator_properties[index + 3]["exported_text"] = '(DampingRatio=1.000000,InterpolationType=BSIT_SpringDamper)'
    replacements = {
        "SampleData": '(' + ','.join(f'(Animation="/Script/Engine.AnimSequence\'/ALS/Animations/A_Test.A_Test\'",SampleValue=(X={pitch}))' for pitch in [0.0, -90.0, 90.0]) + ')',
        "DimensionIndices": '(0)',
        "bLoop": 'False',
        "BlendSpaceData": '(Segments=((SampleIndices[0]=1,SampleIndices[1]=0,Vertices[1]=0.500000),(SampleIndices[0]=0,SampleIndices[1]=2,Vertices[0]=0.500000,Vertices[1]=1.000000)))',
    }
    for row in evaluator_properties:
        row["exported_text"] = replacements.get(row["name"], row["exported_text"])
    evaluator_properties.extend({"name": name, "exported_text": value} for name, value in [
        ("ManualPerBoneOverrides", ""), ("PerBoneBlendProfile", "()"), ("SampleIndexWithMarkers", "-1"),
    ])
    evaluator_asset = blendspace_asset_contract(evaluator_source, evaluator_blendspace, evaluator=True)
    assert evaluator_asset["axis_filter"] == {"x_window_seconds": 0.0, "y_window_seconds": 0.0}
    assert evaluator_asset["triangulation"] == {"axis_min": [-90.0, 0.0], "axis_max": [90.0, 100.0], "triangles": [], "segments": [[1, 0], [0, 2]]}
    evaluator_source["asset_path"] = evaluator_blendspace["source_asset"] = '/Renamed/Fixture.Fixture'
    assert blendspace_asset_contract(evaluator_source, evaluator_blendspace, evaluator=True) == evaluator_asset
    segment_row = next(row for row in evaluator_properties if row["name"] == "BlendSpaceData")
    segment_row["exported_text"] = segment_row["exported_text"].replace('Vertices[1]=1.000000', 'Vertices[1]=0.700000')
    try:
        blendspace_asset_contract(evaluator_source, evaluator_blendspace, evaluator=True)
    except AssetResolutionError:
        pass
    else:
        raise AssertionError("source segment vertex drift was accepted")
    player_node = {
        "properties": [
            {
                "name": "Node",
                "exported_text": '(GroupName="Movement",Method=SyncGroup,PlayRate=1.000000,bLoop=True,bResetPlayTimeWhenBlendSpaceChanges=True,StartPosition=0.200000,BlendSpace="/Script/Engine.BlendSpace\'/ALS/Animations/BS_Test.BS_Test\'",InitialUpdateFunction=(),BecomeRelevantFunction=(),UpdateFunction=())',
            }
        ],
        "pins": [
            {"name": "X", "direction": "Input", "hidden": False, "friendly_name": "Stride", "default_value": "0.000000", "linked_to": []},
            {"name": "Y", "direction": "Input", "hidden": False, "friendly_name": "WalkRun", "default_value": "0.000000", "linked_to": []},
            {"name": "PlayRate", "direction": "Input", "hidden": False, "friendly_name": "Play Rate", "default_value": "1.000000", "linked_to": []},
            {"name": "Pose", "direction": "Output", "hidden": False},
        ],
    }
    player = blendspace_player_contract(player_node, player_blendspace, player_asset)
    assert player["sync_group"] == "Movement" and player["axis_filter"]["y_window_seconds"] == 0.4
    player_asset_properties[8]["exported_text"] = "False"
    try:
        blendspace_asset_contract(player_source, player_blendspace)
    except AssetResolutionError:
        pass
    else:
        raise AssertionError("non-looping source BlendSpace was accepted")
    assert source_blend_profile_wrapper(None, None, "(bIsSkeletonBlendProfile=True)") == {
        "source_profile": None,
        "bone_scales": [],
    }
    try:
        source_blend_profile_wrapper(None, None, "(bIsSkeletonBlendProfile=False)")
    except AssetResolutionError:
        pass
    else:
        raise AssertionError("non-skeleton blend profile was accepted")
    sequence_source = {
        "format": "ue_asset_source_v1",
        "asset_path": "/ALS/ALS/Animations/A_Test.A_Test",
        "asset_class": "/Script/Engine.AnimSequence",
        "asset_properties": [
            {"name": "SequenceLength", "exported_text": "0.800000"},
            {"name": "SamplingFrameRate", "exported_text": "(Numerator=30000,Denominator=1001)"},
            {"name": "bForceRootLock", "exported_text": "True"},
            {"name": "RootMotionRootLock", "exported_text": "Zero"},
            {
                "name": "AuthoredSyncMarkers",
                "exported_text": '((MarkerName="Left",Time=0.033333,TrackIndex=1,Guid=00000000),(MarkerName="Right",Time=0.433333,TrackIndex=1,Guid=00000001))',
            },
        ],
    }
    assert sequence_timing_contract(sequence_source) == {
        "duration": 0.8,
        "frames_per_second": 30000.0 / 1001.0,
        "root_lock": "Zero",
        "markers": [
            {"name": "Left", "time": 0.033333},
            {"name": "Right", "time": 0.433333},
        ],
    }
    profile_source = {
        "format": "ue_asset_source_v1",
        "asset_path": "/ALS/ALS/Character/SK_Test.SK_Test",
        "asset_objects": [
            {
                "object_path": "/ALS/ALS/Character/SK_Test.SK_Test:QuickFeet",
                "class_path": "/Script/Engine.BlendProfile",
                "properties": [
                    {
                        "name": "ProfileEntries",
                        "exported_text": '((BoneReference=(BoneName="calf_l"),BlendScale=5.000000),(BoneReference=(BoneName="calf_r"),BlendScale=5.000000))',
                    }
                ],
            }
        ],
    }
    profile_path = "/ALS/ALS/Character/SK_Test.SK_Test:QuickFeet"
    assert blend_profile_contract(profile_source, profile_path)["bone_scales"] == [
        {"bone": "calf_l", "scale": 5.0},
        {"bone": "calf_r", "scale": 5.0},
    ]
    with tempfile.TemporaryDirectory() as directory:
        content = Path(directory) / "Content"
        sequence_path = content / "ALS" / "Animations" / "A_Test.uasset"
        profile_path_on_disk = content / "ALS" / "Character" / "SK_Test.uasset"
        sequence_path.parent.mkdir(parents=True)
        profile_path_on_disk.parent.mkdir(parents=True)
        sequence_path.write_bytes(b"fixture")
        profile_path_on_disk.write_bytes(b"fixture")
        original_uasset = globals()["UAsset"]
        original_native = globals()["native_source_asset"]
        try:
            def unreadable_uasset(_):
                raise RuntimeError("cooked fixture")
            globals()["UAsset"] = unreadable_uasset
            globals()["native_source_asset"] = lambda source_path, source_reader=None: (
                sequence_source
                if source_path == sequence_source["asset_path"]
                else profile_source
            )
            assert source_sequence_timing(
                content, "/ALS", sequence_source["asset_path"]
            ) == sequence_timing_contract(sequence_source)
            assert source_blend_profile(content, "/ALS", profile_path) == blend_profile_contract(
                profile_source, profile_path
            )
        finally:
            globals()["UAsset"] = original_uasset
            globals()["native_source_asset"] = original_native
    curve_source = {
        "format": "ue_asset_source_v1",
        "asset_path": "/ALS/ALS/Data/AnimationInstance/Blending/CF_Test.CF_Test",
        "asset_class": "/Script/Engine.CurveFloat",
        "asset_properties": [
            {
                "name": "FloatCurve",
                "exported_text": "(Keys=((InterpMode=RCIM_Cubic),(InterpMode=RCIM_Cubic,TangentMode=RCTM_User,Time=0.400000,Value=0.400000,ArriveTangent=2.730308,LeaveTangent=2.730308),(Time=1.000000,Value=1.000000)),DefaultValue=340282346638528859811704183484516925440.000000,PreInfinityExtrap=RCCE_Constant,PostInfinityExtrap=RCCE_Constant)",
            }
        ],
    }
    curve = curve_float_contract(curve_source)
    assert curve["keys"] == [
        {
            "interpolation": "Cubic",
            "tangent_mode": "Auto",
            "tangent_weight_mode": "None",
            "t": 0.0,
            "v": 0.0,
            "arrive_tangent": 0.0,
            "arrive_weight": 0.0,
            "leave_tangent": 0.0,
            "leave_weight": 0.0,
        },
        {
            "interpolation": "Cubic",
            "tangent_mode": "User",
            "tangent_weight_mode": "None",
            "t": 0.4,
            "v": 0.4,
            "arrive_tangent": 2.730308,
            "arrive_weight": 0.0,
            "leave_tangent": 2.730308,
            "leave_weight": 0.0,
        },
        {
            "interpolation": "Linear",
            "tangent_mode": "Auto",
            "tangent_weight_mode": "None",
            "t": 1.0,
            "v": 1.0,
            "arrive_tangent": 0.0,
            "arrive_weight": 0.0,
            "leave_tangent": 0.0,
            "leave_weight": 0.0,
        },
    ]
    curve_source["asset_properties"][0]["exported_text"] = curve_source["asset_properties"][0][
        "exported_text"
    ].replace("PreInfinityExtrap=RCCE_Constant", "PreInfinityExtrap=RCCE_Linear")
    try:
        curve_float_contract(curve_source)
    except AssetResolutionError:
        pass
    else:
        raise AssertionError("unsupported CurveFloat extrapolation was accepted")
    return {
        "source_object_reference_parsed": "PASS",
        "native_source_cache_normalizes_the_reader_key": "PASS",
        "unique_animation_clip_resolved": "PASS",
        "blendspace_metadata_resolved": "PASS",
        "blendspace_samples_resolve_to_target_clips": "PASS",
        "ambiguous_target_rejected": "PASS",
        "source_animation_path_resolves_inside_mount": "PASS",
        "sequence_nodes_resolve_from_source_graphs": "PASS",
        "blendspace_evaluator_schema_is_validated": "PASS",
        "one_dimensional_dynamic_blendspace_evaluator_supported": "PASS",
        "blendspace_player_source_asset_and_timing_contract_resolved": "PASS",
        "empty_skeleton_blend_profile_preserved": "PASS",
        "unsupported_blend_profile_rejected": "PASS",
        "cooked_sequence_and_blend_profile_native_fallback_preserved": "PASS",
        "legacy_sequence_native_timing_and_curves_preserved": "PASS",
        "native_sequence_curve_fields_and_read_failures_preserved": "PASS",
        "source_rich_curve_keys_preserved": "PASS",
        "unsupported_curve_float_extrapolation_rejected": "PASS",
    }


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
