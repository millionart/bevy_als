import argparse
import bisect
import ctypes
import json
import math
import struct
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d227"))
from replay_reference_retention import control_at, f32, normalized, projected, tuple_transform, within
from capture_als_sequence_compression import read_capture
from compile_als_raw_curves import compile_curves
from compile_als_prepared_acl import run_tool


def fraction(value):
    limit = f32(0.99999994)
    return min(max(f32(f32(min(value, limit) + 0.5) - 0.5), 0.0), limit)


def frame_time(time, rate):
    value = time * rate[0] / rate[1]
    frame = math.floor(value)
    subframe = f32(value - frame)
    carry = int(subframe)
    return frame + carry, fraction(subframe - carry)


def convert_time(time, source_rate, target_rate):
    if source_rate == target_rate:
        return time
    frame, subframe = time
    numerator = target_rate[0] * source_rate[1]
    denominator = target_rate[1] * source_rate[0]
    integer = frame * numerator // denominator
    integer_fraction = frame * numerator / denominator - integer
    position = subframe * numerator / denominator + integer_fraction
    carry = math.floor(position)
    return integer + carry, fraction(f32(position - carry))


def from_bits(value):
    return struct.unpack("<d", struct.pack("<Q", value))[0]


def sample_curve(curve, source_time, rate, use_tick_rate=True, use_cubic=True):
    tick_rate = curve["tick_rate"] if use_tick_rate else rate
    frame, subframe = convert_time(source_time, rate, tick_rate)
    frames = curve["frames"]
    next_index = bisect.bisect_right(frames, frame)
    if next_index == 0:
        return f32(curve["endpoints"][0])
    if next_index == len(frames):
        return f32(curve["endpoints"][1])
    start = frames[next_index - 1]
    length = frames[next_index] - start
    offset = frame - start + subframe
    segment = curve["segments"][next_index - 1]
    if "Linear" in segment:
        value, slope = map(from_bits, segment["Linear"])
        return f32(value + offset * slope)
    a, b, c, d = map(from_bits, segment["Cubic"])
    alpha = f32(offset / length)

    def lerp(start, end):
        return start + (end - start) * alpha

    if not use_cubic:
        return f32(lerp(a, d))
    ab, bc, cd = lerp(a, b), lerp(b, c), lerp(c, d)
    return f32(lerp(lerp(ab, bc), lerp(bc, cd)))


def verify_curves(records):
    tests = {"crouch-raw": {"FeetCrossing"}, "cubic": {"PoseGrounded"},
             "tick": {"HandLeftIk", "LayerArmLeft", "LayerArmRight"}}
    counts = Counter()
    witness = {}
    for name, tested in tests.items():
        report = read_capture(HERE / f"{name}.native.json.gz")
        animation = report["asset_path"].rsplit(".", 1)[-1] + "_0"
        sequence = records[animation]
        rate = sequence["frame_rate"]
        for frame in report["sequence_pose_probe"]["frames"]:
            assert frame["pose_evaluation_mode"] == "raw_model"
            time = frame_time(frame["time"], rate)
            expected = {name.lower(): value for name, value in frame["curves"].items()}
            for curve in sequence["curves"]:
                name_key = curve["name"]
                if name_key not in tested:
                    continue
                value = sample_curve(curve, time, rate)
                assert value == expected[name_key.lower()], (name, frame["time"], name_key, value, expected)
                counts[name] += 1
                if name == "tick":
                    counts["source_tick_ablation"] += sample_curve(curve, time, rate, use_tick_rate=False) != value
                    counts["step_scalar_ablation"] += sample_curve(curve, (time[0], 0.0), rate) != value
                if name == "cubic":
                    counts["cubic_ablation"] += sample_curve(curve, time, rate, use_cubic=False) != value
                if name == "crouch-raw" and frame["time"] == f32(0.5302536488):
                    witness = {"time": frame["time"], "source_frame": time[0] + time[1], "value": value}
    assert witness["value"] == f32(0.9076095819473267)
    sequence = records["A_Als_Crouch_Walk_Right_Backward_0"]
    gltf = json.loads((ROOT / "assets" / sequence["gltf"]).read_bytes())
    animation = next(row for row in gltf["animations"] if row["name"] == sequence["animation"])
    legacy = next(row for row in animation["extensions"]["ALS_float_curves"]["curves"] if row["name"] == "FeetCrossing")
    right = bisect.bisect_right(legacy["times"], witness["time"])
    a, b = map(f32, legacy["values"][right - 1:right + 1])
    start, end = map(f32, legacy["times"][right - 1:right + 1])
    weight = f32(f32(witness["time"] - start) / f32(end - start))
    witness["legacy_seconds_value"] = f32(a + f32(f32(b - a) * weight))
    assert witness["legacy_seconds_value"] == f32(0.9076088070869446), witness
    assert counts["source_tick_ablation"] > 0 and counts["step_scalar_ablation"] > 0 and counts["cubic_ablation"] > 0, counts
    return dict(counts), witness


