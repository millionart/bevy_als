import bisect
import json
import math
import struct
import sys
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE.parent / "d226"))
from replay_raw_euler import accessor_values, compile_timelines, f32, load_gltf, nlerp, read_capture, roundtrip
from replay_raw_rotation import normalized
from coordinate_basis import source_rotation_to_runtime
from compile_als_raw_controls import compile_controls

FIELDS = ("rotation", "translation", "scale")
AXES = ("xyzw", "xyz", "xyz")
TOLERANCE = f32(0.0001)


def tuple_transform(transform):
    return tuple(tuple(transform[field][axis] for axis in axes) for field, axes in zip(FIELDS, AXES))


def within(candidate, reference):
    rotation, translation, scale = candidate
    return (
        min(max(abs(a - sign * b) for a, b in zip(rotation, reference[0])) for sign in (1, -1)) <= TOLERANCE
        and max(abs(a - b) for a, b in zip(translation, reference[1])) <= TOLERANCE
        and max(abs(a - b) for a, b in zip(scale, reference[2])) <= TOLERANCE
    )


def projected(reference):
    rotation, (x, y, z), (sx, sy, sz) = reference
    return tuple(f32(value) for value in (*source_rotation_to_runtime(rotation), x * 0.01, z * 0.01, y * 0.01, sx, sz, sy))


def retained_mesh(reference):
    rotation, translation, scale = tuple_transform(reference["mesh_pose"])
    return normalized(rotation), translation, scale


def gltf_candidate(clip, name, time):
    assets = ROOT / "assets"
    path, animation_name, timeline = next(row for row in compile_timelines(assets) if row[1] == clip + "_0")
    document, buffer = load_gltf(assets / path)
    animation = next(row for row in document["animations"] if row["name"] == animation_name)
    keys = {}
    for channel in animation["channels"]:
        if document["nodes"][channel["target"]["node"]]["name"] == name:
            sampler = animation["samplers"][channel["sampler"]]
            keys[channel["target"]["path"]] = accessor_values(document, buffer, sampler["output"])
    second = bisect.bisect_right(timeline, time)
    first = second - 1
    alpha = f32(f32(time - timeline[first]) / f32(timeline[second] - timeline[first]))
    rotation = nlerp(keys["rotation"][first], keys["rotation"][second], alpha)
    rotation = tuple(f32(value) for value in roundtrip(rotation)[0]) if alpha > 1e-8 else rotation

    def linear(field):
        a, b = keys[field][first], keys[field][second]
        return tuple(f32(start + f32(f32(end - start) * alpha)) for start, end in zip(a, b))

    x, y, z = linear("translation")
    sx, sy, sz = linear("scale")
    return tuple(source_rotation_to_runtime(rotation)), (x * 100.0, z * 100.0, y * 100.0), (sx, sz, sy)


def control_at(sequence, keys, span, time, normalize=normalized, blend_rotation=nlerp):
    numerator, denominator = sequence["frame_rate"]
    decimal = time * numerator / denominator
    frame = int(decimal)
    fraction = f32(decimal - frame)
    carry = int(fraction)
    first = min(frame + carry, sequence["frame_count"] - 1)
    second = min(first + 1, sequence["frame_count"] - 1)
    maximum = f32(0.99999994)
    alpha = min(max(f32(f32(min(fraction - carry, maximum) + 0.5) - 0.5), 0.0), maximum)
    if sequence["step"] or first == second:
        alpha = 0.0
    offset, count = span

    def key(index):
        x, y, z, roll, pitch, yaw, sx, sy, sz = struct.unpack_from("<9f", keys, 4 * (offset + 9 * min(index, count - 1)))
        pitch_rad, yaw_rad, roll_rad = (value * (math.pi / 180.0 * 0.5) for value in (pitch, yaw, roll))
        sp, cp = math.sin(pitch_rad), math.cos(pitch_rad)
        syaw, cy = math.sin(yaw_rad), math.cos(yaw_rad)
        sr, cr = math.sin(roll_rad), math.cos(roll_rad)
        rotation = (cr * (sp * syaw) - sr * (cp * cy), -cr * (sp * cy) - sr * (cp * syaw),
                    cr * (cp * syaw) - sr * (sp * cy), cr * (cp * cy) + sr * (sp * syaw))
        return rotation, (x, y, z), (sx, sy, sz)

    start = key(first)
    if alpha <= 1e-8:
        return normalize(start[0]), start[1], start[2]
    end = key(second)
    rotation = blend_rotation(start[0], end[0], alpha)
    rotation = normalize(source_rotation_to_runtime(roundtrip(source_rotation_to_runtime(rotation))[0]))
    if alpha <= 1e-5:
        return tuple(rotation), start[1], start[2]
    if abs(f32(alpha - 1.0)) <= 1e-5:
        return tuple(rotation), end[1], end[2]
    return (tuple(rotation), *(tuple(f32(a + (b - a) * alpha) for a, b in zip(left, right))
                               for left, right in zip(start[1:], end[1:])))


