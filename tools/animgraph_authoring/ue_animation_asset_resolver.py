import json
import re
import sys
import tempfile
from pathlib import Path

IMPORT_ROOT = Path(__file__).resolve().parents[1] / "import"
sys.path.insert(0, str(IMPORT_ROOT))

from uasset_base import UAsset
from uasset_reader import PropReader
from ue_exported_text import ExportedTextError, field as struct_field, parse_bool, parse_number, parse_struct


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


def source_sequence_timing(content_root, mount, source_path):
    path = source_uasset_path(content_root, mount, source_path)
    try:
        asset = UAsset(str(path))
        reader = PropReader(asset)
    except Exception as error:
        raise AssetResolutionError("source animation asset cannot be read") from error
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
        raise AssetResolutionError("source animation sequence properties are unreadable")
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
    return {"duration": float(duration), "markers": markers}


def source_blend_profile(content_root, mount, source_path):
    package_object, separator, profile_name = source_path.rpartition(":")
    if not separator or not package_object or not profile_name:
        raise AssetResolutionError("source blend profile path is invalid")
    path = source_uasset_path(content_root, mount, package_object)
    try:
        asset = UAsset(str(path))
        reader = PropReader(asset)
    except Exception as error:
        raise AssetResolutionError("source blend profile cannot be read") from error
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


def source_blend_profile_wrapper(content_root, mount, value):
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
    return source_blend_profile(content_root, mount, source_object_path(reference))


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


def resolve_blendspace(asset_root, source_path):
    return resolve_blendspace_contract(asset_root, source_path)["target_asset"]


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
        "target_asset": relative_asset_path(asset_root, path),
        "axes": normalized_axes,
        "samples": normalized_samples,
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
    if input_names != ["X", "Y"]:
        raise AssetResolutionError("blendspace evaluator input schema is unsupported")
    axis_names = [axis["name"] for axis in blendspace["axes"]]
    if len(axis_names) < 2 or axis_names[2:] != ["None"] * len(axis_names[2:]):
        raise AssetResolutionError("blendspace evaluator dimensionality is unsupported")
    for index, pin in enumerate(input_pins):
        if pin.get("friendly_name") != axis_names[index]:
            raise AssetResolutionError("blendspace evaluator axis pin does not match metadata")
        source_pin_number(pin.get("default_value"), f"blendspace evaluator input {pin['name']} default")
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
            for index, pin in enumerate(input_pins)
        ],
        "normalized_time": normalized_time,
        "teleport_to_normalized_time": teleport,
        "loop": loop,
        "reset_play_time_when_asset_changes": reset_play_time,
        "configured_play_rate": configured_play_rate,
    }


def resolve_graph_animation_assets(asset_root, source, source_content_root=None, source_mount=None):
    rows = []
    timings = {}
    for graph in source.get("graphs", []):
        for node in graph.get("nodes", []):
            class_path = node.get("class_path")
            if class_path in {
                "/Script/AnimGraph.AnimGraphNode_SequencePlayer",
                "/Script/AnimGraph.AnimGraphNode_SequenceEvaluator",
            }:
                source_path = named_object_reference(node_property(node, "Node"), "Sequence")
                target = resolve_animation_clip(asset_root, source_path)
                category = "sequence"
            elif class_path == "/Script/AnimGraph.AnimGraphNode_BlendSpaceEvaluator":
                source_path = named_object_reference(node_property(node, "Node"), "BlendSpace")
                blendspace = resolve_blendspace_contract(asset_root, source_path)
                evaluator = blendspace_evaluator_contract(node, blendspace)
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
            if class_path == "/Script/AnimGraph.AnimGraphNode_BlendSpaceEvaluator":
                row["blendspace"] = blendspace
                row["evaluator"] = evaluator
            if category == "sequence" and source_content_root is not None:
                timing = timings.get(source_path)
                if timing is None:
                    timing = source_sequence_timing(source_content_root, source_mount, source_path)
                    timings[source_path] = timing
                row["source_timing"] = timing
            rows.append(row)
    return rows


def run_self_test():
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
        assert resolve_blendspace(root, "/ALS/Animations/BS_Test.BS_Test") == "animations/blendspaces/BS_Test.json"
        contract = resolve_blendspace_contract(root, "/ALS/Animations/BS_Test.BS_Test")
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
    return {
        "source_object_reference_parsed": "PASS",
        "unique_animation_clip_resolved": "PASS",
        "blendspace_metadata_resolved": "PASS",
        "blendspace_samples_resolve_to_target_clips": "PASS",
        "ambiguous_target_rejected": "PASS",
        "source_animation_path_resolves_inside_mount": "PASS",
        "sequence_nodes_resolve_from_source_graphs": "PASS",
        "blendspace_evaluator_schema_is_validated": "PASS",
        "empty_skeleton_blend_profile_preserved": "PASS",
        "unsupported_blend_profile_rejected": "PASS",
    }


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test:
        raise SystemExit("--self-test is required")
    print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
