import copy
import gzip
import hashlib
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / "tools/import"))
from capture_als_sequence_compression import mesh_inventory, validate_capture


def read_native(name):
    content = gzip.decompress((HERE / f"{name}.native.json.gz").read_bytes())
    digest, filename = (HERE / f"{name}.native.sha256").read_text().split()
    assert filename == f"{name}.native.json"
    assert hashlib.sha256(content).hexdigest() == digest
    return json.loads(content)


def reject(capture, mesh, sequences, phrase):
    try:
        validate_capture(capture, mesh, sequences)
    except ValueError as error:
        assert phrase in str(error), str(error)
    else:
        raise AssertionError(phrase)


def frame_pairs(raw, prepared):
    assert raw["asset_path"] == prepared["asset_path"]
    raw = raw["sequence_pose_probe"]
    prepared = prepared["sequence_pose_probe"]
    assert raw["required_bones_mesh"] == prepared["required_bones_mesh"]
    assert raw["duration"] == prepared["duration"]
    assert len(raw["frames"]) == len(prepared["frames"])
    for source, target in zip(raw["frames"], prepared["frames"]):
        assert source["pose_evaluation_mode"] == "raw_model"
        assert target["pose_evaluation_mode"] == "compressed"
        assert source["time"] == target["time"]
        assert [bone["name"] for bone in source["bones"]] == [bone["name"] for bone in target["bones"]]
        yield source, target


def rotation_difference(raw, prepared):
    differences = []
    for source, target in frame_pairs(raw, prepared):
        rotations = [
            next(bone for bone in frame["bones"] if bone["name"] == "VB foot_r")["local_transform"]["rotation"]
            for frame in (source, target)
        ]
        sign = -1 if sum(rotations[0][axis] * rotations[1][axis] for axis in "xyzw") < 0 else 1
        differences.append(max(abs(rotations[0][axis] - sign * rotations[1][axis]) for axis in "xyzw"))
    assert differences[0] > 0.001
    return differences


def bow_difference(raw, prepared):
    differences = []
    for source, target in frame_pairs(raw, prepared):
        assert source["curves"] == target["curves"] == {}
        translations = [
            next(bone for bone in frame["bones"] if bone["name"] == "spine_down_02")["local_transform"]["translation"]
            for frame in (source, target)
        ]
        differences.append(max(abs(translations[0][axis] - translations[1][axis]) for axis in "xyz"))
    assert min(differences) > 0.001
    return differences


def replay():
    reports = {}
    inventory = mesh_inventory(ROOT / "assets")
    prepared = read_native("prepared")
    differences = rotation_difference(read_native("raw"), prepared)
    mixed = json.loads((HERE / "mixed-mesh.request.json").read_bytes())["assets"]
    failure = json.loads((HERE / "mixed-mesh.failure.reader.report.json").read_bytes())
    assert failure["process_exit_code"] == 7 and not failure["success"]
    assert set(mixed) == {asset for sequences in inventory.values() for asset in sequences}
    failure_reason = json.loads((HERE / "mixed-mesh.failure.json").read_bytes())
    assert failure_reason["captured"] == len(inventory[prepared["sequence_pose_probe"]["required_bones_mesh"]])
    for name in ("character", "bow"):
        capture = read_native(name)
        mesh = capture["mesh"]
        assert json.loads((HERE / f"{name}.request.json").read_bytes())["assets"] == list(inventory[mesh])
        reports[name] = validate_capture(capture, mesh, inventory[mesh])
        assert reports[name]["pose_modes"] == {"compressed": len(inventory[mesh])}
    character = read_native("character")
    mesh = character["mesh"]
    sequences = inventory[mesh]
    target = next(row for row in character["sequences"] if row["asset"] == prepared["asset_path"])
    assert target["duration"] == prepared["sequence_pose_probe"]["duration"]
    single = read_native("single-stream")
    assert single["asset_path"] == target["asset"]
    assert single["sequence_pose_probe"]["compression"] == target["compression"]
    bone_index = next(index for index, bone in enumerate(character["bones"]) if bone["name"] == "VB foot_r")
    assert bone_index in target["compression"]["track_to_skeleton"]
    invalid = copy.deepcopy(character)
    row = next(row for row in invalid["sequences"] if row["asset"] == target["asset"])
    row["pose_mode"] = "raw_model"
    reject(invalid, mesh, sequences, "Invalid Sequence pose mode")
    row["pose_mode"] = "compressed"
    row["compression"]["track_to_skeleton"].remove(bone_index)
    row["compression"]["track_to_skeleton"].append(len(invalid["bones"]))
    reject(invalid, mesh, sequences, "Invalid track-to-skeleton mapping")
    row["compression"]["track_to_skeleton"] = target["compression"]["track_to_skeleton"]
    with_curves = next(row for row in invalid["sequences"] if len(row["compression"]["indexed_curve_names"]) > 1)
    with_curves["compression"]["indexed_curve_names"][0]["sorted_index"] = -1
    reject(invalid, mesh, sequences, "Invalid platform curve index")
    return {
        "result": "PASS_PREPARED_PLATFORM_INVENTORY",
        "captures": reports,
        "witness": prepared["asset_path"],
        "virtual_foot_rotation_delta": differences,
        "bow_spine_down_02_translation_delta_cm": bow_difference(read_native("bow-raw"), read_native("bow-prepared")),
        "single_stream_matches_batch": True,
        "mixed_mesh_native_rejected": True,
        "static_rejections": ["raw_mode_with_compression", "bad_skeleton_index", "bad_curve_permutation"],
    }


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
