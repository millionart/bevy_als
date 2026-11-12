import bisect
import json
import re
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d224c"))
from replay_curve_consumers import ASSETS, BLEND_SPACE, CLIP, CLIP_NODE, DIFFERENCE_NODES, EDGE, NODE, PACK, POINT
from replay_prepared_acl import clip_data, f32, read_capture, sample, selection
from audit_animation_transform_conversion import accessor_values, load_gltf
from compile_als_prepared_acl import runtime_bone_paths

CHANNELS = (("rotation", "xyzw"), ("translation", "xyz"), ("scale", "xyz"))


def components(transform):
    return tuple(tuple(transform[field][axis] for axis in axes) for field, axes in CHANNELS)


def evaluation_time(time, frame_rate):
    numerator, denominator = frame_rate
    frame_time = f32(time) * numerator / denominator
    frame = int(frame_time)
    subframe = f32(frame_time - frame)
    carry = int(subframe)
    limit = f32(0.99999994)
    fraction = min(max(f32(f32(min(subframe - carry, limit) + 0.5) - 0.5), 0.0), limit)
    return f32(((frame + carry) * denominator + fraction * denominator) / numerator)


def pose_at(sequence, metadata, keys, time, convert_time=True):
    clock = sequence["bone"]
    time = min(f32(time), sequence["duration"])
    if convert_time:
        time = evaluation_time(time, sequence["frame_rate"])
    frames = selection(clock, time, sequence["step"])
    additive = sequence["additive_type"] != 0
    identity_delta = ((0.0, 0.0, 0.0, 1.0), (0.0,) * 3, (0.0,) * 3)
    pose = [identity_delta if additive else components(bone["mesh_pose"]) for bone in metadata["bones"]]
    for index, *tracks in clock["tracks"]:
        reference = identity_delta if additive else components(metadata["bones"][index]["skeleton_pose"])
        output = list(pose[index])
        for channel, (offset, mode) in enumerate(tracks):
            if channel == 2 and not clock["has_scale"]:
                continue
            output[channel] = sample(keys, offset, mode, len(CHANNELS[channel][1]), frames, "bone",
                                     index == 0 and sequence["single_root"]) if mode else tuple(
                                         f32(value) for value in reference[channel])
        pose[index] = tuple(output)
    return pose


def project(value):
    (x, y, z, w), (tx, ty, tz), (sx, sy, sz) = value
    return (tuple(f32(v) for v in (-x, -z, -y, w)),
            tuple(f32(v * 0.01) for v in (tx, tz, ty)),
            tuple(f32(v) for v in (sx, sz, sy)))


def read_pose_probe(name):
    external = {"crouch": ("d224a", "prepared"), "run": ("d224b", "run-prepared"),
                "transition": ("d224c", "transition-prepared"), "turn": ("d224c", "turn-prepared")}
    directory, stem = external.get(name, (HERE.name, name))
    return read_capture(HERE.parent / directory / f"{stem}.native.json.gz")


def verify_poses(metadata, keys):
    sequences = {row["asset"]: row for row in metadata["sequences"]}
    control = read_capture(HERE.parent / "d224a/single-stream.native.json.gz")
    retarget_names = {bone["name"] for bone in control["sequence_pose_probe"]["frames"][0]["bones"]
                      if bone["retarget_mode"] == 3}
    assert retarget_names == {"ik_foot_root", "ik_foot_l", "ik_foot_r"}
    summary = {}
    changes = Counter()
    for name in ("crouch", "run", "stand", "rotate", "lean", "look", "transition", "turn", "rifle", "getup"):
        native = read_pose_probe(name)
        probe = native["sequence_pose_probe"]
        clip = sequences[native["asset_path"]]
        assert probe["required_bones_mesh"] == "/ALS/ALS/Character/SKM_Als.SKM_Als"
        assert probe["duration"] == clip["duration"]
        flags = {row["name"]: row["exported_text"] for row in native["asset_properties"]}
        assert flags["PlatformTargetFrameRate"] == "(Default=(Numerator=30,Denominator=1))"
        assert clip["frame_rate"] == [30, 1]
        source_lock = flags["RootMotionRootLock"] if flags["bForceRootLock"] == "True" else None
        assert clip["forced_root_lock"] == source_lock
        locked = clip["forced_root_lock"] is not None
        if locked:
            assert clip["forced_root_lock"] == "RefPose" and clip["additive_type"] == 0
        mapped = {track[0] for track in clip["bone"]["tracks"]}
        counts = Counter()
        for frame in probe["frames"]:
            assert frame["pose_evaluation_mode"] == "compressed"
            actual = pose_at(clip, metadata, keys, frame["time"])
            without_frame_roundtrip = pose_at(clip, metadata, keys, frame["time"], convert_time=False)
            if locked:
                root_reference = components(metadata["bones"][0]["mesh_pose"])
                changes["ignore_root_lock"] += actual[0] != root_reference
                actual[0] = root_reference
            for index, (value, native_bone, refs) in enumerate(zip(actual, frame["bones"], metadata["bones"])):
                assert native_bone["name"] == refs["name"]
                expected = components(native_bone["local_transform"])
                changes["skip_evaluation_time_roundtrip"] += value == expected and without_frame_roundtrip[index] != value and not (locked and index == 0)
                if clip["additive_type"]:
                    assert expected[2] == (0.0, 0.0, 0.0)
                    changes["additive_unit_scale"] += expected[2] != (1.0, 1.0, 1.0)
                if index not in mapped:
                    assert value == expected
                    changes["skeleton_reference_for_unmapped"] += components(refs["skeleton_pose"]) != expected
                if not clip["bone"]["has_scale"] and not clip["additive_type"]:
                    changes["round_unwritten_scale"] += tuple(f32(v) for v in value[2]) != value[2]
                if value == expected:
                    counts["exact_bones"] += 1
                else:
                    assert native_bone["name"] in retarget_names and clip["additive_type"] == 0, (
                        name, frame["time"], native_bone["name"], value, expected)
                    counts["animation_relative_bones"] += 1
                    counts["animation_relative_after_projection"] += project(value) != project(expected)
        summary[name] = {"frames": len(probe["frames"]), "additive_type": clip["additive_type"],
                         "root_lock": locked, **counts}
    assert all(changes.values()) and len(changes) == 5, changes
    return summary, dict(changes)


