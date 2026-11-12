import json
import re
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d224b"))
from replay_prepared_acl import clip_data, f32, read_capture, sample, selection

ASSETS = ROOT / "assets"
PACK = "assets/ALSRuntime/Animations/als_prepared_acl.bin"
NODE = re.compile(r'\(id: "([^"]+)", name: "(?:\\.|[^"\\])*", inner: \{ "([^"]+)": (\(.*\)) \}, should_debug: (?:true|false)\),')
EDGE = re.compile(r'NodeData\("([^"]+)", "([^"]+)"\): NodeData\("([^"]+)", "([^"]+)"\)')
CLIP = re.compile(r'\bclip: "([^"]+)"')
POINT = re.compile(r'\(id: "([^"]+)", point: ')
SEQUENCE_NODES = {"als_ext::SourceSequencePlayerNode", "als_ext::SourceSequenceEvaluatorNode"}
BLEND_SPACE = "als_ext::NormalizedBlendSpaceNode"
CLIP_NODE = "bevy_animation_graph::builtin_nodes::ClipNode"
DIFFERENCE_NODES = {"als_ext::ReferenceLocalDifferenceNode", "als_ext::ReferenceMeshDifferenceNode"}


def graph_bindings(by_clip):
    files = set()
    for manifest in ASSETS.rglob("installation.manifest.json"):
        files.update(path for path in json.loads(manifest.read_bytes())["files"] if path.endswith(".animgraph.ron"))
    counts = Counter()
    bow = "ALS/OverlayObjects/Bow/A_Als_Bow_Draw.anim.ron"
    for filename in sorted(files):
        content = (ASSETS / filename).read_text()
        nodes = {identifier: (kind, fields) for identifier, kind, fields in NODE.findall(content)}
        edges = {(identifier, pin): source for identifier, pin, source, _ in EDGE.findall(content)}
        for node_kind in SEQUENCE_NODES | {BLEND_SPACE}:
            assert content.count(f'"{node_kind}"') == sum(kind == node_kind for kind, _ in nodes.values()), filename
        for identifier, (kind, fields) in nodes.items():
            if kind in SEQUENCE_NODES:
                clip = CLIP.search(fields)[1]
                assert clip == bow or clip in by_clip, (filename, clip)
                counts["bow_evaluator_empty_curves" if clip == bow else "bound_sequence_nodes"] += 1
            if kind != BLEND_SPACE:
                continue
            points = list(POINT.finditer(fields))
            assert points, filename
            for index, point in enumerate(points):
                leaf = edges[(identifier, f"pose_{point[1]}")]
                leaf_kind, leaf_fields = nodes[leaf]
                if leaf_kind == CLIP_NODE:
                    assert CLIP.search(leaf_fields)[1] in by_clip, filename
                    counts["bound_blend_space_samples"] += 1
                    continue
                assert leaf_kind in DIFFERENCE_NODES, (filename, leaf_kind)
                target_kind, target_fields = nodes[edges[(leaf, "target_pose")]]
                assert target_kind == CLIP_NODE
                target = by_clip[CLIP.search(target_fields)[1]]
                stop = points[index + 1].start() if index + 1 < len(points) else len(fields)
                assert target["curve"] is None and "curves: ({})" in fields[point.start():stop], filename
                counts["wrapped_samples_with_empty_curves"] += 1
    return {"graphs": len(files), **counts}


def sample_curves(clip, keys, time, scales):
    clock = clip["curve"]
    if clock is None:
        return {}
    frames = selection(clock, f32(time))
    return {
        name: f32(sample(keys, offset, mode, 1, frames, "curve", False)[0] * scales.get(name, 1.0))
        for name, (offset, mode) in clock["tracks"]
    }


def native_check(by_asset, keys, scales):
    counts = Counter()
    lock = "FootLeftLock"
    for name in ("run-prepared", "transition-prepared", "turn-prepared"):
        probe_directory = HERE.parent / "d224b" if name.startswith("run-") else HERE
        native = read_capture(probe_directory / f"{name}.native.json.gz")
        clip = by_asset[native["asset_path"]]
        for frame in native["sequence_pose_probe"]["frames"]:
            assert frame["pose_evaluation_mode"] == "compressed"
            expected = {key: f32(value * scales.get(key, 1.0)) for key, value in frame["curves"].items()}
            actual = sample_curves(clip, keys, frame["time"], scales)
            assert actual == expected, (name, frame["time"], actual, expected)
            counts[name] += len(expected)
            if name == "turn-prepared":
                unprojected = sample_curves(clip, keys, frame["time"], {})
                counts["missing_yaw_projection"] += actual != unprojected
            if name == "transition-prepared" and frame["time"] == f32(0.3):
                prepared_lock = actual[lock]

    raw = read_capture(HERE / "transition-raw.native.json.gz")
    raw_frame = next(frame for frame in raw["sequence_pose_probe"]["frames"] if frame["time"] == f32(0.3))
    raw_lock = raw_frame["curves"][lock]
    assert raw_frame["pose_evaluation_mode"] == "raw_model"
    assert prepared_lock == -2**-23 and raw_lock != prepared_lock
    raw_slot = f32(1.0 + f32(raw_lock * f32(0.3)))
    prepared_slot = f32(1.0 + f32(prepared_lock * f32(0.3)))
    assert raw_slot == 1.0 and prepared_slot == f32(0.99999994)
    assert counts["missing_yaw_projection"] == 2
    return {"native_values": dict(counts), "transition_time": f32(0.3), "raw_lock": raw_lock,
            "prepared_lock": prepared_lock, "slot_raw_control": raw_slot, "slot_prepared_control": prepared_slot}


def replay():
    metadata, keys = clip_data(ROOT / PACK)
    old = subprocess.check_output(["git", "show", f"1b5eee25:{PACK}"], cwd=ROOT)
    size, = struct.unpack_from("<I", old)
    scales = metadata.pop("curve_value_scales")
    assert scales == {"RotationYawSpeed": -1.0}
    assert metadata == json.loads(old[4:4 + size]) and keys == old[4 + size:]
    sequences = metadata["sequences"]
    for clip in sequences:
        descriptor = (ASSETS / clip["clip"]).read_text()
        assert re.search(r'\bpath: "([^"]+)"', descriptor)[1] == clip["gltf"]
        assert re.search(r'\banimation_name: "([^"]+)"', descriptor)[1] == clip["animation"]
    bindings = graph_bindings({clip["clip"]: clip for clip in sequences})
    native = native_check({clip["asset"]: clip for clip in sequences}, keys, scales)
    return {"result": "PASS_PREPARED_CURVE_CONSUMERS", "bindings": bindings,
            "curve_projection": scales, "source_keys_preserved": True, **native}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
