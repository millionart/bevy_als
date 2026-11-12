import argparse
import json
import struct
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d224d"))
sys.path.insert(0, str(HERE.parent / "d228"))
from replay_pose_consumers import PACK, clip_data, components, pose_at, project, read_capture, read_pose_probe
from replay_source_frame_time import control_at, source_rotations, within
from compile_als_event_tracks import f32


def multiply(a, b):
    x, y, z, w = a
    bx, by, bz, bw = b
    return (w * bx + x * bw + y * bz - z * by,
            w * by - x * bz + y * bw + z * bx,
            w * bz + x * by - y * bx + z * bw,
            w * bw - x * bx - y * by - z * bz)


def relative(value, bone, normalize):
    rotation, translation, scale = value
    authored_q, authored_t, authored_s = components(bone["skeleton_pose"])
    reference_q, reference_t, reference_s = components(bone["mesh_pose"])
    x, y, z, w = authored_q
    q = normalize(multiply(multiply(rotation, (-x, -y, -z, w)), reference_q))
    t = tuple(a + (b - c) for a, b, c in zip(translation, reference_t, authored_t))
    s = tuple(a * (b * (1.0 / c if abs(c) > f32(1e-8) else 0.0))
              for a, b, c in zip(scale, reference_s, authored_s))
    return q, t, s


def verify_inventory(metadata):
    for name in ("character", "bow"):
        fresh = read_capture(HERE / f"{name}.native.json.gz")
        previous = read_capture(HERE.parent / "d226" / f"{name}.native.json.gz")
        modes = [bone.pop("retarget_mode") for bone in fresh["bones"]]
        expected = [{"bone": index, "authored_pose": fresh["bones"][index]["skeleton_pose"]}
                    for index, mode in enumerate(modes) if mode == 3]
        assert all(row.pop("animation_relative") == expected for row in fresh["sequences"])
        assert fresh == previous
        if name == "character":
            relative_indices = [item["bone"] for item in expected]
            assert relative_indices == metadata["animation_relative_bones"]
        else:
            assert not expected
    bones = metadata["bones"]
    aliases = json.loads((ROOT / "assets/ALS/Data/AnimationInstance/als_raw_virtual_bones.json").read_bytes())
    by_name = {bone["name"]: index for index, bone in enumerate(bones)}
    for binding in aliases["virtual_bones"]:
        for name in (binding["source_bone"], binding["target_bone"]):
            index = by_name[name]
            while index >= 0:
                assert index not in relative_indices
                index = bones[index]["parent_index"]
    return relative_indices


def verify_prepared(metadata, keys, relative_indices, normalize):
    by_asset = {row["asset"]: row for row in metadata["sequences"]}
    counts = Counter()
    witness = None
    reports = [read_pose_probe(name) for name in ("crouch", "run", "stand", "rotate", "lean", "look", "transition", "turn", "rifle", "getup")]
    reports.append(read_capture(HERE / "prepared.native.json.gz"))
    for native in reports:
        sequence = by_asset[native["asset_path"]]
        tracked = {row[0] for row in sequence["bone"]["tracks"]}
        for frame in native["sequence_pose_probe"]["frames"]:
            assert frame["pose_evaluation_mode"] == "compressed"
            values = pose_at(sequence, metadata, keys, frame["time"])
            if sequence["forced_root_lock"] == "RefPose":
                values[0] = components(metadata["bones"][0]["mesh_pose"])
            for index, (value, bone) in enumerate(zip(values, frame["bones"])):
                expected = components(bone["local_transform"])
                if not sequence["additive_type"] and index in relative_indices and index in tracked:
                    before = value
                    value = relative(value, metadata["bones"][index], normalize)
                    projected_change = project(before) != project(expected)
                    counts["prepared_without_tail_f32"] += projected_change
                    counts["prepared_tail_f64"] += before != value
                    if witness is None and projected_change:
                        witness = {"asset": native["asset_path"], "time": frame["time"], "bone": bone["name"],
                                   "before": project(before)[0], "after": project(value)[0]}
                assert value == expected, (native["asset_path"], frame["time"], bone["name"], value, expected)
                counts["prepared_exact_bones"] += 1
                counts["prepared_additive_bones"] += bool(sequence["additive_type"])
    return counts, witness


def verify_raw(metadata, relative_indices, normalize, nlerp):
    data = (ROOT / "assets/ALSRuntime/Animations/als_raw_fk_controls.bin").read_bytes()
    size, = struct.unpack_from("<I", data)
    sequences = json.loads(data[4:4 + size])
    keys = data[4 + size:]
    native = read_capture(HERE / "raw.native.json.gz")
    name = native["asset_path"].rsplit(".", 1)[-1] + "_0"
    sequence = next(row for row in sequences if row["animation"] == name)
    counts = Counter()
    witness = None
    for frame in native["sequence_pose_probe"]["frames"]:
        stages = {row["name"]: row for row in frame["source_stages"]["bones"]}
        for index, span in sequence["tracks"]:
            bone = metadata["bones"][index]
            candidate = control_at(sequence, keys, span, frame["time"], normalize, nlerp)
            retained = within(candidate, components(bone["skeleton_pose"]))
            value = components(bone["mesh_pose"]) if retained else candidate
            value = (normalize(value[0]), value[1], value[2])
            expected = components(frame["bones"][index]["local_transform"])
            if index in relative_indices and not retained:
                before = value
                value = relative(value, bone, normalize)
                native_before = components(stages[bone["name"]]["normalized_bone"])
                native_after = components(stages[bone["name"]]["retargeted_bone"])
                assert relative(native_before, bone, normalize) == native_after
                assert project(value) == project(native_after)
                counts["raw_tail_f64"] += native_before != native_after
                counts["raw_without_tail_f32"] += project(before) != project(expected)
                if frame["time"] == f32(0.4568613768) and bone["name"] == "ik_foot_r":
                    witness = {"time": frame["time"], "before": native_before[0], "after": native_after[0]}
            assert project(value) == project(expected), (frame["time"], bone["name"], value, expected)
            counts["raw_projected_bones"] += 1
    return counts, witness


def replay(work):
    metadata, keys = clip_data(ROOT / PACK)
    relative_indices = verify_inventory(metadata)
    normalize, nlerp = source_rotations(work)
    prepared, prepared_witness = verify_prepared(metadata, keys, relative_indices, normalize)
    raw, witness = verify_raw(metadata, relative_indices, normalize, nlerp)
    assert prepared["prepared_without_tail_f32"] > 0 and raw["raw_tail_f64"] > 0
    assert witness and witness["before"] != witness["after"]
    return {"result": "PASS_SEQUENCE_ANIMATION_RELATIVE", "relative_bones": [
        metadata["bones"][index]["name"] for index in relative_indices],
        "shared_authored_reference_sequences": len(metadata["sequences"]),
        "virtual_bone_dependency_disjoint": True,
        "prepared": dict(prepared), "raw": dict(raw), "prepared_witness": prepared_witness,
        "raw_witness": witness}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d230")
    print(json.dumps(replay(parser.parse_args().work), indent=2))