def source_rotations(work):
    work.mkdir(parents=True, exist_ok=True)
    report = work / "rotation.watchdog.json"
    report.unlink(missing_ok=True)
    library = work / "source_rotation_probe.dylib"
    run_tool(["clang++", "-dynamiclib", "-std=c++20", "-O2", "-ffp-contract=off", "-Wall", "-Wextra", "-Werror",
              str(HERE / "source_rotation_probe.cpp"), "-o", str(library)],
             report, "building the source rotation probe")
    source = ctypes.CDLL(str(library.resolve()))
    pointer = ctypes.POINTER(ctypes.c_double)
    source.source_normalize.argtypes = [pointer, pointer]
    source.source_normalize.restype = None
    source.source_nlerp.argtypes = [pointer, pointer, ctypes.c_double, pointer]
    source.source_nlerp.restype = None
    vector = ctypes.c_double * 4

    def normalize(rotation):
        out = vector()
        source.source_normalize(vector(*rotation), out)
        return tuple(out)

    def nlerp(start, end, alpha):
        if alpha <= 1e-5:
            return start
        if abs(f32(alpha - 1.0)) <= 1e-5:
            return end
        out = vector()
        source.source_nlerp(vector(*start), vector(*end), alpha, out)
        return tuple(out)

    return normalize, nlerp


def verify_bones(work):
    normalize, nlerp = source_rotations(work)
    packed = (ROOT / "assets/ALSRuntime/Animations/als_raw_fk_controls.bin").read_bytes()
    header, = struct.unpack_from("<I", packed)
    controls = json.loads(packed[4:4 + header])
    keys = packed[4 + header:]
    capture = read_capture(HERE.parent / "d224d/character.native.json.gz")
    references = capture["bones"]
    probe = read_capture(HERE / "crouch-raw.native.json.gz")
    clip = probe["asset_path"].rsplit(".", 1)[-1]
    sequence = next(row for row in controls if row["animation"] == clip + "_0")
    counts = Counter()
    for frame in probe["sequence_pose_probe"]["frames"]:
        for index, span in sequence["tracks"]:
            reference = references[index]
            candidate = control_at(sequence, keys, span, frame["time"], normalize, nlerp)
            initial = tuple_transform(reference["skeleton_pose"])
            output = tuple_transform(reference["mesh_pose"]) if within(candidate, initial) else candidate
            result = normalize(output[0]), output[1], output[2]
            actual = tuple_transform(frame["bones"][index]["local_transform"])
            got, expected = projected(result), projected(actual)
            scalar = control_at(sequence, keys, span, frame["time"])
            scalar_output = tuple_transform(reference["mesh_pose"]) if within(scalar, initial) else scalar
            counts["scalar_quaternion_ablation"] += projected((
                normalized(scalar_output[0]), scalar_output[1], scalar_output[2])) != expected
            if got == expected:
                counts["projected_bones_exact"] += 1
            else:
                assert reference["name"].startswith("ik_foot"), (
                    frame["time"], reference["name"], got, expected)
                counts["animation_relative_bones"] += 1
    assert counts["scalar_quaternion_ablation"] > 0
    return dict(counts)


def replay(work):
    captures = [read_capture(HERE / f"{name}.native.json.gz") for name in ("character", "bow")]
    records = compile_curves(captures, ROOT / "assets")
    assert records == json.loads((ROOT / "assets/ALSRuntime/Animations/als_raw_sequencer_curves.json").read_bytes())
    curves, witness = verify_curves({row["animation"]: row for row in records})
    return {"result": "PASS_SEQUENCER_FRAME_DOMAIN", "bound_sequences": len(records),
            "source_curves": sum(len(row["curves"]) for row in records),
            "curve_values": curves, "feet_crossing_witness": witness, "bone_values": verify_bones(work)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d228")
    print(json.dumps(replay(parser.parse_args().work), indent=2))
