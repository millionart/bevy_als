import argparse
import base64
import json
import math
import re
import struct
import tempfile
from pathlib import Path

from capture_als_sequence_compression import mesh_inventory, read_capture, validate_capture
from compile_als_event_tracks import f32
from compile_als_prepared_acl import runtime_bone_paths
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE


ROOT = Path(__file__).resolve().parents[2]


def decode_track(stream, offset, channel, frame_count):
    rotation = channel == "rotation"
    if offset == -1:
        value = [0.0, 0.0, 0.0, 1.0] if rotation else [0.0, 0.0, 0.0]
        return {"keys": [value], "frames": []}, "identity"
    if offset < 0 or offset % 4:
        raise ValueError(f"Invalid PerTrack offset: {offset}")
    header, = struct.unpack_from("<I", stream, offset)
    count = header & 0xFFFFFF
    mask = (header >> 24) & 7
    format_id = header >> 28
    if count < 1 or format_id not in (1, 2) or (rotation and format_id == 1 and mask not in (0, 7)):
        raise ValueError(f"Unsupported PerTrack header: {channel} {header:#010x}")
    axes = [axis for axis in range(3) if mask & (1 << axis) or (mask == 0 and format_id == 1)]
    component = "f" if format_id == 1 else "H"
    key_format = struct.Struct(f"<{len(axes)}{component}")
    position = offset + 4
    keys = []
    for _ in range(count):
        encoded = key_format.unpack_from(stream, position)
        position += key_format.size
        values = [0.0, 0.0, 0.0]
        for axis, value in zip(axes, encoded):
            if format_id == 2:
                value = f32((value - 32767) * f32(1 / 32767)) if rotation else f32(value - 255)
            values[axis] = value
        if rotation:
            squared = 1.0
            for value in values:
                squared = f32(squared - f32(value * value))
            values.append(f32(math.sqrt(max(squared, 0.0))))
        if not all(math.isfinite(value) for value in values):
            raise ValueError("Non-finite PerTrack key")
        keys.append(values)
    frames = []
    if header & (1 << 27):
        table = (position + 3) & ~3
        integer = "H" if frame_count > 255 else "B"
        frames = list(struct.unpack_from(f"<{count}{integer}", stream, table))
        if frames[0] != 0 or frames[-1] + 1 != frame_count or any(a >= b for a, b in zip(frames, frames[1:])):
            raise ValueError(f"Invalid PerTrack frame table: {frames}")
    return {"keys": keys, "frames": frames}, "Float96NoW" if format_id == 1 else "Fixed48NoW"


def compile_capture(capture, assets):
    inventory = mesh_inventory(assets)[capture["mesh"]]
    validate_capture(capture, capture["mesh"], inventory)
    row, = capture["sequences"]
    clip, _ = inventory[row["asset"]]
    descriptor = (assets / clip).read_text()
    gltf = re.search(r'\bpath:\s*"([^"]+)"', descriptor)[1]
    animation = re.search(r'\banimation_name:\s*"([^"]+)"', descriptor)[1]
    skeleton = re.search(r'\bskeleton:\s*"([^"]+)"', descriptor)[1]
    paths = runtime_bone_paths(assets, skeleton, capture["bones"])
    data = row["compression"]
    mapping = data["track_to_skeleton"]
    layout = data["per_track"]
    if (layout["key_encoding"] != 2 or layout["track_offsets"] != 2 * len(mapping)
            or layout["scale_offsets"] != len(mapping) or layout["scale_strip_size"] != 1
            or len(mapping) != len(paths) or row["additive_type"] != 0
            or row["forced_root_lock"] is not None or data["curve_bytes"] != 0):
        raise ValueError("PerTrack clip metadata differs from the installed Bow")
    raw = base64.b64decode(data["bone_stream_base64"], validate=True)
    count = layout["track_offsets"] + layout["scale_offsets"]
    offsets = struct.unpack_from(f"<{count}i", raw)
    stream = raw[count * 4:]
    if len(stream) != layout["key_bytes"]:
        raise ValueError("PerTrack stream length differs from its native layout")
    frame_count = data["key_count"]
    bones = [None] * len(paths)
    for index, bone in enumerate(mapping):
        translation = offsets[2 * index]
        rotation = offsets[2 * index + 1]
        scale = offsets[layout["track_offsets"] + index]
        decoded = {"path": list(paths[bone]), "reference": capture["bones"][bone]}
        formats = {}
        for channel, offset in (("rotation", rotation), ("translation", translation), ("scale", scale)):
            decoded[channel], formats[channel] = decode_track(stream, offset, channel, frame_count)
        decoded["formats"] = formats
        bones[bone] = decoded
    return {
        "format": "als_prepared_per_track_v1", "asset": row["asset"], "clip": clip,
        "gltf": gltf, "animation": animation, "duration": row["duration"], "step": row["step"],
        "frame_rate": [data["sampling_rate_numerator"], data["sampling_rate_denominator"]],
        "frame_count": frame_count,
        "source_length_to_runtime_scale": SOURCE_LENGTH_TO_RUNTIME_SCALE,
        "bones": bones,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture", type=Path, default=ROOT / "docs/audit/bf144/d224e/bow.native.json.gz")
    parser.add_argument("--assets", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    data = compile_capture(read_capture(args.capture), args.assets)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as stream:
        temporary = Path(stream.name)
        try:
            stream.write((json.dumps(data, separators=(",", ":")) + "\n").encode())
            stream.close()
            temporary.chmod(0o644)
            temporary.replace(args.output)
        finally:
            temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
