import argparse
import base64
import ctypes
import gzip
import hashlib
import json
import math
import struct
import sys
import tempfile
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / "tools/import"))
from compile_als_prepared_acl import run_tool
from capture_als_sequence_compression import mesh_inventory, read_capture

F32 = struct.Struct("<f")
FMA = ctypes.CDLL(None).fmaf
FMA.argtypes = [ctypes.c_float] * 3
FMA.restype = ctypes.c_float


def f32(value):
    return F32.unpack(F32.pack(value))[0]


def clip_data(path):
    data = path.read_bytes()
    length, = struct.unpack_from("<I", data)
    return json.loads(data[4:4 + length]), data[4 + length:]


def selection(clock, time, step=False):
    time = max(0.0, min(time, clock["duration"]))
    index = f32(time * clock["sample_rate"])
    count = clock["sample_count"]
    first = int(index)
    if clock["wrap"] and first >= count:
        first = second = 0
        index = 0.0
    else:
        second = (first + 1) % count if clock["wrap"] else min(first + 1, count - 1)
    alpha = 0.0 if step else f32(index - first)
    return first, second, alpha


def bone_dot(a, b, single):
    if single:
        pairs = [f32(x * y) for x, y in zip(a, b)]
        return f32(f32(pairs[0] + pairs[2]) + f32(pairs[1] + pairs[3]))
    dot = f32(a[0] * b[0])
    for x, y in zip(a[1:], b[1:]):
        dot = FMA(x, y, dot)
    return dot


def normalize(value, single):
    inverse = f32(1.0 / f32(math.sqrt(bone_dot(value, value, single))))
    return tuple(f32(component * inverse) for component in value)


def rotation(a, b, alpha, single, normalize_keys=False):
    if normalize_keys:
        a, b = normalize(a, single), normalize(b, single)
    if math.copysign(1.0, bone_dot(a, b, single)) < 0:
        b = tuple(-value for value in b)
    values = [FMA(y, alpha, FMA(-x, alpha, x)) for x, y in zip(a, b)]
    return normalize(values, single)


def sample(keys, offset, mode, width, frames, kind, single, normalize_keys=False):
    first, second, alpha = frames
    if mode == 0:
        return (0.0,) * width
    a = struct.unpack_from(f"<{width}f", keys, 4 * (offset + (first * width if mode == 2 else 0)))
    if mode == 1:
        return a
    b = struct.unpack_from(f"<{width}f", keys, 4 * (offset + second * width))
    if width == 4:
        return rotation(a, b, alpha, single, normalize_keys)
    if kind == "curve":
        return (FMA(b[0], alpha, f32(a[0] - f32(a[0] * alpha))),)
    return tuple(FMA(y, alpha, FMA(-x, alpha, x)) for x, y in zip(a, b))


def watchdog_self_test():
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "watchdog.json"
        try:
            run_tool([sys.executable, "-c", "import time; time.sleep(10)"], path, "waiting for a fixture", 0.05)
        except RuntimeError as error:
            assert str(error) == "No progress while waiting for a fixture"
        else:
            raise AssertionError("Watchdog failed to terminate the fixture")
        status = json.loads(path.read_bytes())
        assert status["exit_reason"] == "no_progress_limit"
        assert status["last_progress"] == "process_started"
        assert status["current_condition"] == "waiting for a fixture"
        assert status["no_progress_seconds"] >= status["no_progress_limit_seconds"]


def native_readback(records, keys):
    prepared = read_capture(HERE / "run-prepared.native.json.gz")
    raw = read_capture(HERE / "run-raw.native.json.gz")
    assert prepared["asset_path"] == raw["asset_path"]
    clip = records[prepared["asset_path"]]
    curve_clock = clip["curve"]
    raw_probe = raw["sequence_pose_probe"]
    native_probe = prepared["sequence_pose_probe"]
    assert raw_probe["required_bones_mesh"] == native_probe["required_bones_mesh"]
    assert len(raw_probe["frames"]) == len(native_probe["frames"])
    curve_matches = 0
    curve_delta = []
    wrong_curve_names = 0
    character = read_capture(HERE.parent / "d224a/character.native.json.gz")
    sequence = next(row for row in character["sequences"] if row["asset"] == prepared["asset_path"])
    indexed = sequence["compression"]["indexed_curve_names"]
    for native, uncompressed in zip(native_probe["frames"], raw_probe["frames"]):
        assert native["pose_evaluation_mode"] == "compressed"
        assert uncompressed["pose_evaluation_mode"] == "raw_model"
        assert native["time"] == uncompressed["time"]
        frames = selection(curve_clock, f32(native["time"]))
        actual = {name: sample(keys, offset, mode, 1, frames, "curve", False)[0]
                  for name, (offset, mode) in curve_clock["tracks"]}
        assert actual == native["curves"], (native["time"], actual, native["curves"])
        permuted = {indexed[item["sorted_index"]]["name"]: actual[item["name"]] for item in indexed}
        wrong_curve_names += sum(permuted[name] != value for name, value in actual.items())
        curve_matches += len(actual)
        curve_delta.append(abs(actual["FootPlanted"] - uncompressed["curves"]["FootPlanted"]))
    assert max(curve_delta) > 0.0007
    assert wrong_curve_names > 0

    codec = read_capture(HERE.parent / "d224a/single-stream.native.json.gz")
    clip = records[codec["asset_path"]]
    bone_clock = clip["bone"]
    native = codec["sequence_pose_probe"]["frames"][0]
    frames = selection(bone_clock, native["time"], clip["step"])
    bone_matches = 0
    for track_index, (bone, *channels) in enumerate(bone_clock["tracks"]):
        native_bone = native["bones"][bone]["codec_pose"]
        for (offset, mode), field, axes in zip(channels, ("rotation", "translation", "scale"), ("xyzw", "xyz", "xyz")):
            if mode == 0:
                continue
            actual = sample(keys, offset, mode, len(axes), frames, "bone", track_index == 0 and clip["single_root"])
            expected = tuple(native_bone[field][axis] for axis in axes)
            assert actual == expected, (bone, field, actual, expected)
            bone_matches += len(axes)
    return {"curve_values_exact": curve_matches, "written_bone_components_exact": bone_matches,
            "foot_planted_delta_vs_raw": curve_delta, "sorted_index_as_stream_order_mismatches": wrong_curve_names}