def verify_root_tracks(metadata, keys):
    sequences = {row["asset"]: row for row in metadata["sequences"]}
    result = {}
    for name in ("roll-root", "mantle-root"):
        native = read_pose_probe(name)
        clip = sequences[native["asset_path"]]
        document, data = load_gltf(ASSETS / clip["gltf"])
        animation = next(item for item in document["animations"] if item["name"] == clip["animation"])
        channel = next(channel for channel in animation["channels"] if channel["target"]["path"] == "translation"
                       and document["nodes"][channel["target"]["node"]]["name"] == "root")
        sampler = animation["samplers"][channel["sampler"]]
        clock = [time for time, in accessor_values(document, data, sampler["input"])]
        values = accessor_values(document, data, sampler["output"])
        differences = []
        frames = native["sequence_pose_probe"]["frames"]
        for frame in frames:
            time = f32(frame["time"])
            actual = pose_at(clip, metadata, keys, time)[0]
            expected = components(frame["bones"][0]["local_transform"])
            assert actual == expected, (name, time, actual, expected)
            index = min(bisect.bisect_right(clock, time) - 1, len(clock) - 2)
            alpha = (time - clock[index]) / (clock[index + 1] - clock[index])
            raw = [f32(first + (last - first) * alpha) for first, last in zip(values[index], values[index + 1])]
            differences.append(max(abs(a - b) for a, b in zip(project(actual)[1], raw)))
        assert max(differences) > 1e-5
        result[name] = {"native_exact": len(frames), "raw_gltf_translation_max_m": max(differences)}
    return result


def verify_bindings(metadata):
    files = set()
    for manifest in ASSETS.rglob("installation.manifest.json"):
        files.update(path for path in json.loads(manifest.read_bytes())["files"] if path.endswith(".animgraph.ron"))
    clips = {row["clip"]: row for row in metadata["sequences"]}
    skel = "ALS/Character/SKM_Als.skn.ron"
    assert [tuple(path) for path in metadata["bone_paths"]] == runtime_bone_paths(ASSETS, skel, metadata["bones"])
    assert metadata["bone_paths"][0] == ["GltfNode0", "root"]
    for row in clips.values():
        descriptor = (ASSETS / row["clip"]).read_text()
        assert f'skeleton: "{skel}"' in descriptor
        gltf = json.loads((ASSETS / row["gltf"]).read_bytes())
        animation = next(a for a in gltf["animations"] if a["name"] == row["animation"])
        assert all(ch["target"]["path"] in {"translation", "rotation", "scale"} for ch in animation["channels"])
    count = Counter()
    for filename in sorted(files):
        content = (ASSETS / filename).read_text()
        nodes = {identifier: (kind, fields) for identifier, kind, fields in NODE.findall(content)}
        edges = {(identifier, pin): source for identifier, pin, source, _ in EDGE.findall(content)}
        used = set()
        for identifier, (kind, fields) in nodes.items():
            if kind != BLEND_SPACE:
                continue
            for point in POINT.finditer(fields):
                leaf = edges[identifier, f"pose_{point[1]}"]
                leaf_kind, leaf_fields = nodes[leaf]
                if leaf_kind in DIFFERENCE_NODES:
                    used.add(leaf)
                    target, fields = nodes[edges[leaf, "target_pose"]]
                    mode = 1 if leaf_kind == "als_ext::ReferenceLocalDifferenceNode" else 2
                else:
                    target, fields, mode = leaf_kind, leaf_fields, 0
                assert target == CLIP_NODE and clips[CLIP.search(fields)[1]]["additive_type"] == mode
                assert "override_duration: None" in fields and "override_interpolation: None" in fields
                assert "root_motion_mode" not in fields or "root_motion_mode: Disabled" in fields
                count[["absolute_blend_sample", "local_additive_blend_sample", "mesh_additive_blend_sample"][mode]] += 1
        assert used == {identifier for identifier, (kind, _) in nodes.items() if kind in DIFFERENCE_NODES}
    return {"bound_skeleton_bones": len(metadata["bone_paths"]), **count}


def replay():
    metadata, keys = clip_data(ROOT / PACK)
    original = read_capture(HERE.parent / "d224a/character.native.json.gz")
    fresh = read_pose_probe("character")
    assert len(fresh["sequences"]) == len(metadata["sequences"])
    locks = Counter()
    for source, compiled in zip(fresh["sequences"], metadata["sequences"]):
        assert source["asset"] == compiled["asset"]
        mode = source.pop("forced_root_lock")
        assert compiled["forced_root_lock"] == mode
        locks[mode or "unlocked"] += 1
    assert original == fresh and locks == {"unlocked": 111, "RefPose": 16}
    poses, ablations = verify_poses(metadata, keys)
    return {"result": "PASS_PREPARED_ACL_POSE_CONSUMERS", "pose_samples": poses,
            "root_tracks": verify_root_tracks(metadata, keys), "bindings": verify_bindings(metadata),
            "root_lock_modes": dict(locks), "source_streams_preserved": True,
            "removed_contract_mismatches": ablations}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
