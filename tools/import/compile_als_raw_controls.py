import argparse
import base64
import json
import math
import re
import struct
import tempfile
from pathlib import Path

from capture_als_sequence_compression import mesh_inventory, read_capture


ROOT = Path(__file__).resolve().parents[2]
AUDIT = ROOT / "docs/audit/bf144/d227"


def compile_controls(captures, assets):
    meshes = mesh_inventory(assets)
    values = bytearray()
    spans = {}
    rows = []
    seen = set()

    def store(encoded, count):
        decoded = base64.b64decode(encoded, validate=True)
        if len(decoded) != count * 36 or not all(math.isfinite(value) for value, in struct.iter_unpack("<f", decoded)):
            raise ValueError("Invalid raw control key stream")
        first = decoded[:36]
        if decoded == first * count:
            decoded, count = first, 1
        if decoded not in spans:
            spans[decoded] = (len(values) // 4, count)
            values.extend(decoded)
        return spans[decoded]

    for capture in captures:
        mesh = capture["mesh"]
        if mesh in seen:
            raise ValueError(f"Duplicate source control mesh: {mesh}")
        seen.add(mesh)
        inventory = meshes[mesh]
        if [row["asset"] for row in capture["sequences"]] != list(inventory):
            raise ValueError(f"Incomplete raw control source inventory: {mesh}")
        for source in capture["sequences"]:
            asset = source["asset"]
            descriptor = (assets / inventory[asset][0]).read_text()
            path = re.search(r'\bpath:\s*"([^"]+)"', descriptor)[1]
            name = re.search(r'\banimation_name:\s*"([^"]+)"', descriptor)[1]
            controls = source["raw_controls"]
            count = controls["frame_count"]
            rate = [controls["rate_numerator"], controls["rate_denominator"]]
            tick = [controls["tick_numerator"], controls["tick_denominator"]]
            if count < 1 or min(rate) < 1 or rate != tick:
                raise ValueError(f"Invalid source control time domain: {asset}")
            tracks = []
            bone_indices = set()
            for track in controls["tracks"]:
                bone = track["bone"]
                if not 0 <= bone < len(capture["bones"]) or bone in bone_indices:
                    raise ValueError(f"Invalid raw control bone mapping: {asset}")
                bone_indices.add(bone)
                if "interpolated" in track:
                    raise ValueError(f"Raw control frame and interpolation keys differ: {asset}")
                discrete = store(track["discrete"], count)
                tracks.append((bone, discrete))
            rows.append({
                "gltf": path, "animation": name, "duration": source["duration"], "step": source["step"],
                "frame_rate": rate, "frame_count": count, "tracks": tracks,
            })
    if seen != meshes.keys() or len({(row["gltf"], row["animation"]) for row in rows}) != len(rows):
        raise ValueError("Incomplete or ambiguous raw control bindings")
    rows.sort(key=lambda row: (row["gltf"], row["animation"]))
    header = json.dumps(rows, separators=(",", ":")).encode()
    return struct.pack("<I", len(header)) + header + values


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--captures", type=Path, nargs="+",
                        default=[AUDIT / f"{mesh}.native.json.gz" for mesh in ("character", "bow")])
    parser.add_argument("--assets", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    data = compile_controls([read_capture(path) for path in args.captures], args.assets)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as stream:
        temporary = Path(stream.name)
        try:
            stream.write(data)
            stream.close()
            temporary.chmod(0o644)
            temporary.replace(args.output)
        finally:
            temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