def replay(asset, oracle):
    metadata, keys = clip_data(asset)
    assert metadata["format"] == "als_prepared_acl_v1"
    assert metadata["sdk_architecture"] == "arm64"
    assert metadata["source_length_to_runtime_scale"] == 0.01
    source = read_capture(HERE.parent / "d224a/character.native.json.gz")
    assert metadata["bones"] == source["bones"]
    assert metadata["platform"] == source["platform"]
    inventory = mesh_inventory(ROOT / "assets")[source["mesh"]]
    assert [row["asset"] for row in metadata["sequences"]] == list(inventory) == list(oracle)
    offsets = 0
    counts = Counter()
    sample_counts = Counter()
    mismatches = []
    negative = Counter()
    for row, source_row in zip(metadata["sequences"], source["sequences"]):
        source_asset = row["asset"]
        assert row["duration"] == source_row["duration"]
        assert row["step"] == source_row["step"]
        assert row["additive_type"] == source_row["additive_type"]
        assert row["clip"] == inventory[source_asset][0]
        for kind, widths in (("bone", (4, 3, 3)), ("curve", (1,))):
            clock = row[kind]
            if clock is None:
                assert kind == "curve" and not source_row["compression"]["indexed_curve_names"]
                counts["empty_curves"] += 1
                continue
            track_ids = [track[0] for track in clock["tracks"]]
            if kind == "bone":
                assert track_ids == source_row["compression"]["track_to_skeleton"]
                assert row["single_root"] == (track_ids[0] == 0)
                counts["bone_wrap"] += clock["wrap"]
                counts["no_scale_stream"] += not clock["has_scale"]
                counts["stripped_step_clips"] += clock["stripped_step"]
                if clock["stripped_step"]:
                    assert row["step"]
            else:
                names = source_row["compression"]["indexed_curve_names"]
                assert track_ids == [item["name"] for item in names]
            for track in clock["tracks"]:
                for (offset, mode), width in zip(track[1:], widths):
                    assert offset == offsets
                    count = (0, 1, clock["sample_count"])[mode]
                    offsets += width * count
                    counts[f"{kind}_mode_{mode}"] += 1
            probes = base64.b64decode(oracle[source_asset][kind], validate=True)
            for time, *expected in struct.iter_unpack(f"<{1 + len(track_ids) * sum(widths)}f", probes):
                frames = selection(clock, time, row["step"] and kind == "bone")
                last = clock["sample_count"] - 1
                sample_counts[f"{kind}_poses"] += 1
                cursor = 0
                for track_index, (track_id, *channels) in enumerate(clock["tracks"]):
                    single = kind == "bone" and track_index == 0 and row["single_root"]
                    for (offset, mode), width in zip(channels, widths):
                        actual = sample(keys, offset, mode, width, frames, kind, single)
                        want = tuple(expected[cursor:cursor + width])
                        if actual != want:
                            mismatches.append((source_asset, kind, track_id, time, mode, actual, want))
                        sample_counts[f"{kind}_components"] += width
                        cursor += width
                        if width == 4 and mode == 2:
                            removed = sample(keys, offset, mode, width, frames, kind, single, True)
                            negative["normalize_keys"] += removed != want
                            if single:
                                batched = sample(keys, offset, mode, width, frames, kind, False)
                                negative["batch_root_arithmetic"] += batched != want
                        if clock["wrap"] and frames[1] == 0 and mode == 2:
                            removed = sample(keys, offset, mode, width, (last, last, 0.0), kind, single)
                            negative["clamp_wrapped_endpoint"] += removed != want
    assert len(keys) == offsets * 4
    if mismatches:
        raise AssertionError((len(mismatches), mismatches[:5]))
    assert all(negative.values()) and len(negative) == 3, negative
    watchdog_self_test()
    native = native_readback({row["asset"]: row for row in metadata["sequences"]}, keys)
    return {
        "result": "PASS_PREPARED_ACL_KEYS_AND_SAMPLING",
        "sequences": len(inventory),
        "track_modes": dict(counts),
        "sdk_sample_match": dict(sample_counts),
        "native_readback": native,
        "removed_contract_mismatches": dict(negative),
        "float_key_values": offsets,
        "target_sha256": hashlib.sha256(asset.read_bytes()).hexdigest(),
        "watchdog_self_test": "PASS",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset", type=Path, default=ROOT / "assets/ALSRuntime/Animations/als_prepared_acl.bin")
    parser.add_argument("--oracle", type=Path, default=HERE / "acl-probes.native.json.gz")
    args = parser.parse_args()
    oracle = args.oracle.read_bytes()
    if args.oracle.suffix == ".gz":
        oracle = gzip.decompress(oracle)
    print(json.dumps(replay(args.asset, json.loads(oracle)), indent=2))