def replay():
    source = read_capture(HERE.parent / "d224d/character.native.json.gz")
    references = {bone["name"]: bone for bone in source["bones"]}
    captures = [read_capture(HERE / f"{mesh}.native.json.gz") for mesh in ("character", "bow")]
    packed = (ROOT / "assets/ALSRuntime/Animations/als_raw_fk_controls.bin").read_bytes()
    assert packed == compile_controls(captures, ROOT / "assets")
    for mesh, capture in zip(("character", "bow"), captures):
        for sequence in capture["sequences"]:
            sequence.pop("raw_controls")
        assert capture == read_capture(HERE.parent / "d226" / f"{mesh}.native.json.gz")
    header, = struct.unpack_from("<I", packed)
    sequences = json.loads(packed[4:4 + header])
    keys = packed[4 + header:]
    assert len(sequences) == 128
    native = read_capture(HERE / "crouch-raw.native.json.gz")
    assert native["sequence_pose_probe"] == read_capture(HERE / "restored-stage.native.json.gz")["sequence_pose_probe"]
    clip = native["asset_path"].rsplit(".", 1)[-1]
    sequence = next(row for row in sequences if row["animation"] == clip + "_0")
    tracks = {source["bones"][index]["name"]: span for index, span in sequence["tracks"]}
    frames = native["sequence_pose_probe"]["frames"]
    assert frames[0] == frames[-1] and frames[1] == frames[3]
    counts = Counter()
    peak_control_rotation_difference = 0.0
    witness = "calf_twist_01_l"
    conditions = []
    for frame in frames:
        assert frame["pose_evaluation_mode"] == "raw_model" and frame["source_stages"]["apply_mode"] == 2
        outputs = {bone["name"]: bone["local_transform"] for bone in frame["bones"]}
        for row in frame["source_stages"]["bones"]:
            if "native_channel_control" not in row:
                continue
            name = row["name"]
            reference = references[name]
            initial = tuple_transform(row["bone_initial_local"])
            assert initial == tuple_transform(reference["skeleton_pose"])
            control = tuple_transform(row["native_channel_control"])
            keeps = row["bone_initial_local"] == row["bone_local"]
            assert within(control, initial) == keeps, (name, frame["time"])
            sampled = control_at(sequence, keys, tracks[name], frame["time"])
            assert within(sampled, initial) == keeps, (name, frame["time"], sampled, control)
            assert sampled[1:] == control[1:], (name, frame["time"], sampled, control)
            difference = max(abs(a - b) for a, b in zip(sampled[0], control[0]))
            peak_control_rotation_difference = max(peak_control_rotation_difference, difference)
            counts["fk_controls"] += 1
            if keeps:
                mesh = projected(retained_mesh(reference))
                actual = projected(tuple_transform(outputs[name]))
                assert mesh == actual, (name, frame["time"], mesh, actual)
                counts["mesh_references_preserved"] += 1
            if name == witness:
                conditions.append(keeps)
    assert conditions == [True, True, False, True, True]
    assert peak_control_rotation_difference < 1e-9
    frame = frames[1]
    output = next(bone["local_transform"] for bone in frame["bones"] if bone["name"] == witness)
    candidate = gltf_candidate(clip, witness, f32(frame["time"]))
    reference = tuple_transform(references[witness]["skeleton_pose"])
    expected = projected(tuple_transform(output))
    assert not within(candidate, reference) and projected(candidate) != expected
    control = control_at(sequence, keys, tracks[witness], f32(frame["time"]))
    assert within(control, reference) and projected(control) != expected
    assert projected(retained_mesh(references[witness])) == expected
    for index in (1, 2):
        changed = list(reference)
        changed[index] = (reference[index][0] + 1.1 * TOLERANCE, *reference[index][1:])
        assert not within(changed, reference)
    return {"result": "PASS_RAW_FK_REFERENCE_RETENTION", "witness": witness, "bound_sequences": len(sequences),
            "fk_tracks": sum(len(row["tracks"]) for row in sequences),
            "peak_double_rotation_control_difference": peak_control_rotation_difference,
            "native_frame_count": len(frames), "witness_keeps_reference": conditions,
            "gltf_before_projection": projected(candidate), "restored_projection": expected, **counts}


if __name__ == "__main__":
    print(json.dumps(replay(), indent=2))
