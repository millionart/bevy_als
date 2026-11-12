import argparse
import json
import math
import re
import struct
from pathlib import Path

from capture_als_sequence_compression import mesh_inventory, read_capture
from coordinate_basis import source_heading_yaw_degrees_to_runtime


ROOT = Path(__file__).resolve().parents[2]
AUDIT = ROOT / "docs/audit/bf144/d228"


def bits(value):
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def compile_curve(curve, name):
    keys = curve["keys"]
    if (not keys or curve["pre_infinity"] != 4 or curve["post_infinity"] != 4
            or any(key["weight_mode"] != 0 or key["interp_mode"] not in (0, 2) for key in keys)):
        raise ValueError(f"Unsupported installed MovieScene curve: {name}")
    frames = [key["frame"] for key in keys]
    if any(first >= second for first, second in zip(frames, frames[1:])):
        raise ValueError(f"Unordered MovieScene curve frames: {name}")
    direction = source_heading_yaw_degrees_to_runtime(1.0) if name.encode().lower() == b"rotationyawspeed" else 1.0
    values = [key["value"] * direction for key in keys]
    segments = []
    for left, right, start, end in zip(keys, keys[1:], values, values[1:]):
        length = right["frame"] - left["frame"]
        leave = left["leave_tangent"] * direction
        arrive = right["arrive_tangent"] * direction
        if 2 in (left["interp_mode"], right["interp_mode"]) and (start != end or leave or arrive):
            coefficients = [start, start + leave * length * (1.0 / 3.0), end - arrive * length * (1.0 / 3.0), end]
            segment = "Cubic"
        else:
            coefficients = [start, (end - start) / length]
            segment = "Linear"
        if not all(math.isfinite(value) for value in coefficients):
            raise ValueError(f"Non-finite MovieScene curve segment: {name}")
        segments.append({segment: [bits(value) for value in coefficients]})
    rate = [curve["tick_numerator"], curve["tick_denominator"]]
    if min(rate) <= 0:
        raise ValueError(f"Invalid MovieScene curve tick rate: {name}")
    return {"name": name, "tick_rate": rate, "frames": frames, "endpoints": [values[0], values[-1]], "segments": segments}


def compile_curves(captures, assets):
    meshes = mesh_inventory(assets)
    seen = set()
    records = []
    for capture in captures:
        mesh = capture["mesh"]
        inventory = meshes[mesh]
        if mesh in seen or [row["asset"] for row in capture["sequences"]] != list(inventory):
            raise ValueError(f"Invalid raw curve inventory: {mesh}")
        seen.add(mesh)
        for row in capture["sequences"]:
            descriptor = (assets / inventory[row["asset"]][0]).read_text()
            data = row["raw_curves"]
            names = {entry["name"].encode().lower(): entry["name"]
                     for entry in row["compression"]["indexed_curve_names"]}
            keys = [curve["name"].encode().lower() for curve in data["curves"]]
            if set(keys) != names.keys() or len(keys) != len(names):
                raise ValueError(f"Raw and prepared curve identity mismatch: {row['asset']}")
            records.append({
                "gltf": re.search(r'\bpath:\s*"([^"]+)"', descriptor)[1],
                "animation": re.search(r'\banimation_name:\s*"([^"]+)"', descriptor)[1],
                "duration": row["duration"], "frame_rate": [data["rate_numerator"], data["rate_denominator"]],
                "curves": [compile_curve(curve, names[key]) for curve, key in zip(data["curves"], keys)],
            })
    if seen != meshes.keys():
        raise ValueError("Incomplete installed raw curve corpus")
    return sorted(records, key=lambda record: (record["gltf"], record["animation"]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--captures", type=Path, nargs="+",
                        default=[AUDIT / f"{mesh}.native.json.gz" for mesh in ("character", "bow")])
    parser.add_argument("--assets", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = compile_curves([read_capture(path) for path in args.captures], args.assets)
    args.output.write_text(json.dumps(result, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
